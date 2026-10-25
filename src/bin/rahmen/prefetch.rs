use image::DynamicImage;
use log::error;
use rahmen::errors::RahmenResult;
use rahmen::provider::{Provider, StatusLineFormatter, load_image_from_path};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TryRecvError};
use std::time::Duration;
use timely::dataflow::operators::generic::builder_raw::OperatorBuilder;
use timely::scheduling::Activator;
use timely::worker::Worker;

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
    slot: Rc<RefCell<Slot>>,
    consumer: Activator,
    stop: Arc<AtomicBool>,
}

impl Loader {
    pub fn new(
        worker: &mut Worker,
        mut provider: Box<dyn Provider<PathBuf> + Send>,
        formatter: StatusLineFormatter,
        buffer_max_size: usize,
    ) -> Self {
        let (requests, request_rx) = mpsc::sync_channel(1);
        let (result_tx, results) = mpsc::sync_channel(1);
        let slot = Rc::new(RefCell::new(Slot::Dormant));
        let consumer_slot = Rc::clone(&slot);
        // This consumer has no output ports and therefore holds no capabilities.
        // A pending load cannot hold back the rendering graph's clock frontier.
        let consumer = worker.dataflow::<Duration, _, _>(|scope| {
            let builder = OperatorBuilder::new("Prepared image receiver".into(), scope);
            let address = builder.operator_info().address;
            let consumer = scope.worker().activator_for(address);
            builder.build(move |_| {
                let mut slot = consumer_slot.borrow_mut();
                if matches!(*slot, Slot::Stopped) {
                    return false;
                }
                if matches!(*slot, Slot::Loading) {
                    match results.try_recv() {
                        Ok(outcome) => *slot = Slot::Ready(outcome),
                        Err(TryRecvError::Disconnected) => {
                            error!("Image worker disconnected, terminating");
                            *slot = Slot::Ready(Outcome::Terminate);
                        }
                        Err(TryRecvError::Empty) => {}
                    }
                }
                true
            });
            consumer
        });
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        // Dropping the handle detaches the worker. Closing the UI must not join I/O.
        std::thread::spawn(move || {
            while request_rx.recv().is_ok() {
                let outcome = loop {
                    if stopping.load(Ordering::Acquire) {
                        return;
                    }
                    // obtain next path
                    let path = match fatal_err(provider.next_image()) {
                        Ok(path) => path,
                        Err(_) => break Outcome::Terminate,
                    };
                    if stopping.load(Ordering::Acquire) {
                        return;
                    }
                    // Load image
                    match suppress_err(load_image_from_path(&path, Some(buffer_max_size))) {
                        Ok(image) => {
                            if stopping.load(Ordering::Acquire) {
                                return;
                            }
                            // Ordinary metadata failures preserve the existing status line.
                            let status = formatter.format(&path).ok();
                            break Outcome::Image(Prepared {
                                image: Arc::new(image),
                                status,
                            });
                        }
                        Err(_) => continue,
                    }
                };
                if stopping.load(Ordering::Acquire) || result_tx.send(outcome).is_err() {
                    return;
                }
            }
        });
        Self {
            requests,
            slot,
            consumer,
            stop,
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        *self.slot.borrow_mut() = Slot::Stopped;
        self.consumer.activate();
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
    deadline: Duration,
    delay: Duration,
}

impl Prefetch {
    pub fn new(loader: Loader, delay: Duration) -> Self {
        Self {
            loader,
            deadline: Duration::ZERO,
            delay,
        }
    }

    pub fn loading(&self) -> bool {
        matches!(*self.loader.slot.borrow(), Slot::Loading)
    }

    pub fn take_due(&mut self, now: Duration) -> Option<Outcome> {
        let mut slot = self.loader.slot.borrow_mut();
        if now < self.deadline || !matches!(*slot, Slot::Ready(_)) {
            return None;
        }
        let Slot::Ready(outcome) = std::mem::replace(&mut *slot, Slot::Presenting) else {
            unreachable!()
        };
        if matches!(outcome, Outcome::Terminate) {
            *slot = Slot::Stopped;
        }
        Some(outcome)
    }

    pub fn presented(&mut self, now: Duration) {
        let mut slot = self.loader.slot.borrow_mut();
        let interval = match *slot {
            Slot::Dormant => Duration::from_secs(1),
            Slot::Presenting => self.delay,
            _ => return,
        };
        self.deadline = now.saturating_add(interval);
        *slot = match self.loader.requests.try_send(()) {
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
pub(super) mod tests;
