extern crate glob;
extern crate image;
extern crate linuxfb;
extern crate memmap;
#[cfg(feature = "minifb")]
extern crate minifb;

use std::panic;
use std::sync::{Arc, Mutex};

pub mod display;
pub mod display_framebuffer;
pub mod display_linuxfb;
#[cfg(feature = "minifb")]
pub mod display_minifb;
pub mod errors;
pub mod provider;
pub mod provider_glob;
pub mod provider_list;

pub fn wrap_panic_and_restore<F: FnOnce(), C: Fn() + Send + Sync + 'static>(f: F, cleanup: C) {
    let panic_hook = Arc::new(Mutex::new(Some(panic::take_hook())));
    let moved_panic_hook = panic_hook.clone();
    panic::set_hook(Box::new(move |p| {
        cleanup();
        if let Some(panic_hook) = moved_panic_hook.lock().unwrap().take() {
            panic_hook(p);
        };
    }));
    f();
    if let Some(panic_hook) = panic_hook.lock().unwrap().take() {
        panic::set_hook(panic_hook);
    };
}
