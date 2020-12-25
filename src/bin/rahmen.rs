extern crate clap;
extern crate image;

use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::panic;
use std::str::FromStr;
use std::time::Duration;

use clap::{App, Arg};
#[cfg(feature = "minifb")]
use minifb::{Window, WindowOptions};

use rahmen::display::Display;
use rahmen::display_framebuffer::FramebufferDisplay;
use rahmen::display_linuxfb::LinuxFBDisplay;
#[cfg(feature = "minifb")]
use rahmen::display_minifb::MiniFBDisplay;
use rahmen::errors::RahmenResult;
use rahmen::provider::{PathToImageProvider, Provider, RateLimitingProvider, RetryProvider};
use rahmen::provider_list::ListProvider;

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
                    "linuxfb",
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
    let provider: Box<dyn Provider<_>> = if input.eq("-") {
        println!("Reading from stdin");
        Box::new(ListProvider::new(BufReader::new(std::io::stdin())))
    } else if let Ok(file) = File::open(input) {
        println!("Reading from file");
        Box::new(ListProvider::new(BufReader::new(file)))
    } else {
        println!("Reading from pattern {}", input);
        Box::new(rahmen::provider_glob::create(input)?)
    };

    let provider = RateLimitingProvider::new(
        RetryProvider::new(provider.path_to_image()),
        Duration::from_millis(
            (f64::from_str(matches.value_of("time").unwrap()).unwrap() * 1000f64) as u64,
        ),
    );

    match matches.value_of("display").expect("Display missing") {
        #[cfg(feature = "minifb")]
        "minifb" => {
            const WIDTH: usize = 640;
            const HEIGHT: usize = 480;

            let mut window = Window::new(
                "Test - ESC to exit",
                WIDTH,
                HEIGHT,
                WindowOptions::default(),
            )
            .unwrap_or_else(|e| panic!("{}", e));
            // Limit to max ~60 fps update rate
            window.limit_update_rate(Some(std::time::Duration::from_secs(1) / 30));
            MiniFBDisplay::new(window, provider).main_loop();
        }
        "linuxfb" => {
            let framebuffer = linuxfb::Framebuffer::new(
                matches
                    .value_of("output")
                    .expect("Framebuffer output missing"),
            )?;
            println!(
                "Framebuffer size: {:?}, virtual: {:?}",
                framebuffer.get_size(),
                framebuffer.get_virtual_size()
            );
            LinuxFBDisplay::new(provider, framebuffer).main_loop();
        }
        "framebuffer" => {
            let path_to_device = matches
                .value_of("output")
                .expect("Framebuffer output missing");
            let device = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path_to_device)?;
            println!(
                "{:?}",
                framebuffer::Framebuffer::get_fix_screeninfo(&device).unwrap()
            );
            println!(
                "{:?}",
                framebuffer::Framebuffer::get_var_screeninfo(&device).unwrap()
            );
            let mut framebuffer = framebuffer::Framebuffer::new(path_to_device).unwrap();
            rahmen::display_framebuffer::setup_framebuffer(&mut framebuffer);
            // let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Graphics).unwrap();
            FramebufferDisplay::new(provider, framebuffer).main_loop();
            // let _ = framebuffer::Framebuffer::set_kd_mode(framebuffer::KdMode::Text).unwrap();
        }
        other => panic!("Unexpected display driver: {}", other),
    };
    Ok(())
}
