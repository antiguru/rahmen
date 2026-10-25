use super::dataflow::{Render, RunControl};
use log::error;
use rahmen::dataflow::Configuration;
use rahmen::display::Display;
use rahmen::errors::{RahmenError, RahmenResult};
use std::time::{Duration, Instant};
use timely::container::CapacityContainerBuilder;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::worker::Worker;

pub(super) fn display_loop<'a>(
    worker: &'a mut Worker,
    input_configuration: &'a mut InputHandle<Duration, CapacityContainerBuilder<Vec<Configuration>>>,
    probe: &'a ProbeHandle<Duration>,
    output: std::sync::mpsc::Receiver<Event<Duration, Vec<Result<Render, RunControl>>>>,
    start_time: Instant,
    delay: Duration,
) -> impl FnMut(&mut dyn Display) -> RahmenResult<()> + 'a {
    let mut dimensions = None;
    let mut next_image_at = start_time.elapsed() + Duration::from_secs(1);

    move |display: &mut dyn Display| {
        let now = start_time.elapsed();

        if next_image_at < now {
            input_configuration.send(Configuration::Tick);
            next_image_at = now + delay;
        }

        if Some(display.dimensions()) != dimensions {
            dimensions = Some(display.dimensions());
            input_configuration.send(Configuration::ScreenDimensions(
                display.dimensions().0,
                display.dimensions().1,
            ));
        }
        input_configuration.advance_to(now);
        while probe.less_than(&now) {
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
        if result.is_ok() && has_update {
            display.update()
        } else {
            result
        }
    }
}
