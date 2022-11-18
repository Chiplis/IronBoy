use crate::Args;
use clap::Parser;
use minifb::{Scale, ScaleMode, Window, WindowOptions};
use once_cell::sync::Lazy;

static mut INSTANCE: Lazy<Option<Window>> = Lazy::new(|| {
    let args = Args::parse();
    if args.headless {
        None
    } else {
        Some(
            Window::new(
                &args.rom_file,
                160,
                144,
                WindowOptions {
                    borderless: false,
                    transparency: false,
                    title: true,
                    resize: true,
                    scale: Scale::X1,
                    scale_mode: ScaleMode::Stretch,
                    topmost: false,
                    none: false,
                },
            )
            .unwrap(),
        )
    }
});

// TODO: Figure out a safer way to access the Window instance
pub fn instance() -> &'static mut Option<Window> {
    unsafe { &mut INSTANCE }
}
