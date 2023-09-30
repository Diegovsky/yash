use std::{
    ops::Add,
    os::{fd::FromRawFd, unix::prelude::OsStrExt},
    path::Path,
    process::Stdio,
};

use bstr::ByteSlice;
use glam::UVec2;

use crate::{
    read, sdbg, shell_print, shell_println,
    utils::{char_at, char_count, path_filename, path_parent},
    write, Shell, YshResult,
};

use self::{completion::SelectionDirection, history::History};

pub mod completion;
pub mod cursor;
pub mod history;
pub mod text_field;

#[derive(Debug, Default)]
pub struct ReadLine {
    history: History,
    completion: completion::Completer,
    text_field: text_field::TextField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Execute {
    Exit,
    Cancel,
    Command(String),
}
pub fn utf8_byte_len(i: u8) -> Option<u8> {
    if i >= 192 {
        let len = if i >> 5 & 1 == 0 {
            2
        } else if i >> 6 & 1 == 0 {
            3
        } else if i >> 7 & 1 == 0 {
            4
        } else {
            panic!("Invalid utf-8 sequence!")
        };
        return Some(len);
    }
    None
}

impl ReadLine {
    pub fn new_with_history(lines: Vec<String>) -> Self {
        Self {
            history: History::from_lines(lines),
            ..Default::default()
        }
    }
    pub fn history(&self) -> &[String] {
        self.history.lines()
    }
    fn aligned_read(c: &mut [u8]) -> nix::Result<&[u8]> {
        loop {
            let mut extra = 0;
            if read(&mut c[0..1])? != 0 {
                if c[0] == b'\x1b' {
                    extra = read(&mut c[1..])?;
                } else if let Some(utf8len) = utf8_byte_len(c[0]) {
                    extra = read(&mut c[1..utf8len as usize])?;
                }
                return Ok(&c[0..1 + extra]);
            } else {
                continue;
            };
        }
    }

    pub fn scroll_history(&mut self, offset: isize) -> YshResult<()> {
        if let Some(new_line) = self.history.scroll(self.text_field.text(), offset) {
            let response = self.text_field.set_text(new_line);
            write(&response.bytes)?;
        } else {
            write(cursor::bell())?;
        }
        Ok(())
    }

    /// This function is not a method because of missing disjoint borrow rules
    // !TODO: put this inside text_field?
    fn word_at_cursor(text_field: &text_field::TextField) -> &str {
        let line = text_field.text();
        let cursor_pos = text_field.cursor_pos();
        let UVec2 { x: word_end, .. } = cursor_pos;
        let word_end = word_end as usize;
        if word_end != 0 && line.chars().nth(word_end - 1) != Some(' ') {
            let word_start = line[0..word_end]
                .rfind(|c| c == ' ')
                .map(|i| i + 1)
                .unwrap_or_default();
            &line[word_start..word_end]
        } else {
            ""
        }
    }

    pub fn complete_next(&mut self, direction: SelectionDirection) -> YshResult<()> {
        let word = Self::word_at_cursor(&self.text_field);
        self.completion.next(word, direction)?;
        Ok(())
    }

    fn handle_response(&mut self, response: text_field::Response) -> YshResult<Option<Execute>> {
        use text_field::{Commands, SpecialKey};
        write(&response.bytes)?;
        match self.completion.current_completion() {
            None => Ok(Some(match response.commands {
                Commands::None => return Ok(None),
                Commands::Exit => Execute::Exit,
                Commands::EOF => Execute::Cancel,
                Commands::Newline => Execute::Command(self.text_field.text().to_string()),
                special if let Some(key) = special.get_key() => { match key {
                    SpecialKey::Up => self.scroll_history(1)?,
                    SpecialKey::Down => self.scroll_history(-1)?,
                    SpecialKey::Tab => self.complete_next(SelectionDirection::Down)?,
                    SpecialKey::ShiftTab => self.complete_next(SelectionDirection::Up)?,
                }; return Ok(None) }
                e => unreachable!("Unknown key: {:?}", e)
            })),
            Some(completion_info) => match response.commands {
                Commands::None => return Ok(None),
                Commands::EOF | Commands::Exit => { self.completion.clear()?; return Ok(None) },
                Commands::Newline => {
                    // Accept completion
                    let word_count = char_count(Self::word_at_cursor(&self.text_field));
                    self.text_field.move_left(word_count as u32);
                    self.text_field.erase_rest();
                    let response = self.text_field.handle_input(completion_info.item.to_str().unwrap());
                    // Prevents special characters in complete prompts from being interpreted
                    self.completion.clear()?;
                    self.handle_response(response)
                },
                special if let Some(key) = special.get_key() => { match key {
                    SpecialKey::Down |
                    SpecialKey::Tab => self.complete_next(SelectionDirection::Down)?,
                    SpecialKey::Up |
                    SpecialKey::ShiftTab => self.complete_next(SelectionDirection::Up)?,
                }; return Ok(None) }
                e => unreachable!("Unknown key: {:?}", e)
            }
        }
    }

    pub fn read_line(&mut self) -> YshResult<Execute> {
        let termsize = cursor::terminal_size()?;
        let pos = cursor::get_cursor_pos()?;
        self.text_field.clear();
        self.text_field.set_bounds(termsize - pos);
        let mut c = [0u8; 4];
        let r = loop {
            let buf = Self::aligned_read(&mut c)?;
            let response = self
                .text_field
                .handle_input(std::str::from_utf8(&buf).unwrap());
            if let Some(execute) = self.handle_response(response)? {
                break execute;
            }
        };
        if let Execute::Command(ref line) = r {
            self.history.push(line);
        }
        self.history.unselect();
        write(b"\r\n\x1b[J")?;
        Ok(r)
    }
}
