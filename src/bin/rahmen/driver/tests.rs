use super::*;
use crate::dataflow::{self, Options};
use crate::prefetch::Loader;
use image::{DynamicImage, Rgb, RgbImage};
use rahmen::Vector;
use rahmen::font::FontRenderer;
use rahmen::picture::Picture;
use rahmen::provider::{Provider, StatusLineFormatter};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::Instant;
use timely::communication::Allocator;
use timely::worker::Config;

struct DisplayState {
    images: Vec<(usize, Picture)>,
    updates: usize,
    dimensions: (u32, u32),
    clock: Rc<Cell<Duration>>,
    update_cost: Duration,
    fail: bool,
}
impl Display for DisplayState {
    fn render(&mut self, key: usize, _: Vector, image: &Picture) -> RahmenResult<()> {
        self.images.push((key, image.clone()));
        Ok(())
    }
    fn blank(&mut self, _: usize, _: Vector, _: Vector) -> RahmenResult<()> {
        Ok(())
    }
    fn update(&mut self) -> RahmenResult<()> {
        self.updates += 1;
        self.clock.set(self.clock.get() + self.update_cost);
        if self.fail {
            Err(RahmenError::Terminate)
        } else {
            Ok(())
        }
    }
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
}

struct Blocked {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}
impl Provider<PathBuf> for Blocked {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        Ok(None)
    }
}

#[test]
fn blocked_provider_does_not_block_clock_progress() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (progress_tx, progress_rx) = mpsc::channel();
    let ui = std::thread::spawn(move || {
        let mut worker = Worker::new(
            Config::default(),
            Allocator::Thread(Default::default()),
            Some(Instant::now()),
        );
        let mut input = InputHandle::new();
        let mut updates = InputHandle::new();
        let probe = ProbeHandle::new();
        let output = dataflow::build(
            &mut worker,
            &mut input,
            &probe,
            &mut updates,
            FontRenderer::with_font(crate::font::load(None)),
            Options {
                loading_marker: "◷",
                show_time: true,
                time_format: "FIXED".into(),
            },
        );
        input.send(Configuration::FontSize(12.));
        input.send(Configuration::FontCanvasVStretch(1.5));
        input.send(Configuration::Greeting("HELLO".into()));
        input.send(Configuration::Splash(Arc::new(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([23, 45, 67]))).into(),
        )));
        let loader = Loader::new(
            &mut worker,
            Box::new(Blocked {
                entered: entered_tx,
                release: release_rx,
            }),
            StatusLineFormatter::new(std::iter::empty(), None, ", ".into()).unwrap(),
            16_000_000,
        );
        let clock = Rc::new(Cell::new(Duration::ZERO));
        let mut display = DisplayState {
            images: vec![],
            updates: 0,
            dimensions: (240, 80),
            clock: clock.clone(),
            update_cost: Duration::ZERO,
            fail: false,
        };
        let c = clock.clone();
        let mut drive = display_loop(
            &mut worker,
            &mut input,
            &mut updates,
            &probe,
            output,
            Prefetch::new(loader, Duration::from_secs(10)),
            move || c.get(),
        );
        drive(&mut display).unwrap();
        let first_status = display
            .images
            .iter()
            .find(|(key, _)| *key == 2)
            .unwrap()
            .1
            .clone();
        display.images.clear();
        clock.set(Duration::from_millis(1));
        drive(&mut display).unwrap();
        let loading_status = display
            .images
            .iter()
            .find(|(key, _)| *key == 2)
            .unwrap()
            .1
            .clone();
        assert_ne!(first_status, loading_status);
        display.images.clear();
        clock.set(Duration::from_secs(2));
        drive(&mut display).unwrap();
        assert!(display.images.iter().any(|(key, _)| *key == 2));
        assert!(
            display.images.iter().all(|(key, _)| *key != 1),
            "clock redraw must retain image"
        );
        progress_tx.send(()).unwrap();
        // Closing both graph inputs releases clock notifications even with blocked I/O.
        drop(drive);
        input.close();
        updates.close();
        for _ in 0..10000 {
            if !worker.step() {
                return;
            }
        }
        panic!("graph shutdown stalled");
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let progressed = progress_rx.recv_timeout(Duration::from_secs(1)).is_ok();
    release_tx.send(()).unwrap();
    ui.join().unwrap();
    assert!(progressed, "UI clock frontier stalled on provider I/O");
}

struct Paths {
    entered: mpsc::Sender<()>,
    paths: mpsc::Receiver<Option<PathBuf>>,
}
impl Provider<PathBuf> for Paths {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        self.entered.send(()).unwrap();
        Ok(self.paths.recv().unwrap())
    }
}

