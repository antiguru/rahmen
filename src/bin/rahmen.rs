use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Arg, Command, value_parser};
use font_kit::loaders::freetype::Font;
use image::{DynamicImage, GenericImageView};
use log::{error, info, warn};
use pyo3::prelude::*;
use pyo3::types::PyList;
use timely::communication::Allocator;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::operators::vec::{Branch, Filter, Map, ResultStream};
use timely::dataflow::operators::{
    Capture, Concat, ConnectLoop, Enter, Inspect, Leave, LoopVariable, Notificator, Operator, Probe,
};
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::order::Product;
use timely::worker::Config;

use pathfinder_geometry::rect::RectI;
use rahmen::Vector;
use rahmen::config::Settings;
use rahmen::dataflow::{Configuration, FormatText, ResizeImage};
use rahmen::display::Display;
use rahmen::display_framebuffer::FramebufferDisplay;
#[cfg(feature = "minifb")]
use rahmen::display_minifb::MinifbDisplay;
use rahmen::errors::{RahmenError, RahmenResult};
use rahmen::font::FontRenderer;
use rahmen::provider::{Provider, StatusLineFormatter, load_image_from_path};
use rahmen::provider_list::ListProvider;

static SPLASH: &[u8] = include_bytes!("rahmen.png");
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// dataflow control, this is used as result R part
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RunControl {
    /// terminate stream processing (by external command)
    Terminate,
    /// stream processing encountered an error, but will continue
    Suppressed,
}

/// error handler for display stuff
fn fatal_err<T>(result: RahmenResult<Option<T>>) -> RunResult<T> {
    match result {
        // empty result means we terminate as planned (e.g. end of list)
        Ok(None) => Err(RunControl::Terminate),
        // we process the result
        Ok(Some(t)) => Ok(t),
        // display error and terminate
        Err(e) => {
            error!("Encountered error, terminating: {}", e);
            Err(RunControl::Terminate)
        }
    }
}

/// to keep running the dataflow when there's a stream error
fn suppress_err<T>(result: RahmenResult<T>) -> RunResult<T> {
    result.map_err(|e| {
        // just notify about error but keep processing
        error!("Encountered error, suppressing: {}", e);
        RunControl::Suppressed
    })
}

// `DynamicImage` only implements `PartialEq` (not `Eq`) since it can hold floating-point pixels.
#[derive(Clone, Debug, PartialEq)]
enum Render {
    Image(usize, Vector, Arc<DynamicImage>),
    Blank(usize, Vector, Vector),
}

