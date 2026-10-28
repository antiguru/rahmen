use super::*;
use rahmen::config::Element;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::picture::Picture;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::time::Instant;

// Exiv2 initializes native process-wide metadata state. Keep fixture access serial
// within this binary while the other headless graph tests remain parallel.
pub(crate) static METADATA: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Controlled {
    calls: Arc<AtomicUsize>,
    entered: mpsc::Sender<()>,
    paths: mpsc::Receiver<RahmenResult<Option<PathBuf>>>,
}
impl Provider<PathBuf> for Controlled {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.send(()).unwrap();
        self.paths.recv().unwrap()
    }
}

pub(super) fn formatter() -> StatusLineFormatter {
    StatusLineFormatter::new(
        std::iter::once(Element {
            exif_tags: vec!["Iptc.Application2.City".into()],
            case_conversion: None,
            capitalize: None,
            replace: None,
        }),
        None,
        ", ".into(),
    )
    .unwrap()
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rahmen-exiv2/testdata/tagged.jpg")
}
type ControlledPrefetch = (
    Prefetch,
    Worker,
    mpsc::Sender<RahmenResult<Option<PathBuf>>>,
    mpsc::Receiver<()>,
    Arc<AtomicUsize>,
);

fn worker() -> Worker {
    Worker::new(
        timely::worker::Config::default(),
        timely::communication::Allocator::Thread(Default::default()),
        Some(Instant::now()),
    )
}

fn controlled() -> ControlledPrefetch {
    controlled_on(worker())
}

fn controlled_on(worker: Worker) -> ControlledPrefetch {
    controlled_with(worker, 0)
}

fn controlled_with(mut worker: Worker, reserved_height: u32) -> ControlledPrefetch {
    let (tx, paths) = mpsc::channel();
    let (entered, rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let loader = Loader::new(
        &mut worker,
        Box::new(Controlled {
            calls: calls.clone(),
            entered,
            paths,
        }),
        formatter(),
        16_000_000,
        reserved_height,
    );
    (
        Prefetch::new(loader, Duration::from_secs(10)),
        worker,
        tx,
        rx,
        calls,
    )
}
fn wait_ready(p: &mut Prefetch, worker: &mut Worker) {
    let end = Instant::now() + Duration::from_secs(5);
    while p.loading() {
        worker.step();
        assert!(Instant::now() < end, "worker failed to return");
        std::thread::yield_now();
    }
}

#[test]
fn eager_prefetch_has_one_slot_and_preserves_real_image_metadata() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, mut worker, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(p.loading());
    tx.send(Ok(Some(PathBuf::from("does-not-exist.jpg"))))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p, &mut worker);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "failed decode must retry");
    assert!(p.take_due(Duration::from_millis(999)).is_none());
    for _ in 0..100 {
        worker.step();
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "ready slot must stop provider advancement"
    );
    let Outcome::Image(prepared) = p.take_due(Duration::from_secs(1)).unwrap() else {
        panic!("missing decoded image")
    };
    assert_eq!(prepared.image.dimensions(), (4, 4));
    assert_eq!(prepared.status.as_deref(), Some("Zurich"));
    assert!(p.take_due(Duration::from_secs(99)).is_none());
    p.presented(Duration::from_secs(3));
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(None)).unwrap();
    wait_ready(&mut p, &mut worker);
    assert!(
        p.take_due(Duration::from_secs(12)).is_none(),
        "delay starts after presentation"
    );
    assert!(matches!(
        p.take_due(Duration::from_secs(13)),
        Some(Outcome::Terminate)
    ));
}

#[test]
fn loads_prepare_pictures_for_the_display() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, mut worker, tx, entered, _) = controlled_with(worker(), 2);
    let field = |offset| framebuffer::Bitfield {
        offset,
        length: 8,
        msb_right: 0,
    };
    let bgra = PixelFormat::from_var_screen_info(&framebuffer::VarScreeninfo {
        bits_per_pixel: 32,
        red: field(16),
        green: field(8),
        blue: field(0),
        transp: field(24),
        ..Default::default()
    })
    .unwrap();
    // The image area excludes the status line's 2 rows.
    p.set_display((2, 4), Some(bgra));
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p, &mut worker);
    let Outcome::Image(prepared) = p.take_due(Duration::from_secs(1)).unwrap() else {
        panic!("missing decoded image")
    };
    assert!(matches!(*prepared.image, Picture::Native(format, _) if format == bgra));
    assert_eq!(prepared.image.dimensions(), (2, 2));
}

#[test]
fn late_terminal_and_provider_error_wait_for_deadline() {
    for error in [false, true] {
        let (mut p, mut worker, tx, entered, _) = controlled();
        p.presented(Duration::ZERO);
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(p.take_due(Duration::from_secs(2)).is_none());
        tx.send(if error {
            Err(RahmenError::Retry)
        } else {
            Ok(None)
        })
        .unwrap();
        wait_ready(&mut p, &mut worker);
        assert!(matches!(
            p.take_due(Duration::from_secs(2)),
            Some(Outcome::Terminate)
        ));
    }
}

