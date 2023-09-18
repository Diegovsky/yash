use std::sync::{Arc, atomic::AtomicBool};


#[derive(Debug, Clone, Default)]
pub struct Signals {
    pub sigint: Arc<AtomicBool>,
}

impl Signals {
    pub fn init() -> Self {
        let sigint = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, sigint.clone()).unwrap();
        Self {
            sigint
        }
    }
}
