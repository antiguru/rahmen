use super::*;
use font_kit::{family_name::FamilyName, properties::Properties, source::SystemSource};
use image::{DynamicImage, Rgb, RgbImage};
use std::cell::RefCell;
use std::rc::Rc;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::InputHandle;
use timely::dataflow::operators::{Inspect, Probe};

type Input<D> = InputHandle<u64, CapacityContainerBuilder<Vec<D>>>;
type Frames = Rc<RefCell<Vec<(u64, usize, Vector, Arc<Picture>)>>>;

fn font() -> FontRenderer {
    FontRenderer::with_font(
        SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .unwrap()
            .load()
            .unwrap(),
    )
}

fn advance(
    worker: &mut timely::worker::Worker,
    probe: &timely::dataflow::ProbeHandle<u64>,
    time: u64,
) {
    for _ in 0..10_000 {
        if !probe.less_than(&time) {
            return;
        }
        worker.step();
    }
    panic!("frontier failed to advance to {time}");
}

fn resize_order(configuration_first: bool) {
    timely::execute_directly(move |worker| {
        let mut images = Input::new();
        let mut config = Input::new();
        let frames: Frames = Default::default();
        let captured = frames.clone();
        let probe = worker.dataflow(|scope| {
            images
                .to_stream(scope)
                .resize_image(&config.to_stream(scope), 7)
                .inspect_time(move |t, (key, pos, img)| {
                    captured.borrow_mut().push((*t, *key, *pos, img.clone()))
                })
                .probe()
                .0
        });
        let red: Arc<Picture> =
            Arc::new(DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([255, 0, 0]))).into());
        if configuration_first {
            config.send(Configuration::ScreenDimensions(8, 8));
            config.advance_to(1);
        } else {
            images.send(Arc::new(
                DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([0, 0, 255]))).into(),
            ));
            images.send(red.clone());
            images.advance_to(1);
        }
        for _ in 0..100 {
            worker.step();
        }
        assert!(
            frames.borrow().is_empty(),
            "both frontiers must pass timestamp zero"
        );
        assert!(probe.less_equal(&0));
        if configuration_first {
            images.send(Arc::new(
                DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([0, 0, 255]))).into(),
            ));
            images.send(red);
            images.advance_to(1);
        } else {
            config.send(Configuration::ScreenDimensions(8, 8));
            config.advance_to(1);
        }
        advance(worker, &probe, 1);
        {
            let frames = frames.borrow();
            assert_eq!(frames.len(), 1);
            let (time, key, pos, img) = &frames[0];
            assert_eq!(
                (*time, *key, *pos, img.dimensions()),
                (0, 7, Vector::new(0, 2), (8, 4))
            );
            assert!(img.to_rgb8().pixels().all(|p| *p == Rgb([255, 0, 0])));
        }
        config.send(Configuration::ScreenDimensions(4, 8));
        config.advance_to(2);
        images.advance_to(2);
        advance(worker, &probe, 2);
        assert_eq!(frames.borrow()[1].0, 1);
        assert_eq!(frames.borrow()[1].2, Vector::new(0, 3));
        assert_eq!(frames.borrow()[1].3.dimensions(), (4, 2));
        images.send(Arc::new(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 4, Rgb([0, 255, 0]))).into(),
        ));
        images.advance_to(3);
        config.advance_to(3);
        advance(worker, &probe, 3);
        assert_eq!(frames.borrow().len(), 3);
        assert_eq!(frames.borrow()[2].3.dimensions(), (4, 8));
        assert!(
            frames.borrow()[2]
                .3
                .to_rgb8()
                .pixels()
                .all(|p| *p == Rgb([0, 255, 0]))
        );
        images.close();
        config.close();
        for _ in 0..10_000 {
            if !worker.step() {
                assert!(probe.done());
                return;
            }
        }
        panic!("resize graph did not shut down");
    });
}

#[test]
fn resize_configuration_first() {
    resize_order(true);
}
#[test]
fn resize_image_first() {
    resize_order(false);
}

