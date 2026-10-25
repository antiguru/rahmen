use super::*;
use font_kit::loaders::freetype::Font;
use image::{Rgb, RgbImage};
use rahmen::config::Element;
use rahmen::errors::RahmenError;
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Instant;
use timely::communication::Allocator;
use timely::worker::Config;

// Exiv2 initializes native process-wide metadata state. Keep fixture access serial
// within this binary while the other headless graph tests remain parallel.
static METADATA: Mutex<()> = Mutex::new(());

type Frame = (Duration, RunResult<Render>);

struct Paths {
    paths: VecDeque<PathBuf>,
    error: bool,
    calls: Rc<Cell<usize>>,
}

impl Provider<PathBuf> for Paths {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        self.calls.set(self.calls.get() + 1);
        if self.error {
            Err(RahmenError::Retry)
        } else {
            Ok(self.paths.pop_front())
        }
    }
}

struct Graph {
    worker: Worker,
    input: InputHandle<Duration, CapacityContainerBuilder<Vec<Configuration>>>,
    probe: ProbeHandle<Duration>,
    output: std::sync::mpsc::Receiver<Event<Duration, Vec<RunResult<Render>>>>,
    calls: Rc<Cell<usize>>,
}

impl Graph {
    fn new(paths: Vec<PathBuf>, error: bool, show_time: bool) -> Self {
        let allocator =
            Allocator::Thread(timely::communication::allocator::thread::Thread::default());
        let mut worker = Worker::new(Config::default(), allocator, Some(Instant::now()));
        let mut input = InputHandle::new();
        let probe = ProbeHandle::new();
        let calls = Rc::new(Cell::new(0));
        let provider = Box::new(Paths {
            paths: paths.into(),
            error,
            calls: calls.clone(),
        });
        let formatter = StatusLineFormatter::new(
            std::iter::once(Element {
                exif_tags: vec!["Iptc.Application2.City".into()],
                case_conversion: None,
                capitalize: None,
                replace: None,
            }),
            None,
            ", ".into(),
        )
        .unwrap();
        let font = FontRenderer::with_font(
            Font::from_path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0).unwrap(),
        );
        let output = build(
            &mut worker,
            &mut input,
            &probe,
            provider,
            formatter,
            font,
            Options {
                buffer_max_size: 16_000_000,
                show_time,
                time_format: "FIXED".into(),
            },
        );
        Self {
            worker,
            input,
            probe,
            output,
            calls,
        }
    }

    fn configure(&mut self) {
        self.input.send(Configuration::FontSize(12.));
        self.input.send(Configuration::FontCanvasVStretch(1.5));
        self.input.send(Configuration::ScreenDimensions(96, 80));
    }

    fn drain(&self) -> Vec<Frame> {
        self.output
            .try_iter()
            .flat_map(|event| match event {
                Event::Messages(t, messages) => messages
                    .into_iter()
                    .map(move |message| (t, message))
                    .collect(),
                Event::Progress(_) => vec![],
            })
            .collect()
    }

    fn advance(&mut self, millis: u64) -> Vec<Frame> {
        let time = Duration::from_millis(millis);
        self.input.advance_to(time);
        for _ in 0..10_000 {
            if !self.probe.less_than(&time) {
                return self.drain();
            }
            self.worker.step();
        }
        panic!("production graph failed to advance to {time:?}");
    }

    fn close(mut self) -> Vec<Frame> {
        std::mem::take(&mut self.input).close();
        for _ in 0..10_000 {
            if !self.worker.step() {
                assert!(self.probe.done());
                return self.drain();
            }
        }
        panic!("production graph failed to release pending notifications");
    }
}