#[test]
fn close_during_blocked_provider_does_not_wait_or_retry() {
    let (mut p, mut worker, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let (closed_tx, closed_rx) = mpsc::channel();
    let release = std::thread::spawn(move || {
        let closed = closed_rx.recv_timeout(Duration::from_secs(1)).is_ok();
        tx.send(Ok(Some(PathBuf::from("does-not-exist.jpg"))))
            .unwrap();
        closed
    });
    drop(p);
    close_consumer(&mut worker);
    let _ = closed_tx.send(());
    let closed = release.join().unwrap();
    assert!(closed, "UI close waited for provider");
    assert!(entered.recv_timeout(Duration::from_secs(5)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn zero_delay_still_requires_presentation_before_next_load() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, mut worker, tx, entered, calls) = controlled();
    p.delay = Duration::ZERO;
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p, &mut worker);
    assert!(matches!(
        p.take_due(Duration::from_secs(1)),
        Some(Outcome::Image(_))
    ));
    worker.step();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    p.presented(Duration::from_secs(1));
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(None)).unwrap();
    wait_ready(&mut p, &mut worker);
    assert!(matches!(
        p.take_due(Duration::from_secs(1)),
        Some(Outcome::Terminate)
    ));
}

#[test]
fn unexpected_worker_panic_is_a_terminal_outcome() {
    struct Panics;
    impl Provider<PathBuf> for Panics {
        fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
            panic!("controlled provider panic")
        }
    }
    let mut worker = worker();
    let mut p = Prefetch::new(
        Loader::new(&mut worker, Box::new(Panics), formatter(), 16_000_000, 0),
        Duration::ZERO,
    );
    // Drain the initial activation so only the unwind notification can expose disconnect.
    for _ in 0..10 {
        worker.step();
    }
    p.presented(Duration::ZERO);
    wait_ready(&mut p, &mut worker);
    assert!(p.take_due(Duration::from_millis(999)).is_none());
    assert!(matches!(
        p.take_due(Duration::from_secs(1)),
        Some(Outcome::Terminate)
    ));
}

