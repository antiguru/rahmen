extern crate clap;
extern crate ctrlc;
extern crate timely;

use std::fs::File;
use std::io::BufReader;

use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::{App, Arg};
#[cfg(feature = "minifb")]
use minifb::{Window, WindowOptions};
use timely::dataflow::operators::{
    Branch, Capability, CapabilityRef, Concat, ConnectLoop, Enter, Inspect, Leave, LoopVariable,
    Map, Operator, Probe,
};
use timely::dataflow::{InputHandle, ProbeHandle, Scope};

use rahmen::display::{preprocess_image, Display};
use rahmen::display_framebuffer::FramebufferDisplay;
#[cfg(feature = "minifb")]
use rahmen::display_minifb::MiniFBDisplay;
use rahmen::errors::RahmenResult;
use rahmen::provider::{load_image_from_path, Provider};
use rahmen::provider_list::ListProvider;

use timely::order::Product;
use timely::progress::Timestamp;

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
                    "framebuffer",
                    #[cfg(feature = "minifb")]
                    "minifb",
                ])
                .default_value("minifb"),
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

    let path_to_device = matches
        .value_of("output")
        .expect("Framebuffer output missing");
    let framebuffer = framebuffer::Framebuffer::new(path_to_device).unwrap();
    let mut display = FramebufferDisplay::new(framebuffer);

    let dimensions = display.dimensions();

    worker.dataflow(|scope| {
        let _last_time: Option<Instant> = None;
        let time_str = matches.value_of("time").unwrap();
        let delay = Duration::from_millis((f64::from_str(time_str).unwrap() * 1000f64) as u64);
        println!("Delay: {:?}", delay);
        let stream = input
            .to_stream(scope)
            .unary_frontier(
                timely::dataflow::channels::pact::Pipeline,
                "Ticker",
                |cap: Capability<Duration>, _op| {
                    let mut buffer = vec![];
                    let mut retained_cap: Capability<Duration> = cap.delayed(cap.time());
                    move |input_handle, output_handle| {
                        if !input_handle
                            .frontier
                            .frontier()
                            .less_equal(retained_cap.time())
                        {
                            output_handle.session(&retained_cap).give(());
                            retained_cap.downgrade(&(*retained_cap.time() + delay))
                        }
                        while let Some((cap, in_buffer)) = input_handle.next() {
                            in_buffer.swap(&mut buffer);
                            output_handle.session(&cap).give_vec(&mut buffer);
                        }
                    }
                },
            )
            .inspect_time(|x, t| println!("{:?} at {:?}", t, x));
        let dimensions_stream = input_dimensions.to_stream(scope);
        scope
            .scoped::<Product<_, u32>, _, _>("File loading", |inner| {
                let (handle, cycle) = inner.loop_variable(1);
                let (ok, err) = stream
                    .enter(inner)
                    .concat(&cycle)
                    .map(move |_| provider.next_image().unwrap())
                    .map(|p| {
                        load_image_from_path(p)
                            .map_err(|e| eprintln!("Failed to load image: {}", e))
                            .ok()
                    })
                    .branch(|_t, d| d.is_none());
                err.map(|_| ()).connect_loop(handle);
                ok.map(Option::unwrap).leave()
            })
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
                                println!("Got image");
                                current_image = Some(image.clone());
                                did_work |= true;
                            }
                            track_time(&mut cap, time);
                        });
                        in2.for_each(|time, data| {
                            if let Some(dims) = data.last() {
                                println!("Got dimension");
                                dimensions = Some(dims.clone());
                                did_work |= true;
                            }
                            track_time(&mut cap, time);
                        });
                        if did_work && dimensions.is_some() && current_image.is_some() {
                            println!("Dimensions: {:?}", dimensions);
                            out.session(cap.as_ref().unwrap()).give(preprocess_image(
                                current_image.as_ref().unwrap(),
                                dimensions.unwrap().0,
                                dimensions.unwrap().1,
                            ));
                        }
                    }
                },
            )
            .map(move |img| display.render(img).unwrap())
            .probe_with(&mut probe);
    });

    input_dimensions.send(dimensions);
    input_dimensions.close();

    let start_time = Instant::now();
    loop {
        let now = start_time.elapsed();
        input.advance_to(now);
        while probe.less_than(input.time()) {
            worker.step();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
