pub struct Logger;

impl Logger {
    pub fn info<S: Into<String>>(s: S) {
        let s: String = s.into();
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&s.into());

        #[cfg(any(unix, windows))]
        println!("{s}");
    }

    pub fn error<S: Into<String>>(s: S) {
        let s: String = s.into();

        #[cfg(target_arch = "wasm32")]
        web_sys::console::error_1(&s.into());

        #[cfg(any(unix, windows))]
        eprintln!("{s}");
    }
}