#[test]
fn resize_reuses_fitting_picture() {
    timely::execute_directly(move |worker| {
        let mut images = Input::new();
        let mut config = Input::new();
        let frames: Frames = Default::default();
        let captured = frames.clone();
        let probe = worker.dataflow(|scope| {
            images
                .to_stream(scope)
                .resize_image(&config.to_stream(scope), 7)
                .inspect_time(move |t, (key, pos, img)| {
                    captured.borrow_mut().push((*t, *key, *pos, img.clone()))
                })
                .probe()
                .0
        });
        let fitting: Arc<Picture> = Arc::new(DynamicImage::new_rgb8(8, 4).into());
        config.send(Configuration::ScreenDimensions(8, 8));
        images.send(fitting.clone());
        config.advance_to(1);
        images.advance_to(1);
        advance(worker, &probe, 1);
        let frames = frames.borrow();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].2, Vector::new(0, 2));
        assert!(Arc::ptr_eq(&frames[0].3, &fitting));
    });
}

fn text_order(configuration_first: bool) {
    timely::execute_directly(move |worker| {
        let mut text = Input::new();
        let mut config = Input::new();
        let frames: Frames = Default::default();
        let captured = frames.clone();
        let probe = worker.dataflow(|scope| {
            text.to_stream(scope)
                .format_text(&config.to_stream(scope), font(), 9)
                .inspect_time(move |t, (key, pos, img)| {
                    captured.borrow_mut().push((*t, *key, *pos, img.clone()))
                })
                .probe()
                .0
        });
        let send_config = |input: &mut Input<_>| {
            input.send(Configuration::FontSize(12.));
            input.send(Configuration::ScreenDimensions(96, 60));
        };
        if configuration_first {
            send_config(&mut config);
            config.advance_to(1);
        } else {
            text.send(vec!["DROP".into()]);
            text.send(vec!["TEST".into()]);
            text.advance_to(1);
        }
        for _ in 0..100 {
            worker.step();
        }
        assert!(frames.borrow().is_empty());
        assert!(probe.less_equal(&0));
        if configuration_first {
            text.send(vec!["DROP".into()]);
            text.send(vec!["TEST".into()]);
            text.advance_to(1);
        } else {
            send_config(&mut config);
            config.advance_to(1);
        }
        advance(worker, &probe, 1);
        assert!(frames.borrow().is_empty(), "vertical stretch is required");
        config.send(Configuration::FontCanvasVStretch(1.5));
        config.advance_to(2);
        text.advance_to(2);
        advance(worker, &probe, 2);
        {
            let frames = frames.borrow();
            assert_eq!(frames.len(), 1);
            assert_eq!(
                (
                    frames[0].0,
                    frames[0].1,
                    frames[0].2,
                    frames[0].3.dimensions()
                ),
                (1, 9, Vector::new(0, 42), (96, 18))
            );
            assert_eq!(
                frames[0].3.to_rgb8(),
                font()
                    .render(["TEST"].into_iter(), 12., (96, 18))
                    .unwrap()
                    .to_rgb8()
            );
            assert!(frames[0].3.to_rgb8().pixels().any(|p| p.0 != [0, 0, 0]));
            assert!(frames[0].3.to_rgb8().pixels().any(|p| p.0 == [0, 0, 0]));
        }
        config.send(Configuration::ScreenDimensions(120, 80));
        config.advance_to(3);
        text.advance_to(3);
        advance(worker, &probe, 3);
        assert_eq!(frames.borrow()[1].2, Vector::new(0, 62));
        assert_eq!(frames.borrow()[1].3.dimensions(), (120, 18));
        text.send(vec!["".into()]);
        config.advance_to(4);
        text.advance_to(4);
        advance(worker, &probe, 4);
        assert_eq!(frames.borrow().len(), 3);
        assert!(
            frames.borrow()[2]
                .3
                .to_rgb8()
                .pixels()
                .all(|p| p.0 == [0, 0, 0])
        );
        config.close();
        text.close();
        for _ in 0..10_000 {
            if !worker.step() {
                assert!(probe.done());
                return;
            }
        }
        panic!("text graph did not shut down");
    });
}

#[test]
fn text_configuration_first() {
    text_order(true);
}
#[test]
fn text_input_first() {
    text_order(false);
}
