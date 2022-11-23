use minifb::Window;

#[derive(Default)]
pub struct Renderer {
    window: Option<Window>,
}

impl Renderer {
    pub fn new() -> Self {
        Self { window: None }
    }

    pub fn window(&mut self) -> &mut Option<Window> {
        &mut self.window
    }

    pub fn set_window(&mut self, window: Window) {
        self.window = Some(window);
    }
}