type Drive<'a> = dyn FnMut(&mut dyn Display) -> RahmenResult<()> + 'a;
fn with_driver(
    provider: Box<dyn Provider<PathBuf> + Send>,
    run: impl FnOnce(&mut Drive<'_>, &mut DisplayState),
) {
    let mut worker = Worker::new(
        Config::default(),
        Allocator::Thread(Default::default()),
        Some(Instant::now()),
    );
    let mut input = InputHandle::new();
    let mut updates = InputHandle::new();
    let probe = ProbeHandle::new();
    let output = dataflow::build(
        &mut worker,
        &mut input,
        &probe,
        &mut updates,
        FontRenderer::with_font(crate::font::load(None)),
        Options {
            loading_marker: "◷",
            show_time: true,
            time_format: "FIXED".into(),
        },
    );
    input.send(Configuration::FontSize(12.));
    input.send(Configuration::FontCanvasVStretch(1.5));
    input.send(Configuration::Greeting("HELLO".into()));
    input.send(Configuration::Splash(Arc::new(
        DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([23, 45, 67]))).into(),
    )));
    let loader = Loader::new(
        &mut worker,
        provider,
        StatusLineFormatter::new(std::iter::empty(), None, ", ".into()).unwrap(),
        16_000_000,
    );
    let clock = Rc::new(Cell::new(Duration::ZERO));
    let mut display = DisplayState {
        images: vec![],
        updates: 0,
        dimensions: (240, 80),
        clock: clock.clone(),
        update_cost: Duration::ZERO,
        fail: false,
    };
    let c = clock.clone();
    let mut drive = display_loop(
        &mut worker,
        &mut input,
        &mut updates,
        &probe,
        output,
        Prefetch::new(loader, Duration::from_secs(10)),
        move || c.get(),
    );
    run(&mut drive, &mut display);
    drop(drive);
    input.close();
    updates.close();
    for _ in 0..10000 {
        if !worker.step() {
            return;
        }
    }
    panic!("graph shutdown stalled");
}

fn wait_until(
    drive: &mut Drive<'_>,
    display: &mut DisplayState,
    condition: impl Fn(&DisplayState) -> bool,
) {
    let end = Instant::now() + Duration::from_secs(5);
    while !condition(display) {
        drive(display).unwrap();
        assert!(Instant::now() < end, "UI condition did not become true");
        std::thread::yield_now();
    }
}

#[test]
fn early_image_waits_and_final_interval_starts_after_display_update() {
    let _guard = crate::prefetch::tests::METADATA.lock().unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (paths_tx, paths_rx) = mpsc::channel();
    with_driver(
        Box::new(Paths {
            entered: entered_tx,
            paths: paths_rx,
        }),
        |drive, display| {
            drive(display).unwrap();
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let splash = display
                .images
                .iter()
                .find(|(key, _)| *key == 1)
                .unwrap()
                .1
                .clone();
            let idle_status = display
                .images
                .iter()
                .find(|(key, _)| *key == 2)
                .unwrap()
                .1
                .clone();
            display.images.clear();
            display.clock.set(Duration::from_millis(1));
            drive(display).unwrap();
            assert_ne!(display.images.last().unwrap().1, idle_status);
            paths_tx
                .send(Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bin/rahmen/rahmen.png"),
                ))
                .unwrap();
            display.clock.set(Duration::from_millis(500));
            wait_until(drive, display, |d| {
                d.images
                    .last()
                    .is_some_and(|(key, image)| *key == 2 && image == &idle_status)
            });
            assert!(
                display.images.iter().all(|(key, _)| *key != 1),
                "early result replaced splash"
            );
            display.images.clear();
            display.clock.set(Duration::from_secs(1));
            display.update_cost = Duration::from_secs(2);
            drive(display).unwrap();
            assert_ne!(
                display.images.iter().find(|(key, _)| *key == 1).unwrap().1,
                splash
            );
            assert_eq!(display.clock.get(), Duration::from_secs(3));
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            paths_tx.send(None).unwrap();
            display.update_cost = Duration::ZERO;
            // Observe EOF readiness through the disappearing marker, without advancing the deadline.
            display.clock.set(Duration::from_secs(4));
            let mut renderer = FontRenderer::with_font(crate::font::load(None));
            let ready_status = renderer
                .render(["[FIXED] "].into_iter(), 12., (240, 18))
                .unwrap();
            wait_until(drive, display, |d| {
                d.images.last().is_some_and(|(key, image)| {
                    *key == 2 && *image == Picture::from(ready_status.clone())
                })
            });
            display.images.clear();
            display.clock.set(Duration::from_secs(12));
            drive(display).unwrap();
            assert!(display.images.iter().all(|(key, _)| *key != 1));
            display.dimensions = (200, 80);
            drive(display).unwrap();
            display.clock.set(Duration::from_secs(13));
            assert!(
                matches!(drive(display), Err(RahmenError::Terminate)),
                "clock/resize renders must not reset slideshow deadline"
            );
        },
    );
}

#[test]
fn display_failure_does_not_start_prefetch() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (_paths_tx, paths_rx) = mpsc::channel();
    with_driver(
        Box::new(Paths {
            entered: entered_tx,
            paths: paths_rx,
        }),
        |drive, display| {
            display.fail = true;
            assert!(matches!(drive(display), Err(RahmenError::Terminate)));
        },
    );
    assert!(entered_rx.recv_timeout(Duration::from_secs(5)).is_err());
}

#[test]
fn zero_area_viewport_can_finish_eof() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (paths_tx, paths_rx) = mpsc::channel();
    with_driver(
        Box::new(Paths {
            entered: entered_tx,
            paths: paths_rx,
        }),
        |drive, display| {
            display.dimensions = (0, 0);
            drive(display).unwrap();
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            paths_tx.send(None).unwrap();
            display.clock.set(Duration::from_secs(2));
            let end = Instant::now() + Duration::from_secs(5);
            while drive(display).is_ok() {
                assert!(Instant::now() < end);
                std::thread::yield_now();
            }
            assert!(display.images.is_empty());
        },
    );
}
