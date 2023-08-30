use std::{path::Path, os::unix::prelude::OsStrExt};

use bstr::ByteSlice;

use crate::{read, write, YshResult, Shell, shell_println, shell_print, utils::{path_parent, path_filename, char_count}};

#[derive(Debug, Default)]
pub struct ReadLine {
    history: Vec<String>,
    hist_index: usize,
    suggestion_index: usize,
    current_match: Option<String>,
    current_choice: Option<String>,
    text_field: text_field::TextField,
}

pub mod cursor;
pub mod text_field;
mod parser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Execute {
    Exit,
    Cancel,
    Command(Command),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    command: String,
    args: Vec<String>,
    redir: Option<String>
}

pub fn utf8_byte_len(i: u8) -> Option<u8> {
    if i >= 192 {
        let len =
        if i >> 5 & 1 == 0 { 2 }
        else if i >> 6 & 1 == 0 { 3 }
        else if i >> 7 & 1 == 0 { 4 }
        else { panic!("Invalid utf-8 sequence!") };
        return Some(len)
    }
    None
}

// pub fn parse_line() -> 

impl ReadLine {
    pub fn new_with_history(lines: Vec<String>) -> Self {
        Self {
            history: lines,
            ..Default::default()
        }
    }
    pub fn history(&self) -> &[String] {
        &self.history
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
                return Ok(&c[0..1 + extra])
            } else { continue };
        }
    }

    pub fn scroll_history(&mut self, offset: isize) -> YshResult<()> {
        let new_index = self.hist_index as isize + offset;
        if new_index < 0 || new_index >= self.history.len() as isize {
            return Ok(())
        }
        if self.hist_index == 0 {
            self.history.push(self.text_field.text().to_string());
        }
        self.hist_index = new_index as usize;
        let response = if new_index == 0 {
            self.text_field.set_text(&self.history.pop().unwrap())
        } else {
            self.text_field.set_text(&self.history[self.history.len() - self.hist_index - 1])
        };
        write(&response.bytes)?;
        Ok(())
    }

    pub fn suggest_files(&mut self) -> YshResult<()> {
        let current_line = self.text_field.text();//self.current_match.get_or_insert_with(|| self.text_field.text().to_owned());
        let current_word = Shell::split_whitespace(current_line)?.last().cloned().unwrap_or_default();
        if current_word.is_empty() {
            return Ok(())
        }
        let path = std::path::Path::new(&current_word);
        let file_name = path_filename(path).unwrap_or_default();
        let parent = path_parent(path).unwrap_or(Path::new("."));
        let files: Vec<_> = match std::fs::read_dir(parent) {
            Ok(e) => e.collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e)?,
        };
        let mut entries: Vec<_> = files.into_iter().filter_map(Result::ok).filter(|e| e.file_name().as_bytes().starts_with(file_name.as_bytes())).collect();
        entries.sort_by_cached_key(|entry| entry.file_name());
        let entries: Vec<Vec<u8>> = entries.iter().take(5).map(|e| e.file_name().to_string_lossy().into_owned().into()).collect();
        self.suggest_completions(&entries)
    }

    pub fn suggest_completions<T: AsRef<[u8]>>(&mut self, items: &[T]) -> YshResult<()> {
        let pos = cursor::get_cursor_pos()?;
        if items.len() == 0 {
            return Ok(())
        }
        self.current_choice = Some(items[self.suggestion_index % items.len()].as_ref().to_str_lossy().into_owned());
        let mut buf = vec![cursor::kill_to_term_end()];
        buf.extend(items.iter().map(T::as_ref));
        let lines = buf.len()-1;
        let c = [cursor::bell(), b"no matches"].concat();
        if lines == 0 {
            buf.push(&c);
        }
        let mut buf = buf.join(b"\n\r".as_slice());
        buf.extend_from_slice(cursor::kill_to_term_end());
        buf.extend_from_slice(&cursor::move_up(lines as _));
        buf.push(b'\r');
        buf.extend_from_slice(&cursor::move_right(pos.x-1));
        write(&buf)?;
        Ok(())
    }

    pub fn complete(&mut self) -> YshResult<()> {
        if let Some(sug) = self.current_choice.take() {
            self.text_field.erase_left(char_count(&sug) as u32).write()?;
        }
        self.suggest_files()?;
        self.text_field.handle_input(self.current_choice.as_ref().unwrap()).write()?;
        self.suggestion_index += 1;
        Ok(())
    }

    pub fn accept(&mut self) -> YshResult<()> {
        self.suggestion_index = 0;
        self.text_field.handle_input(&self.current_choice.take().unwrap()).write()?;
        self.current_match = None;
        Ok(())
    }

    pub fn read_line(&mut self) -> YshResult<Execute> {
        let termsize = cursor::terminal_size()?;
        let pos = cursor::get_cursor_pos()?;
        self.text_field.clear();
        self.text_field.set_bounds(termsize - pos);
        let mut c = [0u8; 4];
        let r = loop {
            let buf = Self::aligned_read(&mut c)?;
            let response = self.text_field.handle_input(std::str::from_utf8(&buf).unwrap());
            write(&response.bytes)?;
            match response.commands {
                text_field::Commands::Exit =>break Execute::Exit,
                text_field::Commands::EOF => break Execute::Cancel,
                text_field::Commands::Newline => if self.current_match.is_none() { break Execute::Command(self.text_field.text().to_string()) } else { self.accept()? },
                special if special.get_key().is_some() => {
                    let key = special.get_key().unwrap();
                    match key {
                        text_field::SpecialKey::Up => self.scroll_history(1)?,
                        text_field::SpecialKey::Down => self.scroll_history(-1)?,
                        text_field::SpecialKey::Tab => self.suggest_files()?,
                    }
                }
                _ => (),
            }
        };
        if let Execute::Command(ref line) = r {
            if !line.is_empty() {
                self.history.push(line.clone());
            }
        }
        write(b"\r\n\x1b[J")?;
        Ok(r)
    }
}
