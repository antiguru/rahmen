extern crate clap;
extern crate ctrlc;
extern crate exif;
extern crate timely;

use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::{App, Arg};
use timely::dataflow::operators::capture::Event;
use timely::dataflow::operators::{
    Branch, Capability, CapabilityRef, Capture, Concat, ConnectLoop, Enter, Inspect, Leave,
    LoopVariable, Map, Operator, Probe, ResultStream,
};
use timely::dataflow::{InputHandle, ProbeHandle, Scope};
use timely::order::Product;
use timely::progress::Timestamp;

use image::{DynamicImage, GenericImage, GenericImageView, Pixel};
use rahmen::display::{preprocess_image, Display};
#[cfg(feature = "fltk")]
use rahmen::display_fltk::FltkDisplay;
use rahmen::display_framebuffer::FramebufferDisplay;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::font::FontRenderer;
use rahmen::provider::{
    coordinates_from_exif, coordinates_to_location, load_image_from_path, read_exif_from_path,
    Provider,
};
use rahmen::provider_list::ListProvider;
use rahmen::Timer;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Copy, Clone, Eq, PartialEq)]
enum RunControl {
    Terminate,
    Suppressed,
}

fn fatal_err<T>(result: RahmenResult<Option<T>>) -> RunResult<T> {
    match result {
        Ok(None) => Err(RunControl::Terminate),
        Ok(Some(t)) => Ok(t),
        Err(e) => {
            eprintln!("Encountered error, terminating: {}", e);
            Err(RunControl::Terminate)
        }
    }
}

fn suppress_err<T>(result: RahmenResult<T>) -> RunResult<T> {
    result.map_err(|e| {
        eprintln!("Encountered error, suppressing: {}", e);
        RunControl::Suppressed
    })
}

type RunResult<T> = Result<T, RunControl>;

