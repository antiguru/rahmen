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
    Branch, Capability, CapabilityRef, Capture, Concat, ConnectLoop, Enter, Leave, LoopVariable,
    Map, Operator, Probe,
};
use timely::dataflow::{InputHandle, ProbeHandle, Scope};
use timely::order::Product;
use timely::progress::Timestamp;

use rahmen::display::{preprocess_image, Display};
#[cfg(feature = "fltk")]
use rahmen::display_fltk::FltkDisplay;
use rahmen::display_framebuffer::FramebufferDisplay;
#[cfg(feature = "minifb")]
use rahmen::display_minifb::MiniFBDisplay;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::provider::{load_image_from_path, Provider};
use rahmen::provider_list::ListProvider;
use rahmen::timely_result::ResultStream;

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

    let output = worker.dataflow(|scope| {
        let _last_time: Option<Instant> = None;
        let time_str = matches.value_of("time").unwrap();
        let delay = Duration::from_millis((f64::from_str(time_str).unwrap() * 1000f64) as u64);
        println!("Delay: {:?}", delay);
        let stream = input.to_stream(scope).unary_frontier(
            timely::dataflow::channels::pact::Pipeline,
            "Ticker",
            |cap: Capability<Duration>, _op| {
                let mut buffer = vec![];
                let mut retained_cap: Option<Capability<Duration>> = Some(cap.delayed(cap.time()));
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
                            retained_cap.downgrade(&(*retained_cap.time() + delay))
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
                .map(move |_| provider.next_image())
                .and_then(|p| load_image_from_path(&p).map(|img| (p, img)))
                .branch(|_t, d| d.as_ref().err() == Some(&RahmenError::Retry));
            err.map(|_| ()).connect_loop(handle);
            ok.leave()
        });

        // img_path_stream
        //     .ok()
        //     .flat_map(|(p, _img)| read_exif_from_path(&p))
        //     .inspect(|x| println!("exif: {:?}", x))
        //     .probe_with(&mut probe);

        img_path_stream
            .map(|res| res.map(|(_, img)| img))
            .binary(
                &dimensions_stream,
                timely::dataflow::channels::pact::Pipeline,
                timely::dataflow::channels::pact::Pipeline,
                "Resize",
                |_cap, _op| {
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
                                if let Ok(current_image) = current_image {
                                    out.session(cap.as_ref().unwrap()).give(Ok(preprocess_image(
                                        &current_image,
                                        dimensions.unwrap().0,
                                        dimensions.unwrap().1,
                                    )));
                                } else if let Err(current_image_err) = current_image {
                                    out.session(cap.as_ref().unwrap())
                                        .give(Err(current_image_err.clone()));
                                }
                            }
                        }
                    }
                },
            )
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
                    .any(|r| r.as_ref().err() == Some(&RahmenError::Terminate))
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
