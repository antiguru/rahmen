use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{App, Arg};
use font_kit::loaders::freetype::Font;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::operators::{
    Branch, Capture, Concat, ConnectLoop, Enter, Filter, Inspect, Leave, LoopVariable, Map, Probe,
    ResultStream,
};
use timely::dataflow::{InputHandle, ProbeHandle, Scope};
use timely::order::Product;

use rahmen::dataflow::{ComposeImage, Configuration, FormatText, ResizeImage};
use rahmen::display::Display;
#[cfg(feature = "fltk")]
use rahmen::display_fltk::FltkDisplay;
use rahmen::display_framebuffer::FramebufferDisplay;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::font::FontRenderer;
use rahmen::provider::{format_exif, load_image_from_path, Provider};
use rahmen::provider_list::ListProvider;

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
        .arg(
            Arg::new("font")
                .long("font")
                .takes_value(true)
                .validator(|f| File::open(f))
                .default_value("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
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

    let mut input_configuration: InputHandle<_, Configuration> = InputHandle::new();
    let mut probe = ProbeHandle::new();

    let buffer_max_size: usize = matches
        .value_of("buffer_max_size")
        .expect("Missing buffer_max_size")
        .parse()
        .unwrap();

    let font = Font::from_path(matches.value_of("font").unwrap(), 0).unwrap();
    let font_renderer = FontRenderer::with_font(font);

    let time_str = matches.value_of("time").unwrap();
    let delay = Duration::from_millis((f64::from_str(time_str).unwrap() * 1000f64) as u64);
    println!("Delay: {:?}", delay);

    let output = worker.dataflow(|scope| {
        let configuration_stream = input_configuration.to_stream(scope);

        let img_path_stream = scope.scoped::<Product<_, u32>, _, _>("File loading", |inner| {
            let (handle, cycle) = inner.loop_variable(1);
            let (ok, err) = configuration_stream
                .filter(|c| matches!(c, Configuration::Tick))
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
            err.map(|_| Configuration::Tick).connect_loop(handle);
            ok.leave()
        });
        let err_stream = img_path_stream.err();

        let status_line_stream = img_path_stream
            .ok()
            .flat_map(|(p, _img)| format_exif(&p).ok())
            .inspect(|loc| println!("Status line: {}", loc));

        let text_img_stream =
            status_line_stream.format_text(&configuration_stream, font_renderer, 2);

        let adjusted_configuration_stream = {
            // Hack: adjust screen size for the resize operator to reserve space for the status line
            let mut current_font_size = None;
            configuration_stream.map(move |configuration| match configuration {
                Configuration::FontSize(font_size) => {
                    current_font_size = Some(font_size);
                    Configuration::FontSize(font_size)
                }
                Configuration::ScreenDimensions(width, height) => Configuration::ScreenDimensions(
                    width,
                    height - ( current_font_size.unwrap_or(0.) * 1.5) as u32, // this factor is the same as in dataflow.rs
                ),
                configuration => configuration,
            })
        };

        let img_stream = img_path_stream
            .ok()
            .map(|(_, img)| img)
            .resize_image(&adjusted_configuration_stream, 1);

        let composed_img_stream = img_stream
            .concat(&text_img_stream)
            .compose_image(&configuration_stream);

        err_stream
            .map(Err)
            .concat(&composed_img_stream.map(Ok))
            .probe_with(&mut probe)
            .capture()
    });

    let start_time = Instant::now();
    let mut dimensions = None;
    input_configuration.send(Configuration::FontSize(30.)); /// font size to use (px?)
    input_configuration.send(Configuration::FontCanvasVStretch(1.5)); /// enlarge font canvas vertically by this factor (default: 1.5)

    let mut next_image_at = start_time.elapsed();

    let display_fn = |display: &mut dyn Display| {
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
        while probe.less_than(input_configuration.time()) {
            worker.step();
        }
        match output.try_iter().all(|result| match result {
            // Continue processing on progress messages
            Event::Progress(_) => true,
            // Handle data messages by rending an image and determining whether to terminate
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

    input_configuration.close();
    while worker.step() {}
    Ok(())
}
