use pixels::Pixels;
use std::cmp::max;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Renderer {
    pub(crate) slowest: Duration,
    pixels: Option<Pixels>,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            slowest: Duration::from_secs(0),
            pixels: None,
        }
    }

    pub fn pixels(&mut self) -> &mut Option<Pixels> {
        &mut self.pixels
    }

    pub fn set_pixels(&mut self, pixels: Pixels) {
        self.pixels = Some(pixels);
    }

    pub(crate) fn render(&mut self, screen: &[u8]) {
        let now = Instant::now();
        if let Some(pixels) = self.pixels().as_mut() {
            let frame = pixels.get_frame_mut();
            frame.copy_from_slice(screen);
            pixels.render().unwrap();
            let duration = Instant::now() - now;
            // println!("Render took {:?}", duration);
            self.slowest = max(self.slowest, duration);
        }
    }
}
