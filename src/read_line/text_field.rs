use std::mem;

use bstr::ByteVec;

use crate::utils::{char_at, char_count};
use crate::Vec2 as Pos;

use super::cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialKey {
    Up,
    Down,
    Tab,
    ShiftTab,
}

#[derive(Debug, Default)]
pub struct TextField {
    text: String,
    cursor_pos: Pos,
    bounds: Pos,
    response: Response,
}

#[macro_export]
macro_rules! commands {
    ($($e:expr),* $(,)?) => {
        [$(AsRef::<[u8]>::as_ref(&$e)),*].concat()
    };
}

bitflags::bitflags! {
    /// This struct gives feeback about which special sequences were intercepted by [`TextField`].
    ///
    /// Note that, in order to save memory, is either a special key or a command, but not both.
    ///
    /// ## Internals
    /// If the `Special` bit is set, this other bits correspond to a special key.
    /// Otherwise, they correspond to the aforementioned commands, which you can handle according
    /// to your own priorities
    ///
    /// ## High-level use
    /// It is highly recommended to use `is_*` methods instead of the low-level `contains` method,
    /// mainly because it handles the special bit quirk for you.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Commands: u8 {
        const None = 0;
        const EOF = 1;
        const Cancel = 1<<1;
        const Newline = 1<<2;
        const Special = 1<<7;
    }
}

impl Commands {
    /// Creates a new [`Commands`] instance from a [`SpecialKey`].
    pub fn special(key: SpecialKey) -> Self {
        Commands::from_bits_retain(key as u8) | Commands::Special
    }
    /// Returns a [`SpecialKey`] if this instance is a special key.
    pub fn get_key(&self) -> Option<SpecialKey> {
        if self.contains(Commands::Special) {
            let key = (*self & !Self::Special).bits();
            if key as usize >= std::mem::variant_count::<SpecialKey>() {
                panic!("Invalid key: {}", key)
            }
            unsafe {
                // SAFETY: this is safe because we checked earlier
                Some(std::mem::transmute(key))
            }
        } else {
            None
        }
    }
    /// Returns true if this instance is the command [`Commands::EOF`].
    pub fn is_eof(&self) -> bool {
        !self.contains(Commands::Special) && self.contains(Commands::EOF)
    }
    /// Returns true if this instance is the command [`Commands::Exit`].
    pub fn is_exit(&self) -> bool {
        !self.contains(Commands::Special) && self.contains(Commands::Cancel)
    }
    /// Returns true if this instance is the command [`Commands::Newline`].
    pub fn is_newline(&self) -> bool {
        !self.contains(Commands::Special) && self.contains(Commands::Newline)
    }
}

/// This is returned by [`TextInput`] after changes are requested. This pattern
/// was chosen because.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    pub bytes: Vec<u8>,
    pub commands: Commands,
}

impl TextField {
    pub fn new(bounds: Pos) -> Self {
        Self {
            bounds,
            ..Default::default()
        }
    }

    pub fn set_bounds(&mut self, bounds: Pos) {
        self.text.truncate(bounds.x as usize);
        self.bounds = bounds;
    }

    fn handle_backspace(&mut self) {
        // Do nothing on line start
        if self.cursor_pos.x == 0 {
            return;
        }
        self.cursor_pos.x -= 1;
        let char_idx = self.char_at(self.cx()).unwrap();
        self.text.remove(char_idx);
        let replacement = &self.text[char_idx..].to_owned();
        self.response.bytes.extend_from_slice(&commands![
            cursor::move_left(1),
            cursor::kill_line(),
            replacement,
            cursor::move_left(char_count(replacement) as u32),
        ])
    }

    pub fn erase_left(&mut self, times: u32) {
        // This could be better optimized
        for _ in 0..times {
            self.handle_backspace();
        }
    }

    pub fn erase_right(&mut self, times: u32) {
        self.move_right(times);
        for _ in 0..times {
            self.handle_backspace();
        }
    }

