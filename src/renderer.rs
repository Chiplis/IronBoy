use minifb::Window;

static mut INSTANCE: Option<Window> = None;

pub fn set_instance(window: Window) {
    unsafe { INSTANCE = Some(window) }
}

pub fn instance() -> &'static mut Option<Window> {
    unsafe { &mut INSTANCE }
}
