use std::collections::VecDeque;

use crate::{
    read_line::cursor,
    utils::{char_count, BytesBuf},
    write, Vec2,
};

#[cfg(debug_assertions)]
mod debug {
    use bstr::B;

    use crate::bytes_buf;

    use super::*;
    #[derive(Default, Debug)]
    struct DebugLines {
        lines: VecDeque<String>,
    }

    impl DebugLines {
        pub const fn new() -> Self {
            Self {
                lines: VecDeque::new(),
            }
        }
        pub fn push(&mut self, line: String) {
            if self.lines.len() > 5 {
                self.lines.pop_front();
            }
            self.lines.push_back(line);
        }

        pub fn render(&self, term_size: Vec2) -> std::io::Result<()> {
            if self.lines.len() == 0 {
                return Ok(());
            }
            let current_pos = cursor::get_cursor_pos().unwrap();
            let line_len = term_size.x as usize / 2;
            let startx = term_size.x - line_len as u32;
            let mut lines = BytesBuf::new();
            for l in self.lines.iter() {
                if l.len() > line_len {
                    let chars = l.chars().collect::<Vec<_>>();
                    lines.extend(
                        chars
                            .chunks(line_len)
                            .map(|slice| slice.iter().collect::<String>())
                            .map(|s| s.into_bytes()),
                    );
                } else {
                    lines.push_slice(l.as_bytes());
                }
            }
            let sep = [
                B("\n\r"),
                cursor::move_right(startx).as_slice(),
                cursor::kill_line(),
            ]
            .concat();
            let lines = lines.join(sep);
            let buf = bytes_buf! {
                cursor::set_position(startx as u8+1, 1),
                cursor::kill_line(),
                lines,
                cursor::set_position(current_pos.x as u8, current_pos.y as u8)
            };
            write(&buf.join(B("")))?;
            Ok(())
        }
    }

    static DEBUG_LINES: std::sync::Mutex<DebugLines> = std::sync::Mutex::new(DebugLines::new());

    pub fn push_debug_text<S: Into<String>>(line: S) {
        DEBUG_LINES.lock().unwrap().push(line.into());
    }

    pub fn render_debug_text() -> std::io::Result<()> {
        let term_size = cursor::terminal_size()?;
        DEBUG_LINES.lock().unwrap().render(term_size)
    }

    #[macro_export]
    macro_rules! sdbg {
        ($expr:expr) => {{
            let expr = $expr;
            $crate::debug::push_debug_text(format!(
                "[{}:{}] {} = {:?}",
                file!(),
                line!(),
                stringify!($expr),
                expr
            ));
            expr
        }};
    }
}

#[cfg(not(debug_assertions))]
mod release {
    #[macro_export]
    macro_rules! sdbg {
        ($expr:expr) => {};
    }
    pub fn push_debug_text<S: Into<String>>(line: S) {}
    pub fn render_debug_text(term_size: Vec2) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(not(debug_assertions))]
pub use release::*;

#[cfg(debug_assertions)]
pub use debug::*;