#[test]
fn close_with_ready_result_releases_worker() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, mut worker, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p, &mut worker);
    drop(p);
    close_consumer(&mut worker);
    assert!(entered.recv_timeout(Duration::from_secs(5)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn close_during_decoder_read_stops_before_retry() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("rahmen-prefetch-decode-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("blocked.png");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let (mut p, mut worker, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(path.clone()))).unwrap();
    let (opened_tx, opened_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        // Opening the writer pairs with the production decoder's open, then holds its read.
        let mut fifo = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        opened_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        let _ = fifo.write_all(b"invalid PNG");
    });
    opened_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (closed_tx, closed_rx) = mpsc::channel();
    let release = std::thread::spawn(move || {
        let closed = closed_rx.recv_timeout(Duration::from_secs(1)).is_ok();
        release_tx.send(()).unwrap();
        closed
    });
    drop(p);
    close_consumer(&mut worker);
    let _ = closed_tx.send(());
    let closed = release.join().unwrap();
    writer.join().unwrap();
    assert!(closed, "close joined decoder I/O");
    assert!(entered.recv_timeout(Duration::from_secs(5)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn late_image_is_available_immediately_and_metadata_failure_is_not_terminal() {
    let _guard = METADATA.lock().unwrap();
    use std::io::{Cursor, Write};
    let path = std::env::temp_dir().join(format!("rahmen-prefetch-{}.png", std::process::id()));
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let mut encoded = Cursor::new(Vec::new());
    image::RgbImage::new(2, 2)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let (mut p, mut worker, tx, entered, _) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(p.take_due(Duration::from_secs(20)).is_none());
    tx.send(Ok(Some(path.clone()))).unwrap();
    let writer = std::thread::spawn(move || {
        let mut fifo = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        // The decoder has its file descriptor, but metadata must open the path again.
        std::fs::remove_file(path).unwrap();
        fifo.write_all(encoded.get_ref()).unwrap();
    });
    wait_ready(&mut p, &mut worker);
    writer.join().unwrap();
    let Some(Outcome::Image(prepared)) = p.take_due(Duration::from_secs(20)) else {
        panic!("late result must be due")
    };
    assert_eq!(prepared.image.dimensions(), (2, 2));
    assert!(
        prepared.status.is_none(),
        "unsupported metadata must retain previous status"
    );
    drop(p);
    close_consumer(&mut worker);
    assert!(entered.recv_timeout(Duration::from_secs(5)).is_err());
}

#[test]
fn python_postprocessor_created_on_ui_thread_runs_on_worker() {
    use pyo3::prelude::*;
    use pyo3::types::PyModule;
    let _guard = METADATA.lock().unwrap();
    Python::attach(|py| {
        let module = PyModule::from_code(
            py,
            c"def export():\n    return lambda values, separator: ['worker:' + values[0]]\n",
            c"prefetch_test.py",
            c"_rahmen_prefetch_test",
        )
        .unwrap();
        py.import("sys")
            .unwrap()
            .getattr("modules")
            .unwrap()
            .set_item("_rahmen_prefetch_test", module)
            .unwrap();
    });
    let formatter = StatusLineFormatter::new(
        std::iter::once(Element {
            exif_tags: vec!["Iptc.Application2.City".into()],
            case_conversion: None,
            capitalize: None,
            replace: None,
        }),
        Some("_rahmen_prefetch_test".into()),
        ", ".into(),
    )
    .unwrap();
    let (tx, paths) = mpsc::channel();
    let (entered, rx) = mpsc::channel();
    let mut worker = worker();
    let loader = Loader::new(
        &mut worker,
        Box::new(Controlled {
            calls: Arc::new(AtomicUsize::new(0)),
            entered,
            paths,
        }),
        formatter,
        16_000_000,
        0,
    );
    let mut p = Prefetch::new(loader, Duration::ZERO);
    p.presented(Duration::ZERO);
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p, &mut worker);
    let Some(Outcome::Image(prepared)) = p.take_due(Duration::from_secs(1)) else {
        panic!("worker failed Python processing")
    };
    assert_eq!(prepared.status.as_deref(), Some("worker:Zurich"));
    drop(p);
    close_consumer(&mut worker);
    assert!(rx.recv_timeout(Duration::from_secs(5)).is_err());
    Python::attach(|py| {
        py.import("sys")
            .unwrap()
            .getattr("modules")
            .unwrap()
            .del_item("_rahmen_prefetch_test")
            .unwrap();
    });
}

fn close_consumer(worker: &mut Worker) {
    for _ in 0..100 {
        if !worker.step() {
            return;
        }
    }
    panic!("result consumer did not close");
}

#[test]
fn completion_wakes_parked_consumer_without_ui_polling() {
    use timely::logging::{ParkEvent, StartStop, TimelyEvent, TimelyEventBuilder};
    let worker = worker();
    let (parked_tx, parked_rx) = mpsc::channel();
    let events = Rc::new(RefCell::new(Vec::new()));
    let logged = events.clone();
    worker
        .log_register()
        .unwrap()
        .insert::<TimelyEventBuilder, _>("timely", move |_, data| {
            if let Some(data) = data {
                for (_, event) in data.drain(..) {
                    if matches!(event, TimelyEvent::Park(ParkEvent::Park(_))) {
                        let _ = parked_tx.send(());
                    }
                    logged.borrow_mut().push(event);
                }
            }
        });
    let (mut p, mut worker, tx, entered, _) = controlled_on(worker);
    // Consume registration's initial activation before allowing the loader to finish.
    for _ in 0..10 {
        worker.step();
    }
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let release = std::thread::spawn(move || {
        parked_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        tx.send(Ok(None)).unwrap();
    });
    worker.step_or_park(Some(Duration::from_secs(1)));
    release.join().unwrap();
    for _ in 0..10 {
        worker.step();
    }
    assert!(
        !p.loading(),
        "completion did not activate the parked result consumer"
    );
    let scheduled = events
        .borrow()
        .iter()
        .filter(
            |event| matches!(event, TimelyEvent::Schedule(s) if s.start_stop == StartStop::Start),
        )
        .count();
    assert!(scheduled > 0, "scheduler logging was not enabled");
    for _ in 0..100 {
        worker.step();
    }
    assert_eq!(events.borrow().iter().filter(|event| matches!(event, TimelyEvent::Schedule(s) if s.start_stop == StartStop::Start)).count(), scheduled, "idle steps rescheduled the result consumer");
    assert!(p.take_due(Duration::from_millis(999)).is_none());
    assert!(matches!(
        p.take_due(Duration::from_secs(1)),
        Some(Outcome::Terminate)
    ));
    drop(p);
    close_consumer(&mut worker);
}

#[test]
fn completion_before_first_consumer_activation_is_not_lost() {
    let mut worker = worker();
    let (sender, results) = mpsc::sync_channel(1);
    let slot = Rc::new(RefCell::new(Slot::Loading));
    let (consumer, activator) = register_consumer(&mut worker, results, Rc::clone(&slot));
    let sender = ResultSender {
        sender: Some(sender),
        activator,
    };
    assert!(sender.send(Outcome::Terminate));
    for _ in 0..10 {
        worker.step();
    }
    assert!(matches!(*slot.borrow(), Slot::Ready(Outcome::Terminate)));
    *slot.borrow_mut() = Slot::Stopped;
    consumer.activate();
    close_consumer(&mut worker);
}