fn main() -> RahmenResult<()> {
    let matches = App::new("Rahmen client")
        .arg(
            Arg::new("display")
                .short('d')
                .long("display")
                .about("Select the display provider")
                .value_name("display")
                .takes_value(true)
                .possible_values(&[
                    #[cfg(feature = "fltk")]
                    "fltk",
                    "framebuffer",
                ])
                .default_value("framebuffer"),
        )
        .arg(Arg::new("input").takes_value(true).required(true).index(1))
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .takes_value(true),
        )
        .arg(
            Arg::new("time")
                .short('t')
                .long("time")
                .takes_value(true)
                .validator(|v| f64::from_str(v))
                .default_value("90"),
        )
        .arg(
            Arg::new("buffer_max_size")
                .long("buffer_max_size")
                .takes_value(true)
                .validator(|v| f64::from_str(v))
                .default_value(format!("{}", 4000 * 4000).as_str()),
        )
        .get_matches();

    let input = matches.value_of("input").expect("Input missing");
    let mut provider: Box<dyn Provider<_>> = if input.eq("-") {
        println!("Reading from stdin");
        Box::new(ListProvider::new(BufReader::new(std::io::stdin())))
    } else if let Ok(file) = File::open(input) {
        println!("Reading from file");
        Box::new(ListProvider::new(BufReader::new(file)))
    } else {
        println!("Reading from pattern {}", input);
        Box::new(rahmen::provider_glob::create(input)?)
    };

    let allocator = timely::communication::allocator::Thread::new();
    let mut worker = timely::worker::Worker::new(allocator);

    let mut input: InputHandle<_, ()> = InputHandle::new();
    let mut input_dimensions: InputHandle<_, (u32, u32)> = InputHandle::new();
    let mut probe = ProbeHandle::new();

    let buffer_max_size: usize = matches
        .value_of("buffer_max_size")
        .expect("Missing buffer_max_size")
        .parse()
        .unwrap();

    let output = worker.dataflow(|scope| {
        let font_size = 30.;

        let _last_time: Option<Instant> = None;
        let time_str = matches.value_of("time").unwrap();
        let delay = Duration::from_millis((f64::from_str(time_str).unwrap() * 1000f64) as u64);
        println!("Delay: {:?}", delay);
        let stream = input.to_stream(scope).unary_frontier(
            timely::dataflow::channels::pact::Pipeline,
            "Ticker",
            |cap: Capability<Duration>, _op| {
                let mut buffer = vec![];
                let mut retained_cap: Option<Capability<Duration>> = Some(cap);
                move |input_handle, output_handle| {
                    if input_handle.frontier.is_empty() {
                        retained_cap.take();
                    } else if let Some(retained_cap) = retained_cap.as_mut() {
                        if !input_handle
                            .frontier
                            .frontier()
                            .less_equal(retained_cap.time())
                        {
                            output_handle.session(&retained_cap).give(());
                            retained_cap.downgrade(&(*retained_cap.time() + delay));
                            while !input_handle
                                .frontier
                                .frontier()
                                .less_equal(retained_cap.time())
                            {
                                retained_cap
                                    .downgrade(&(*retained_cap.time() + Duration::from_secs(1)));
                            }
                            println!("retained time: {:?}", retained_cap.time());
                        }
                    }
                    while let Some((cap, in_buffer)) = input_handle.next() {
                        in_buffer.swap(&mut buffer);
                        output_handle.session(&cap).give_vec(&mut buffer);
                    }
                }
            },
        );
        let dimensions_stream = input_dimensions.to_stream(scope);
        let img_path_stream = scope.scoped::<Product<_, u32>, _, _>("File loading", |inner| {
            let (handle, cycle) = inner.loop_variable(1);
            let (ok, err) = stream
                .enter(inner)
                .concat(&cycle)
                // obtain next path
                .map(move |_| fatal_err(provider.next_image()))
                // Load image
                .and_then(move |ref path| {
                    suppress_err(
                        load_image_from_path(path, Some(buffer_max_size))
                            .map(|img| (path.clone(), Arc::new(img))),
                    )
                })
                .branch(|_t, d| d.as_ref().err() == Some(&RunControl::Suppressed));
            err.map(|_| ()).connect_loop(handle);
            ok.leave()
        });
        let err_stream = img_path_stream.err();

        let exif_stream = img_path_stream
            .ok()
            .flat_map(|(p, _img)| read_exif_from_path(&p).ok())
            // .inspect(|x| println!("exif: {:?}", x))
            .probe_with(&mut probe);

        let location_stream = exif_stream
            .map(|exif| coordinates_from_exif(exif.iter()))
            .map(|c| {
                c.map(|c| coordinates_to_location(c).unwrap_or(String::from("")))
                    .unwrap_or(String::from(""))
            })
            .inspect(|loc| println!("Location: {}", loc))
            .probe_with(&mut probe);

        let font_renderer = FontRenderer::new();

        let text_img_stream = {
            let mut dimensions = HashMap::new();
            let mut texts = HashMap::new();
            let mut current_dimension = None;
            let mut current_text = None;
            let mut in_buffer1 = vec![];
            let mut in_buffer2 = vec![];
            let font_size1 = font_size;
            location_stream.binary_notify(
                &dimensions_stream,
                timely::dataflow::channels::pact::Pipeline,
                timely::dataflow::channels::pact::Pipeline,
                "Render font",
                None,
                move |in1, in2, out, not| {
                    let _t = Timer::new(|e| println!("Render font op {}ms", e.as_millis()));
                    in1.for_each(|time, data| {
                        data.swap(&mut in_buffer1);
                        for text in in_buffer1.drain(..) {
                            texts.insert(*time.time(), text);
                        }
                        not.notify_at(time.retain());
                    });
                    in2.for_each(|time, data| {
                        data.swap(&mut in_buffer2);
                        for dimension in in_buffer2.drain(..) {
                            dimensions.insert(*time.time(), dimension);
                        }
                        not.notify_at(time.retain());
                    });
                    not.for_each(|time, _cnt, _not| {
                        if let Some(dimension) = dimensions.remove(time.time()) {
                            current_dimension = Some(dimension);
                        }
                        if let Some(text) = texts.remove(time.time()) {
                            current_text = Some(text);
                        }
                        if current_text.is_some() && current_dimension.is_some() {
                            let dimension = current_dimension.as_ref().unwrap();
                            // println!("Dimension: {:?}", dimension);
                            // println!("Text: {}", current_text.as_ref().unwrap());
                            let mut img = DynamicImage::new_bgra8(dimension.0, font_size as _);
                            font_renderer.render(
                                current_text.as_ref().unwrap(),
                                font_size,
                                (dimension.0, font_size as _),
                                |x, y, pixel| Ok(img.put_pixel(x as _, y as _, pixel.to_rgba())),
                            );
                            out.session(&time).give((*dimension, Arc::new(img)));
                        }
                    });
                },
            )
        };

        let img_stream = img_path_stream.ok().map(|(_, img)| img).binary(
            &dimensions_stream.map(move |(x, y)| (x, y - font_size as u32)),
            timely::dataflow::channels::pact::Pipeline,
            timely::dataflow::channels::pact::Pipeline,
            "Resize",
            |_cap, _op| {
                let _t = Timer::new(|e| println!("Resize op {}ms", e.as_millis()));
                let mut dimensions = None;
                let mut current_image = None;
                move |in1, in2, out| {
                    let mut did_work = false;
                    let mut cap: Option<Capability<_>> = None;
                    fn track_time<T: Timestamp>(
                        cap: &mut Option<Capability<T>>,
                        time: CapabilityRef<T>,
                    ) {
                        if let Some(cap) = cap.as_mut() {
                            if cap.time() < time.time() {
                                cap.downgrade(time.time());
                            }
                        } else {
                            cap.replace(time.retain());
                        }
                    }
                    in1.for_each(|time, data| {
                        if let Some(image) = data.last() {
                            current_image = Some(image.clone());
                            did_work |= true;
                        }
                        track_time(&mut cap, time);
                    });
                    in2.for_each(|time, data| {
                        if let Some(dims) = data.last() {
                            dimensions = Some(dims.clone());
                            did_work |= true;
                        }
                        track_time(&mut cap, time);
                    });
                    if did_work && dimensions.is_some() {
                        if let Some(current_image) = current_image.as_ref() {
                            out.session(cap.as_ref().unwrap())
                                .give(Arc::new(preprocess_image(
                                    &current_image,
                                    dimensions.unwrap().0,
                                    dimensions.unwrap().1,
                                )));
                        }
                    }
                }
            },
        );

        let composed_img_stream = {
            let mut imgs = HashMap::new();
            let mut texts = HashMap::new();
            let mut current_img = None;
            let mut current_text = None;
            let mut in_buffer1 = vec![];
            let mut in_buffer2 = vec![];
            let font_size1 = font_size;
            img_stream.binary_notify(
                &text_img_stream,
                timely::dataflow::channels::pact::Pipeline,
                timely::dataflow::channels::pact::Pipeline,
                "Compose",
                None,
                move |in1, in2, out, not| {
                    let _t = Timer::new(|e| println!("Compose op {}ms", e.as_millis()));
                    in1.for_each(|time, data| {
                        data.swap(&mut in_buffer1);
                        for img in in_buffer1.drain(..) {
                            imgs.insert(*time.time(), img);
                        }
                        not.notify_at(time.retain());
                    });
                    in2.for_each(|time, data| {
                        data.swap(&mut in_buffer2);
                        for text_dim in in_buffer2.drain(..) {
                            texts.insert(*time.time(), text_dim);
                        }
                        not.notify_at(time.retain());
                    });
                    not.for_each(|time, _cnt, _not| {
                        if let Some(img) = imgs.remove(time.time()) {
                            current_img = Some(img);
                        }
                        if let Some(text) = texts.remove(time.time()) {
                            current_text = Some(text);
                        }
                        if current_text.is_some() && current_img.is_some() {
                            let (dimension, text_img) = current_text.as_ref().unwrap();
                            let current_img = current_img.as_ref().unwrap();
                            let mut img = DynamicImage::new_bgra8(dimension.0, dimension.1);
                            let x_offset = (dimension.0 - current_img.dimensions().0) / 2;
                            let y_offset = (dimension.1 - current_img.dimensions().1) / 2;
                            // println!(
                            //     "Dimension: {:?} offset: ({}, {})",
                            //     dimension, x_offset, y_offset
                            // );
                            img.copy_from(current_img.as_ref(), x_offset, y_offset);
                            img.copy_from(text_img.as_ref(), 0, dimension.1 - font_size as u32);
                            out.session(&time).give(Arc::new(img));
                        }
                    });
                },
            )
        };

        err_stream
            .map(Err)
            .concat(&composed_img_stream.map(Ok))
            .probe_with(&mut probe)
            .capture()
    });

    let start_time = Instant::now();
    let mut dimensions = None;

    let display_fn = |display: Box<&mut dyn Display>| {
        let now = start_time.elapsed();
        input.advance_to(now);
        if Some(display.dimensions()) != dimensions {
            dimensions = Some(display.dimensions());
            input_dimensions.send(display.dimensions());
        }
        input_dimensions.advance_to(now);
        while probe.less_than(input.time()) {
            worker.step();
        }
        match output.try_iter().all(|result| match result {
            Event::Progress(_) => true,
            Event::Messages(_, ref r) => {
                r.iter()
                    .filter(|r| r.is_ok())
                    .last()
                    .map(|img| img.as_ref().map(|img| display.render(img)));
                !r.iter()
                    .any(|r| r.as_ref().err() == Some(&RunControl::Terminate))
            }
        }) {
            true => Ok(()),
            false => Err(RahmenError::Terminate),
        }
    };

    match matches.value_of("display").expect("Display missing") {
        "framebuffer" => {
            let path_to_device = matches
                .value_of("output")
                .expect("Framebuffer output missing");
            let framebuffer = framebuffer::Framebuffer::new(path_to_device).unwrap();
            let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Graphics)
                .map_err(|_e| println!("Failed to set graphics mode."));
            ctrlc::set_handler(|| {
                let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Text)
                    .map_err(|_e| println!("Failed to set graphics mode."));
                std::process::exit(0);
            })
            .unwrap();
            FramebufferDisplay::new(framebuffer).main_loop(display_fn);
            let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Text)
                .map_err(|_e| println!("Failed to set graphics mode."));
        }
        #[cfg(feature = "fltk")]
        "fltk" => FltkDisplay::new().main_loop(display_fn),
        _ => panic!("Unknown display"),
    };

    input.close();
    input_dimensions.close();
    while worker.step() {}
    Ok(())
}
