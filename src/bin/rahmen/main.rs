mod dataflow;

use dataflow::{Render, RunControl};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Arg, Command, value_parser};
use font_kit::loaders::freetype::Font;
use log::{error, info, warn};
use pyo3::prelude::*;
use pyo3::types::PyList;
use timely::communication::Allocator;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::worker::Config;

use rahmen::config::Settings;
use rahmen::dataflow::Configuration;
use rahmen::display::Display;
use rahmen::display_framebuffer::FramebufferDisplay;
#[cfg(feature = "minifb")]
use rahmen::display_minifb::MinifbDisplay;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::font::FontRenderer;
use rahmen::provider::{Provider, StatusLineFormatter};
use rahmen::provider_list::ListProvider;

static SPLASH: &[u8] = include_bytes!("rahmen.png");
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(unix)]
const SYSTEM_CONFIG_PATH: &str = "/etc/rahmen.toml";

fn main() -> RahmenResult<()> {
    env_logger::init();

    // read command line args
    let matches = Command::new("Rahmen client")
        .arg(
            Arg::new("display")
                .short('d')
                .long("display")
                .help("Select the display provider")
                .value_name("display")
                .value_parser([
                    #[cfg(feature = "minifb")]
                    "minifb",
                    "framebuffer",
                ])
                .default_value("framebuffer"),
        )
        .arg(Arg::new("input").required(true).index(1))
        .arg(Arg::new("output").short('o').long("output"))
        .arg(
            Arg::new("time")
                .short('t')
                .long("time")
                .value_parser(value_parser!(f64)),
        )
        .arg(
            Arg::new("buffer_max_size")
                .long("buffer_max_size")
                .value_parser(value_parser!(usize))
                .default_value("16000000"),
        )
        .arg(
            Arg::new("font")
                .long("font")
                .default_value("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        )
        .arg(
            Arg::new("font_size")
                .long("font_size")
                .value_parser(value_parser!(f32)),
        )
        .arg(Arg::new("config").long("config").short('c'))
        .get_matches();

    // evaluate input arg
    let input = matches
        .get_one::<String>("input")
        .expect("Input missing")
        .as_str();
    // box is used bec of dynamic typing for provider
    let provider: Box<dyn Provider<_>> = if input.eq("-") {
        info!("Reading from stdin");
        Box::new(ListProvider::new(BufReader::new(std::io::stdin())))
    } else if let Ok(file) = File::open(input) {
        info!("Reading from file");
        Box::new(ListProvider::new(BufReader::new(file)))
    } else {
        info!("Reading from pattern {}", input);
        Box::new(rahmen::provider_glob::create(input)?)
    };

    // look for config file
    let dirs = xdg::BaseDirectories::new();
    let settings: Settings = if let Some(path) = matches
        .get_one::<String>("config")
        .map(PathBuf::from)
        .or_else(|| dirs.find_config_file("rahmen.toml"))
        .or_else(|| {
            #[cfg(unix)]
            if std::fs::metadata(SYSTEM_CONFIG_PATH).is_ok() {
                Some(SYSTEM_CONFIG_PATH.into())
            } else {
                None
            }
            #[cfg(not(unix))]
            None
        }) {
        config::Config::builder()
            .add_source(config::File::from(path))
            .build()?
            .try_deserialize()?
    } else {
        warn!("Config file not found, continuing with default settings");
        Default::default()
    };
    // Python search path: use the Python system path, and prepend the value(s) from the config file
    // Note: contrary to the documentation, the Python system path will not contain the directory from which we're called,
    // so this has to be indicated in the configuration file
    if let Some(python_paths) = settings.py_path {
        Python::attach(|py| -> PyResult<()> {
            let syspath = py.import("sys")?.getattr("path")?.cast_into::<PyList>()?;
            for path in &python_paths {
                syspath.insert(0, path)?;
            }
            Ok(())
        })
        .expect("Failed to configure Python sys.path");
    }

    // build the status line, using the settings from the config file for the individual
    // metadata tags,
    // the metadata items being joined using the separator from the config file (or with the
    // default value (", ") if no separator is given there)
    let status_line_formatter = StatusLineFormatter::new(
        settings.status_line.iter().cloned(),
        settings.py_postprocess,
        settings.separator.unwrap_or_else(|| ", ".to_string()),
    )?;

    // continue evaluating the command line args
    let buffer_max_size: usize = *matches
        .get_one::<usize>("buffer_max_size")
        .expect("Missing buffer_max_size");

    let font =
        Font::from_path(matches.get_one::<String>("font").expect("Missing font"), 0).unwrap();
    let font_renderer = FontRenderer::with_font(font);

    let duration_millis = (matches
        .get_one::<f64>("time")
        .copied()
        .or(settings.delay)
        .unwrap_or(90.)
        * 1000f64) as u64;
    let delay = Duration::from_millis(duration_millis);
    info!("Delay: {:?}", delay);

    // font size to use (px)
    let font_size_f = matches
        .get_one::<f32>("font_size")
        .copied()
        .or(settings.font_size)
        .unwrap_or(30.);

    let show_time = settings.display_time.unwrap_or(false);
    let time_format = settings.time_format.unwrap_or("%H:%M:%S".into());

    // initialization for timely dataflow
    let allocator = Allocator::Thread(timely::communication::allocator::thread::Thread::default());
    let mut worker =
        timely::worker::Worker::new(Config::default(), allocator, Some(Instant::now()));

    // input: #1 timeline #2 screen resolution
    let mut input_configuration: InputHandle<
        Duration,
        CapacityContainerBuilder<Vec<Configuration>>,
    > = InputHandle::new();
    // to gather information about progress
    let probe = ProbeHandle::new();

    let output = dataflow::build(
        &mut worker,
        &mut input_configuration,
        &probe,
        provider,
        status_line_formatter,
        font_renderer,
        dataflow::Options {
            buffer_max_size,
            show_time,
            time_format,
        },
    );

    let start_time = Instant::now();
    let mut dimensions = None;

    input_configuration.send(Configuration::FontSize(font_size_f));
    // enlarge font canvas vertically by this factor (default given here: 1.4)
    input_configuration.send(Configuration::FontCanvasVStretch(1.4));
    match image::load_from_memory(SPLASH) {
        Ok(image) => {
            info!("Sending splash screen");
            input_configuration.send(Configuration::Splash(Arc::new(image)));
        }
        Err(err) => warn!("Failed to load splash screen: {}", err),
    }
    input_configuration.send(Configuration::Greeting(format!("Rahmen {}", VERSION)));

    let mut next_image_at = start_time.elapsed() + Duration::from_secs(1);

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
    };

    match matches
        .get_one::<String>("display")
        .expect("Display missing")
        .as_str()
    {
        "framebuffer" => {
            let path_to_device = matches
                .get_one::<String>("output")
                .expect("Framebuffer output missing");
            let framebuffer = framebuffer::Framebuffer::new(path_to_device).unwrap();
            let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Graphics)
                .map_err(|_e| warn!("Failed to set graphics mode."));
            ctrlc::set_handler(|| {
                let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Text)
                    .map_err(|_e| warn!("Failed to set graphics mode."));
                std::process::exit(0);
            })
            .unwrap();
            FramebufferDisplay::new(framebuffer).main_loop(display_fn);
            let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Text)
                .map_err(|_e| warn!("Failed to set graphics mode."));
        }
        #[cfg(feature = "minifb")]
        "minifb" => MinifbDisplay::new()?.main_loop(display_fn),
        _ => panic!("Unknown display"),
    };

    input_configuration.close();
    while worker.step() {}
    Ok(())
}
