use super::*;
use rahmen::config::Element;
use rahmen::errors::{RahmenError, RahmenResult};
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
    StatusLineFormatter::new(std::iter::once(Element {
        exif_tags: vec!["Iptc.Application2.City".into()],
        case_conversion: None, capitalize: None, replace: None,
    }), None, ", ".into()).unwrap()
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rahmen-exiv2/testdata/tagged.jpg")
}
fn controlled() -> (Prefetch, mpsc::Sender<RahmenResult<Option<PathBuf>>>, mpsc::Receiver<()>, Arc<AtomicUsize>) {
    let (tx, paths) = mpsc::channel();
    let (entered, rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let loader = Loader::new(Box::new(Controlled { calls: calls.clone(), entered, paths }), formatter(), 16_000_000);
    (Prefetch::new(loader, Duration::from_secs(10)), tx, rx, calls)
}
fn wait_ready(p: &mut Prefetch) {
    let end = Instant::now() + Duration::from_secs(5);
    while p.loading() {
        p.poll();
        assert!(Instant::now() < end, "worker failed to return");
        std::thread::yield_now();
    }
}

#[test]
fn eager_prefetch_has_one_slot_and_preserves_real_image_metadata() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(p.loading());
    tx.send(Ok(Some(PathBuf::from("does-not-exist.jpg")))).unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "failed decode must retry");
    assert!(p.take_due(Duration::from_millis(999)).is_none());
    for _ in 0..100 { p.poll(); }
    assert_eq!(calls.load(Ordering::SeqCst), 2, "ready slot must stop provider advancement");
    let Outcome::Image(prepared) = p.take_due(Duration::from_secs(1)).unwrap() else { panic!("missing decoded image") };
    assert_eq!((prepared.image.width(), prepared.image.height()), (4, 4));
    assert_eq!(prepared.status.as_deref(), Some("Zurich"));
    assert!(p.take_due(Duration::from_secs(99)).is_none());
    p.presented(Duration::from_secs(3));
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(None)).unwrap();
    wait_ready(&mut p);
    assert!(p.take_due(Duration::from_secs(12)).is_none(), "delay starts after presentation");
    assert!(matches!(p.take_due(Duration::from_secs(13)), Some(Outcome::Terminate)));
}

#[test]
fn late_terminal_and_provider_error_wait_for_deadline() {
    for error in [false, true] {
        let (mut p, tx, entered, _) = controlled();
        p.presented(Duration::ZERO);
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(p.take_due(Duration::from_secs(2)).is_none());
        tx.send(if error { Err(RahmenError::Retry) } else { Ok(None) }).unwrap();
        wait_ready(&mut p);
        assert!(matches!(p.take_due(Duration::from_secs(2)), Some(Outcome::Terminate)));
    }
}

#[test]
fn close_during_blocked_provider_does_not_wait_or_retry() {
    let (mut p, tx, entered, calls) = controlled();
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let (closed_tx, closed_rx) = mpsc::channel();
    let thread = std::thread::spawn(move || { drop(p); closed_tx.send(()).unwrap(); });
    let closed = closed_rx.recv_timeout(Duration::from_secs(1)).is_ok();
    tx.send(Ok(Some(PathBuf::from("does-not-exist.jpg")))).unwrap();
    thread.join().unwrap();
    assert!(closed, "UI close waited for provider");
    assert!(entered.recv_timeout(Duration::from_secs(5)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn zero_delay_still_requires_presentation_before_next_load() {
    let _guard = METADATA.lock().unwrap();
    let (mut p, tx, entered, calls) = controlled();
    p.delay = Duration::ZERO;
    p.presented(Duration::ZERO);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(Some(fixture()))).unwrap();
    wait_ready(&mut p);
    assert!(matches!(p.take_due(Duration::from_secs(1)), Some(Outcome::Image(_))));
    p.poll();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    p.presented(Duration::from_secs(1));
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(Ok(None)).unwrap();
    wait_ready(&mut p);
    assert!(matches!(p.take_due(Duration::from_secs(1)), Some(Outcome::Terminate)));
}
