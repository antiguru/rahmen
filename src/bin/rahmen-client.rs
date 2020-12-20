extern crate clap;
extern crate image;

use std::io::BufReader;

use clap::{App, Arg};
use minifb::{Window, WindowOptions};

use rahmen::display::Display;
use rahmen::display_minifb::MiniFBDisplay;
use rahmen::provider::RateLimitingProvider;
use rahmen::provider_list::ListProvider;
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let matches = App::new("Rahmen client")
        .arg(
            Arg::new("display")
                .short('d')
                .long("display")
                .about("Select the display provider")
                .value_name("display")
                .takes_value(true)
                .possible_values(&["framebuffer", "minifb"])
                .default_value("minifb"),
        )
        .arg(
            Arg::new("provider")
                .short('p')
                .long("provider")
                .about("Image provider")
                .takes_value(true)
                .possible_values(&["directory", "list"])
                .default_value("list"),
        )
        .arg(Arg::new("input").short('i').long("input").takes_value(true))
        .get_matches();

    match matches.value_of("display").expect("Display missing") {
        "framebuffer" => unimplemented!(),
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
            MiniFBDisplay::new(
                window,
                RateLimitingProvider::new(
                    ListProvider::new(BufReader::new(
                        File::open(&PathBuf::from(
                            matches.value_of("input").expect("Input mising"),
                        ))
                        .expect("Failed to open input"),
                    )),
                    Duration::from_secs(10),
                ),
            )
            .main_loop();
        }
        other => panic!("Unexpected display driver: {}", other),
    };
}
