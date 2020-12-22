extern crate clap;
extern crate image;

use std::io::BufReader;

use clap::{App, Arg};
use minifb::{Window, WindowOptions};

use rahmen::display::Display;
use rahmen::display_linuxfb::LinuxFBDisplay;
use rahmen::display_minifb::MiniFBDisplay;
use rahmen::errors::RahmenResult;
use rahmen::provider::{ImageErrorToRetryProvider, Provider, RateLimitingProvider, RetryProvider};
use rahmen::provider_list::ListProvider;
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> RahmenResult<()> {
    let matches = App::new("Rahmen client")
        .arg(
            Arg::new("display")
                .short('d')
                .long("display")
                .about("Select the display provider")
                .value_name("display")
                .takes_value(true)
                .possible_values(&["linuxfb", "minifb"])
                .default_value("minifb"),
        )
        .arg(
            Arg::new("provider")
                .short('p')
                .long("provider")
                .about("Image provider")
                .takes_value(true)
                .possible_values(&["pattern", "list"])
                .default_value("list"),
        )
        .arg(Arg::new("input").short('i').long("input").takes_value(true))
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .takes_value(true),
        )
        .get_matches();

    let provider: Box<dyn Provider> = match matches.value_of("provider").expect("Provider missing")
    {
        "pattern" => Box::new(rahmen::provider_glob::create(
            matches.value_of("input").expect("Input mising"),
        )?),
        "list" => Box::new(ListProvider::new(BufReader::new(
            File::open(&PathBuf::from(
                matches.value_of("input").expect("Input mising"),
            ))
            .expect("Failed to open input"),
        ))),
        other => panic!("Unknown provider: {}", other),
    };

    let provider = RateLimitingProvider::new(
        RetryProvider::new(ImageErrorToRetryProvider::new(provider)),
        Duration::from_secs(10),
    );

    match matches.value_of("display").expect("Display missing") {
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
            let mut framebuffer = linuxfb::Framebuffer::new(
                matches
                    .value_of("output")
                    .expect("Framebuffer output missing"),
            )?;
            println!("Framebuffer size: {:?}", framebuffer.get_size());
            println!(
                "Framebuffer virtual size: {:?}",
                framebuffer.get_virtual_size()
            );
            rahmen::display_linuxfb::setup_framebuffer(&mut framebuffer);
            LinuxFBDisplay::new(provider, framebuffer).main_loop();
        }
        other => panic!("Unexpected display driver: {}", other),
    };
    Ok(())
}
