//! Functionality to render images in a window using `minifb`.
//!
//! This is a lightweight development/testing display: unlike the framebuffer backend it opens a
//! regular desktop window. `minifb` loads X11/Wayland at runtime via `dlopen`, so it needs no
//! build-time system libraries.

use crate::Vector;
use crate::display::Display;
use crate::errors::{RahmenError, RahmenResult};

use image::DynamicImage;
use minifb::{Key, ScaleMode, Window, WindowOptions};
use std::time::Duration;

fn window_err<E: std::fmt::Display>(e: E) -> RahmenError {
    RahmenError::WindowError(e.to_string())
}

/// A display driver rendering to a window via `minifb`.
///
/// The backing buffer is a `Vec<u32>` of `0x00RR_GGBB` pixels, the format `minifb` expects.
pub struct MinifbDisplay {
    window: Window,
    buffer: Vec<u32>,
    width: usize,
    height: usize,
}

impl std::fmt::Debug for MinifbDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinifbDisplay")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl MinifbDisplay {
    /// Create a new `minifb` display window.
    pub fn new() -> RahmenResult<Self> {
        let width = 400;
        let height = 300;
        let window = Window::new(
            "Rahmen",
            width,
            height,
            WindowOptions {
                resize: true,
                scale_mode: ScaleMode::Stretch,
                ..WindowOptions::default()
            },
        )
        .map_err(window_err)?;
        Ok(Self {
            window,
            buffer: vec![0; width * height],
            width,
            height,
        })
    }

    /// Main loop to pump `minifb` events and call back into Rahmen's logic.
    pub fn main_loop<F: FnMut(&mut dyn Display) -> RahmenResult<()>>(&mut self, mut callback: F) {
        while self.window.is_open() && !self.window.is_key_down(Key::Escape) {
            if callback(self).is_err() {
                break;
            }
            // Present every iteration so the window keeps processing input even when no new image
            // was produced.
            if let Err(e) = self.present() {
                error!("minifb error: {}", e);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn present(&mut self) -> RahmenResult<()> {
        self.window
            .update_with_buffer(&self.buffer, self.width, self.height)
            .map_err(window_err)
    }

    fn match_dimensions(&mut self) -> RahmenResult<()> {
        let (width, height) = self.window.get_size();
        if (width, height) != (self.width, self.height) {
            self.width = width;
            self.height = height;
            self.buffer = vec![0; width * height];
        }
        Ok(())
    }
}

impl Display for MinifbDisplay {
    fn render(&mut self, _key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Rendering {}ms", e.as_millis()));
        self.match_dimensions()?;
        let rgba = img.to_rgba8();
        let (img_width, img_height) = rgba.dimensions();
        let raw = rgba.as_raw();
        for y in 0..img_height as i32 {
            let dest_y = anchor.y() + y;
            if dest_y < 0 || dest_y >= self.height as i32 {
                continue;
            }
            for x in 0..img_width as i32 {
                let dest_x = anchor.x() + x;
                if dest_x < 0 || dest_x >= self.width as i32 {
                    continue;
                }
                let src = ((y as u32 * img_width + x as u32) * 4) as usize;
                let value = (u32::from(raw[src]) << 16)
                    | (u32::from(raw[src + 1]) << 8)
                    | u32::from(raw[src + 2]);
                self.buffer[dest_y as usize * self.width + dest_x as usize] = value;
            }
        }
        Ok(())
    }

    fn blank(&mut self, _key: usize, anchor: Vector, size: Vector) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Blanking {}ms", e.as_millis()));
        self.match_dimensions()?;
        for y in 0..size.y() {
            let dest_y = anchor.y() + y;
            if dest_y < 0 || dest_y >= self.height as i32 {
                continue;
            }
            let row = dest_y as usize * self.width;
            for x in 0..size.x() {
                let dest_x = anchor.x() + x;
                if dest_x < 0 || dest_x >= self.width as i32 {
                    continue;
                }
                self.buffer[row + dest_x as usize] = 0;
            }
        }
        Ok(())
    }

    fn update(&mut self) -> RahmenResult<()> {
        self.present()
    }

    fn dimensions(&self) -> (u32, u32) {
        let (width, height) = self.window.get_size();
        (width as _, height as _)
    }
}