fn image(frames: &[Frame], key: usize) -> (&Duration, Vector, &DynamicImage) {
    frames
        .iter()
        .find_map(|(time, render)| match render {
            Ok(Render::Image(k, pos, image)) if *k == key => Some((time, *pos, image.as_ref())),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing image key {key}: {frames:?}"))
}

#[test]
fn splash_greeting_and_configuration_only_updates() {
    let mut graph = Graph::new(vec![], false, false);
    graph.configure();
    graph
        .input
        .send(Configuration::Splash(Arc::new(DynamicImage::ImageRgb8(
            RgbImage::from_pixel(4, 4, Rgb([23, 45, 67])),
        ))));
    graph.input.send(Configuration::Greeting("HELLO".into()));
    for _ in 0..100 {
        graph.worker.step();
    }
    assert!(graph.drain().is_empty());
    assert!(graph.probe.less_equal(&Duration::ZERO));
    let frames = graph.advance(1);
    assert_eq!(frames.len(), 2);
    let (time, pos, img) = image(&frames, 1);
    assert_eq!(
        (*time, pos, img.dimensions()),
        (Duration::ZERO, Vector::new(17, 0), (62, 62))
    );
    assert!(img.to_rgb8().pixels().all(|p| *p == Rgb([23, 45, 67])));
    let (_, pos, img) = image(&frames, 2);
    assert_eq!((pos, img.dimensions()), (Vector::new(0, 62), (96, 18)));
    assert!(img.to_rgb8().pixels().any(|p| p.0 != [0, 0, 0]));
    graph.input.send(Configuration::ScreenDimensions(80, 60));
    let frames = graph.advance(2);
    assert_eq!(image(&frames, 1).1, Vector::new(19, 0));
    assert_eq!(image(&frames, 1).2.dimensions(), (42, 42));
    assert_eq!(image(&frames, 2).1, Vector::new(0, 42));
    assert!(
        frames
            .iter()
            .any(|(_, r)| matches!(r, Ok(Render::Blank(1, _, _))))
    );
    assert!(
        frames
            .iter()
            .any(|(_, r)| matches!(r, Ok(Render::Blank(2, _, _))))
    );
    assert!(frames.iter().all(|(t, _)| *t == Duration::from_millis(1)));
    assert!(
        graph.advance(3000).is_empty(),
        "disabled clock must not schedule frames"
    );
    assert!(graph.close().is_empty());
}

#[test]
fn suppressed_load_retries_within_one_tick_and_renders_real_metadata() {
    let _guard = METADATA.lock().unwrap();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rahmen-exiv2/testdata/tagged.jpg");
    let mut graph = Graph::new(
        vec![PathBuf::from("does-not-exist.jpg"), fixture],
        false,
        false,
    );
    graph.configure();
    graph.input.send(Configuration::Tick);
    let frames = graph.advance(1);
    assert_eq!(
        graph.calls.get(),
        2,
        "one outer tick must retry the suppressed load"
    );
    assert_eq!(
        frames.len(),
        2,
        "suppressed errors stay inside the feedback loop"
    );
    assert!(
        frames
            .iter()
            .all(|(t, r)| *t == Duration::ZERO && r.is_ok())
    );
    assert_eq!(
        (image(&frames, 1).1, image(&frames, 1).2.dimensions()),
        (Vector::new(17, 0), (62, 62))
    );
    assert!(
        image(&frames, 2)
            .2
            .to_rgb8()
            .pixels()
            .any(|p| p.0 != [0, 0, 0]),
        "fixture metadata must reach the real status formatter"
    );
    assert!(graph.close().is_empty());
}

fn termination(error: bool) {
    let mut graph = Graph::new(vec![], error, false);
    graph.input.send(Configuration::Tick);
    let frames = graph.advance(1);
    assert_eq!(frames, vec![(Duration::ZERO, Err(RunControl::Terminate))]);
    assert_eq!(graph.calls.get(), 1);
    assert!(!graph.probe.less_than(&Duration::from_millis(1)));
    assert!(graph.close().is_empty());
}

#[test]
fn exhausted_provider_terminates_and_progresses() {
    termination(false);
}
#[test]
fn provider_error_terminates_and_progresses() {
    termination(true);
}

#[test]
fn periodic_clock_emits_later_status_and_releases_notifications_on_close() {
    let mut graph = Graph::new(vec![], false, true);
    graph.configure();
    graph.input.send(Configuration::Greeting("HELLO".into()));
    let first = graph.advance(1);
    assert_eq!(first.len(), 1);
    let first_pixels = image(&first, 2).2.to_rgb8();
    let later = graph.advance(2000);
    assert!(
        !later.is_empty(),
        "clock must emit without new status input"
    );
    assert!(
        later
            .iter()
            .all(|(t, r)| *t > Duration::ZERO && matches!(r, Ok(Render::Image(2, _, _))))
    );
    for (_, render) in later {
        if let Ok(Render::Image(_, _, img)) = render {
            assert_eq!(img.to_rgb8(), first_pixels);
        }
    }
    graph
        .input
        .send(Configuration::Greeting("NEW STATUS".into()));
    let changed = graph.advance(2001);
    assert_eq!(changed.len(), 1);
    let changed_pixels = image(&changed, 2).2.to_rgb8();
    assert_ne!(changed_pixels, first_pixels);
    let recurring = graph.advance(4000);
    assert!(
        !recurring.is_empty(),
        "clock must continue after a status update"
    );
    for (_, render) in recurring {
        if let Ok(Render::Image(_, _, img)) = render {
            assert_eq!(img.to_rgb8(), changed_pixels);
        } else {
            panic!("unexpected clock output: {render:?}");
        }
    }
    let final_frames = graph.close();
    assert!(
        final_frames
            .iter()
            .all(|(t, r)| *t >= Duration::from_secs(4) && matches!(r, Ok(Render::Image(2, _, _))))
    );
}
