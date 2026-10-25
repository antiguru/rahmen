use image::DynamicImage;
use log::error;
use rahmen::errors::RahmenResult;
use rahmen::provider::{Provider, StatusLineFormatter, load_image_from_path};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(super) struct Prepared {
    pub image: Arc<DynamicImage>,
    pub status: Option<String>,
}

#[derive(Debug)]
pub(super) enum Outcome {
    Image(Prepared),
    Terminate,
}

pub(super) struct Loader {
    requests: SyncSender<()>,
    results: Receiver<Outcome>,
    stop: Arc<AtomicBool>,
}

impl Loader {
    pub fn new(
        mut provider: Box<dyn Provider<PathBuf> + Send>,
        formatter: StatusLineFormatter,
        buffer_max_size: usize,
    ) -> Self {
        let (requests, request_rx) = mpsc::sync_channel(1);
        let (result_tx, results) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        // Dropping the handle detaches the worker. Closing the UI must not join I/O.
        std::thread::spawn(move || {
            while request_rx.recv().is_ok() {
                let outcome = loop {
                    if stopping.load(Ordering::Acquire) { return; }
                    // obtain next path
                    let path = match fatal_err(provider.next_image()) {
                        Ok(path) => path,
                        Err(_) => break Outcome::Terminate,
                    };
                    if stopping.load(Ordering::Acquire) { return; }
                    // Load image
                    match suppress_err(load_image_from_path(&path, Some(buffer_max_size))) {
                        Ok(image) => {
                            if stopping.load(Ordering::Acquire) { return; }
                            // Ordinary metadata failures preserve the existing status line.
                            let status = formatter.format(&path).ok();
                            break Outcome::Image(Prepared { image: Arc::new(image), status });
                        }
                        Err(_) => continue,
                    }
                };
                if stopping.load(Ordering::Acquire) || result_tx.send(outcome).is_err() { return; }
            }
        });
        Self { requests, results, stop }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

enum Slot {
    Dormant,
    Loading,
    Ready(Outcome),
    Presenting,
    Stopped,
}

pub(super) struct Prefetch {
    loader: Loader,
    slot: Slot,
    deadline: Duration,
    delay: Duration,
}

impl Prefetch {
    pub fn new(loader: Loader, delay: Duration) -> Self {
        Self { loader, slot: Slot::Dormant, deadline: Duration::ZERO, delay }
    }

    pub fn loading(&self) -> bool { matches!(self.slot, Slot::Loading) }

    pub fn poll(&mut self) {
        if self.loading() {
            match self.loader.results.try_recv() {
                Ok(outcome) => self.slot = Slot::Ready(outcome),
                Err(TryRecvError::Disconnected) => {
                    error!("Image worker disconnected, terminating");
                    self.slot = Slot::Ready(Outcome::Terminate);
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    pub fn take_due(&mut self, now: Duration) -> Option<Outcome> {
        if now < self.deadline || !matches!(self.slot, Slot::Ready(_)) { return None; }
        let Slot::Ready(outcome) = std::mem::replace(&mut self.slot, Slot::Presenting) else { unreachable!() };
        if matches!(outcome, Outcome::Terminate) { self.slot = Slot::Stopped; }
        Some(outcome)
    }

    pub fn presented(&mut self, now: Duration) {
        let interval = match self.slot {
            Slot::Dormant => Duration::from_secs(1),
            Slot::Presenting => self.delay,
            _ => return,
        };
        self.deadline = now.saturating_add(interval);
        self.slot = match self.loader.requests.try_send(()) {
            Ok(()) => Slot::Loading,
            Err(_) => Slot::Ready(Outcome::Terminate),
        };
    }
}

type RunResult<T> = Result<T, RunControl>;
/// dataflow control, this is used as result R part
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum RunControl {
    /// terminate stream processing (by external command)
    Terminate,
    /// stream processing encountered an error, but will continue
    Suppressed,
}

/// error handler for display stuff
fn fatal_err<T>(result: RahmenResult<Option<T>>) -> RunResult<T> {
    match result {
        // empty result means we terminate as planned (e.g. end of list)
        Ok(None) => Err(RunControl::Terminate),
        // we process the result
        Ok(Some(t)) => Ok(t),
        // display error and terminate
        Err(e) => {
            error!("Encountered error, terminating: {}", e);
            Err(RunControl::Terminate)
        }
    }
}

/// to keep running the dataflow when there's a stream error
fn suppress_err<T>(result: RahmenResult<T>) -> RunResult<T> {
    result.map_err(|e| {
        // just notify about error but keep processing
        error!("Encountered error, suppressing: {}", e);
        RunControl::Suppressed
    })
}

#[cfg(test)]
mod tests;
