use super::dataflow::{Render, RunControl, Update};
use super::prefetch::{Outcome, Prefetch};
use log::error;
use rahmen::dataflow::Configuration;
use rahmen::display::Display;
use rahmen::errors::{RahmenError, RahmenResult};
use std::time::Duration;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::worker::Worker;

pub(super) fn display_loop<'a>(
    worker: &'a mut Worker,
    input_configuration: &'a mut InputHandle<
        Duration,
        CapacityContainerBuilder<Vec<Configuration>>,
    >,
    input_updates: &'a mut InputHandle<Duration, CapacityContainerBuilder<Vec<Update>>>,
    probe: &'a ProbeHandle<Duration>,
    output: std::sync::mpsc::Receiver<Event<Duration, Vec<Result<Render, RunControl>>>>,
    mut prefetch: Prefetch,
    mut clock: impl FnMut() -> Duration + 'a,
) -> impl FnMut(&mut dyn Display) -> RahmenResult<()> + 'a {
    let mut dimensions = None;
    let mut first_presentation = true;
    let mut was_loading = false;

    move |display: &mut dyn Display| {
        let now = clock().max(*input_configuration.time());
        input_configuration.advance_to(now);
        input_updates.advance_to(now);
        prefetch.poll();
        if prefetch.loading() != was_loading {
            was_loading = prefetch.loading();
            input_updates.send(Update::Loading(was_loading));
        }
        let mut promoted = false;
        if let Some(outcome) = prefetch.take_due(now) {
            match outcome {
                Outcome::Image(prepared) => {
                    input_updates.send(Update::Image(prepared));
                    promoted = true;
                }
                Outcome::Terminate => input_updates.send(Update::Terminate),
            }
        }

        if Some(display.dimensions()) != dimensions {
            dimensions = Some(display.dimensions());
            input_configuration.send(Configuration::ScreenDimensions(
                display.dimensions().0,
                display.dimensions().1,
            ));
        }
        let frontier = now + Duration::from_nanos(1);
        input_configuration.advance_to(frontier);
        input_updates.advance_to(frontier);
        while probe.less_than(&frontier) {
            worker.step();
        }
        let mut has_update = false;
        let result = match output.try_iter().all(|result| match result {
            // Continue processing on progress messages
            Event::Progress(_) => true,
            // Handle data messages by rending an image and determining whether to terminate
            Event::Messages(_, r) => {
                let mut terminate = false;
                for result in r {
                    match result {
                        Ok(Render::Image(key, anchor, ref img)) => {
                            has_update = true;
                            if let Err(err) = display.render(key, anchor, img.as_ref()) {
                                error!("Render failed: {}", err);
                                terminate = true;
                            }
                        }
                        Ok(Render::Blank(key, anchor, size)) => {
                            has_update = true;
                            if let Err(err) = display.blank(key, anchor, size) {
                                error!("Blank failed: {}", err);
                                terminate = true;
                            }
                        }
                        Err(RunControl::Terminate) => terminate = true,
                        _ => {}
                    }
                }
                !terminate
            }
        }) {
            true => Ok(()),
            false => Err(RahmenError::Terminate),
        };
        result?;
        if has_update {
            display.update()?;
        }
        // A zero-area viewport retains graph content but produces no display operations.
        // Acknowledge that logical presentation so EOF still completes without a resize.
        if first_presentation || promoted {
            prefetch.presented(clock());
            first_presentation = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