    fn cx(&self) -> usize {
        self.cursor_pos.x as usize
    }

    fn char_at(&self, index: usize) -> Option<usize> {
        char_at(&self.text, index)
    }

    fn text_len(&self) -> usize {
        char_count(&self.text)
    }

    fn handle_char(&mut self, c: char) {
        if self.cursor_pos.x >= self.bounds.x {
            return;
        }
        let text_len = self.text_len();
        if self.cursor_pos.x as usize == text_len {
            self.text.push(c);
            self.response.bytes.push_char(c);
        } else {
            self.text.insert(self.char_at(self.cx()).unwrap(), c);
            let replacement = &self.text[self.cursor_pos.x as usize..];
            self.response.bytes.extend_from_slice(&commands![
                cursor::kill_line(),
                replacement,
                cursor::move_left(char_count(replacement) as u32 - 1),
            ])
        }
        self.cursor_pos.x += 1;
    }

    pub fn set_text(&mut self, text: &str) -> Response {
        self.response.commands = Commands::empty();
        self.response.bytes = commands![
            cursor::move_left(self.cursor_pos.x),
            cursor::kill_line(),
            text
        ];

        self.cursor_pos.x = char_count(text) as u32;
        self.text = text.to_string();

        mem::take(&mut self.response)
    }

    pub fn erase_rest(&mut self) {
        self.response.bytes = commands![cursor::kill_line(),];
        self.text.truncate(self.cursor_pos.x as usize);
    }

    pub fn move_to(&mut self, _text: &str, index: usize) {
        let x = self.cursor_pos.x;
        let index = index as u32;
        if index > x {
            self.move_right(index - x)
        } else if index < x {
            self.move_left(x - index)
        }
    }

    pub fn move_left(&mut self, times: u32) {
        let times = times.min(self.cursor_pos.x);
        if times == 0 {
            return;
        };
        self.cursor_pos.x -= times;
        self.response
            .bytes
            .extend_from_slice(&cursor::move_left(times));
    }

    pub fn move_right(&mut self, times: u32) {
        let newx = self.cursor_pos.x + times;
        if newx >= self.bounds.x {
            return;
        }
        self.cursor_pos.x = newx;
        self.response
            .bytes
            .extend_from_slice(&cursor::move_right(times));
    }

    pub fn handle_input(&mut self, input: &str) -> Response {
        let mut it = input.chars();
        while let Some(c) = it.next() {
            match c as u8 {
                1 => {
                    // ctrl A
                    self.move_left(self.cursor_pos.x);
                }
                3 => {
                    // ctrl C
                    self.response.commands = Commands::Cancel;
                }
                4 => {
                    // ctrl D
                    self.response.commands = Commands::EOF;
                }
                5 => {
                    // ctrl E
                    self.move_right(self.text_len() as u32 - self.cursor_pos.x);
                }
                b'\t' => {
                    self.response.commands = Commands::special(SpecialKey::Tab);
                }
                b'\r' => {
                    self.response.commands = Commands::Newline;
                }
                b'\x1b' => {
                    if it.next() != Some('[') {
                        continue;
                    }
                    match it.next().unwrap() {
                        'A' => self.response.commands = Commands::special(SpecialKey::Up),
                        'B' => self.response.commands = Commands::special(SpecialKey::Down),
                        'C' => self.move_right(1),
                        'D' => self.move_left(1),
                        'Z' => self.response.commands = Commands::special(SpecialKey::ShiftTab),
                        '3' => {
                            if it.next() == Some('~') {
                                self.move_right(1);
                                self.handle_backspace()
                            }
                        }
                        _ => (),
                    }
                }
                1..=26 => (),
                127 => self.handle_backspace(),
                _ => self.handle_char(c),
            }
        }
        self.take_response()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = Default::default();
        self.response = Default::default();
    }

    pub fn take_response(&mut self) -> Response {
        mem::take(&mut self.response)
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor_pos(&self) -> Pos {
        self.cursor_pos
    }
}
