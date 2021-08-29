//! Functionality to render images on a FLTK window

use crate::display::Display;
use crate::errors::{RahmenError, RahmenResult};

use crate::Vector;
use fltk::{
    app::{App, Scheme},
    enums::ColorDepth,
    enums::{Event, Key},
    frame::Frame,
    prelude::{GroupExt, WidgetBase, WidgetExt, WindowExt},
    window::Window,
};
use image::{DynamicImage, GenericImage, Rgb, RgbImage};
use std::time::Duration;

/// A display driver rendering to a FLTK window
#[derive(Debug)]
pub struct FltkDisplay {
    window: Window,
    frame: Frame,
    image: RgbImage,
}

impl FltkDisplay {
    /// Create a new FLTK display
    pub fn new() -> Self {
        let dim_x = 400;
        let dim_y = 300;
        let _app = App::default().with_scheme(Scheme::Gleam);
        let mut window = Window::new(100, 100, dim_x, dim_y, "Rahmen");
        let frame = Frame::new(0, 0, dim_x, dim_y, "");
        window.make_resizable(true);
        window.end();
        window.show_with_env_args();

        let mut is_fullscreen = false;
        window.handle(move |t, ev| match ev {
            Event::KeyDown => match fltk::app::event_key() {
                Key::Enter => {
                    t.fullscreen(!is_fullscreen);
                    is_fullscreen = !is_fullscreen;
                    true
                }
                _ => false,
            },
            _ => false,
        });

        Self {
            window,
            frame,
            image: Default::default(),
        }
    }

    /// Main loop to handle FLTK events and call back into Rahmen's logic
    pub fn main_loop<F: FnMut(&mut dyn Display) -> RahmenResult<()>>(&mut self, mut callback: F) {
        while callback(self).is_ok() && self.window.shown() {
            match fltk::app::wait_for(Duration::from_millis(50).as_secs_f64()) {
                Err(e) => {
                    eprintln!("FLTK error: {}", e);
                    // break;
                }
                _ => {}
            }
        }
    }

    fn match_dimensions(&mut self) -> RahmenResult<()> {
        if self.image.dimensions() != self.dimensions() {
            self.image = RgbImage::from_raw(
                self.dimensions().0,
                self.dimensions().1,
                vec![0u8; (self.dimensions().0 * self.dimensions().1 * 3) as usize],
            )
            .ok_or(RahmenError::Terminate)?;
        }
        Ok(())
    }
}

impl Display for FltkDisplay {
    fn render(&mut self, _key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| println!("Rendering {}ms", e.as_millis()));
        self.match_dimensions()?;
        self.image
            .copy_from(&img.to_rgb8(), anchor.x() as _, anchor.y() as _)?;
        Ok(())
    }

    fn blank(&mut self, _key: usize, anchor: Vector, size: Vector) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| println!("Rendering {}ms", e.as_millis()));
        self.match_dimensions()?;
        let black = image::FlatSamples::with_monocolor(&Rgb([0; 3]), size.x() as _, size.y() as _);
        self.image
            .copy_from(&black.as_view().unwrap(), anchor.x() as _, anchor.y() as _)?;
        Ok(())
    }

    fn update(&mut self) -> RahmenResult<()> {
        let (x, y) = self.image.dimensions();
        let image =
            fltk::image::RgbImage::new(self.image.as_raw(), x as _, y as _, ColorDepth::Rgb8)
                .unwrap();
        self.frame.set_image(Some(image));
        self.window.redraw();
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.frame.width() as _, self.frame.height() as _)
    }
}