type RunResult<T> = Result<T, RunControl>;

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
    let mut provider: Box<dyn Provider<_>> = if input.eq("-") {
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

    let output = worker.dataflow(|scope| {
        let configuration_stream = input_configuration.to_stream(scope);

        let img_path_stream = scope.scoped::<Product<_, u32>, _, _>("File loading", |inner| {
            let (handle, cycle) = inner.loop_variable(1);
            let (ok, err) = configuration_stream
                .clone()
                .filter(|c| matches!(c, Configuration::Tick))
                .enter(inner)
                .concat(cycle)
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
            ok.leave(scope)
        });
        let err_stream = img_path_stream.clone().err();

        let mut stash: HashMap<Duration, String> = HashMap::new();
        let mut current_text = None;

        let mut status_line_stream = img_path_stream
            .clone()
            .ok()
            .flat_map(move |(p, _img)| status_line_formatter.format(&p).ok())
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Greeting(text) => Some(text),
                _ => None,
            }))
            .inspect(|loc| info!("Status line: {}", loc));
        if show_time {
            status_line_stream = status_line_stream.unary_notify(
                Pipeline,
                "Show time",
                Some(Duration::from_secs(0)),
                move |input, output, not: &mut Notificator<Duration>| {
                    input.for_each(|cap, data| {
                        if let Some(text) = data.drain(..).next_back() {
                            *stash.entry(*cap.time()).or_default() = text;
                            not.notify_at(cap.retain(0));
                        }
                    });
                    not.for_each(|cap, cnt, not| {
                        let request_notification = if let Some(text) = stash.remove(cap.time()) {
                            current_text = Some(text);
                            cnt == 2
                        } else {
                            true
                        };
                        let now = chrono::Local::now();
                        let delay = std::cmp::max(50, 1000 - now.timestamp_subsec_millis() as u64);
                        if request_notification && !not.frontier(0).is_empty() {
                            let mut next_time = *cap.time() + Duration::from_millis(delay);
                            while !not.frontier(0).less_equal(&next_time) {
                                next_time += Duration::from_secs(1);
                            }
                            not.notify_at(cap.delayed(&next_time));
                        }
                        if let Some(text) = &current_text {
                            let time_text = format!("[{}] {}", now.format(&time_format), text);
                            output.session(&cap).give(time_text);
                        }
                    });
                },
            );
        }
        let status_line_stream =
            status_line_stream.map(|s| s.split('\n').map(Into::into).collect());

        let text_img_stream =
            status_line_stream.format_text(&configuration_stream, font_renderer, 2);

        let adjusted_configuration_stream = {
            let mut stash: HashMap<_, Vec<_>> = HashMap::new();
            // Hack: adjust screen size for the resize operator to reserve space for the status line
            let mut current_font_size = None;
            let mut current_font_canvas_vstretch = None;

            configuration_stream.clone().unary_notify(
                Pipeline,
                "Adjust configuration",
                None,
                move |input, output, not| {
                    input.for_each(|cap, data| {
                        stash.entry(*cap.time()).or_default().append(data);
                        not.notify_at(cap.retain(0));
                    });
                    not.for_each(|cap, _, _not| {
                        if let Some(updates) = stash.remove(cap.time()) {
                            output.session(&cap).give_iterator(updates.into_iter().map(
                                |configuration| {
                                    match configuration {
                                        Configuration::FontSize(font_size) => {
                                            current_font_size = Some(font_size);
                                            Configuration::FontSize(font_size)
                                        }
                                        Configuration::FontCanvasVStretch(font_canvas_vstretch) => {
                                            current_font_canvas_vstretch =
                                                Some(font_canvas_vstretch);
                                            Configuration::FontCanvasVStretch(font_canvas_vstretch)
                                        }
                                        Configuration::ScreenDimensions(width, height) => {
                                            Configuration::ScreenDimensions(
                                                width,
                                                height
                                                    - (current_font_size.unwrap_or(0.)
                                                        * current_font_canvas_vstretch
                                                            .unwrap_or(1.0))
                                                    .ceil()
                                                        as u32,
                                            )
                                        }
                                        configuration => configuration,
                                    }
                                },
                            ));
                        }
                    });
                },
            )
        };

        let img_stream = img_path_stream
            .clone()
            .ok()
            .map(|(_, img)| img)
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Splash(img) => Some(img),
                _ => None,
            }))
            .resize_image(&adjusted_configuration_stream, 1);

        let mut size_stash: HashMap<usize, _> = HashMap::new();
        let mut input_buffer: HashMap<_, Vec<(_, _, _)>> = HashMap::new();

        let composed_img_stream = img_stream.concat(text_img_stream).unary_notify(
            Pipeline,
            "Infer blanking",
            None,
            move |input, output, not| {
                input.for_each(|time, data| {
                    input_buffer.entry(*time.time()).or_default().append(data);
                    not.notify_at(time.retain(0));
                });
                not.for_each(|time, _count, _not| {
                    if let Some(updates) = input_buffer.remove(time.time()) {
                        output
                            .session(&time)
                            .give_iterator(updates.into_iter().flat_map(|(key, anchor, img)| {
                                let rect = RectI::new(
                                    anchor,
                                    Vector::new(img.dimensions().0 as _, img.dimensions().1 as _),
                                );
                                size_stash
                                    .insert(key, rect)
                                    .into_iter()
                                    .flat_map(move |old_rect| compute_blanking(key, rect, old_rect))
                                    .chain(Some(Render::Image(key, anchor, img)))
                            }));
                    }
                })
            },
        );

        err_stream
            .map(Err)
            .concat(composed_img_stream.map(Ok))
            .probe_with(&probe)
            .capture()
    });

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

fn compute_blanking(key: usize, rect: RectI, old_rect: RectI) -> Vec<Render> {
    if let Some(overlap) = old_rect.intersection(rect) {
        let above = RectI::from_points(old_rect.origin(), overlap.upper_right());
        let left = RectI::from_points(old_rect.origin(), overlap.lower_left());
        let right = RectI::from_points(overlap.upper_right(), old_rect.lower_right());
        let below = RectI::from_points(overlap.lower_left(), old_rect.lower_right());
        [above, left, right, below]
            .iter()
            .filter(|r| r.width() > 0 && r.height() > 0)
            .map(|r| Render::Blank(key, r.origin(), r.size()))
            .collect::<Vec<_>>()
    } else {
        vec![Render::Blank(key, old_rect.origin(), old_rect.size())]
    }
}
