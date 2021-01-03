//! Functionality to render images on a FLTK window

use crate::display::Display;
use crate::errors::RahmenResult;

use fltk::{
    app::{App, Scheme},
    frame::Frame,
    image::RgbImage,
    text::Key,
    window::Window,
    Event, GroupExt, WidgetBase, WidgetExt, WindowExt,
};
use image::{DynamicImage, GenericImageView};
use std::time::Duration;

/// A display driver rendering to a FLTK window
#[derive(Debug)]
pub struct FltkDisplay {
    window: Window,
    frame: Frame,
}

impl FltkDisplay {
    /// Create a new FLTK display
    pub fn new() -> Self {
        let _app = App::default().with_scheme(Scheme::Gleam);
        let mut window = Window::new(100, 100, 400, 300, "Rahmen");
        let frame = Frame::new(0, 0, 400, 300, "");
        window.make_resizable(true);
        window.end();
        window.show_with_env_args();

        let mut is_fullscreen = false;
        window.handle2(move |t, ev| match ev {
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

        Self { window, frame }
    }

    /// Main loop to handle FLTK events and call back into Rahmen's logic
    pub fn main_loop<F: FnMut(Box<&mut dyn Display>) -> RahmenResult<()>>(
        &mut self,
        mut callback: F,
    ) {
        while callback(Box::new(self)).is_ok() && self.window.shown() {
            match fltk::app::wait_for(Duration::from_millis(50).as_secs_f64()) {
                Err(e) => {
                    eprintln!("FLTK error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    }
}

impl Display for FltkDisplay {
    fn render(&mut self, img: &DynamicImage) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| println!("Rendering {}ms", e.as_millis()));
        let (x, y) = img.dimensions();
        let image = RgbImage::new(&img.to_rgb8().into_raw(), x, y, 3).unwrap();
        self.frame.set_image(Some(image));
        self.window.redraw();
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.frame.width() as _, self.frame.height() as _)
    }
}
