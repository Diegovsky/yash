#[macro_export]
macro_rules! sdbg {
    ($expr:expr) => {$expr};
}
pub fn push_debug_text<S: Into<String>>(line: S) {}
pub fn render_debug_text() -> std::io::Result<()> {
    Ok(())
}

