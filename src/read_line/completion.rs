use color_eyre::eyre::Context;
use glam::UVec2;

use crate::utils::BytesBuf;
use crate::write;
use crate::{sdbg, shell_println, utils, YshResult};
use std::{
    borrow::Cow, ffi::OsString, fs::DirEntry, io::Result as IoResult, os::unix::prelude::OsStrExt,
    path::Path,
};

use super::cursor;

use bstr::{BString, ByteSlice, ByteVec};

#[derive(Default, Debug)]
pub struct Completer {
    current_selection: Option<Selection>,
}

pub enum SelectionDirection {
    Up,
    Down,
}

#[derive(Default, Debug)]
struct Selection {
    index: u8,
    word_hash: u64,
    items: Vec<BString>,
}

impl Selection {
    fn new(current_word: &str) -> Selection {
        Selection {
            word_hash: utils::hash(current_word),
            index: 0,
            items: provide_files(current_word).unwrap_or_else(|e| vec![format!("{:#}", e).into()]),
        }
    }
}

fn format_filename(entry: DirEntry) -> BString {
    let file_type = entry.file_type().expect("Failed to query file informaton");
    let file_name = entry.file_name();
    let mut file_name = BString::from(Vec::from_os_string(file_name).expect("Got invalid filename"));
    if file_type.is_dir() {
        // Append a slash if it is a directory
        file_name.push(b'/');
    }
    if file_name.find_byteset(b" \t").is_some() {
        // Surround filename with quotes if it contains
        file_name.insert(0, b'"');
        file_name.push(b'"');
    }
    file_name
}

fn provide_files(filter: &str) -> YshResult<Vec<BString>> {
    let folder = Path::new(filter);
    let filename = utils::path_filename(folder).unwrap_or_default();
    let folder = utils::path_parent(folder).unwrap_or(Path::new("."));
    let mut items: Vec<_> = std::fs::read_dir(folder)
        .wrap_err(folder.display().to_string())?
        .filter_map(Result::ok)
        .map(format_filename)
        .filter(|f| f.starts_with(filename.as_bytes()))
        .collect();
    items.sort();
    Ok(items)
}

#[derive(Debug, Clone, Copy)]
pub struct CompletionInfo<'a> {
    pub item: &'a BString,
    pub total_items: usize,
}

fn paint_selected(text: &[u8]) -> Vec<u8> {
    [b"\x1b[7m", text, b"\x1B[0m"].concat()
}

const MAX_ROWS: usize = 6;

fn index_safe(items_len: usize, index: u8) -> Option<usize> {
    if items_len > 0 {
        Some(index as usize % MAX_ROWS.min(items_len))
    } else {
        None
    }
}

fn suggest_completions<T: AsRef<[u8]> + std::fmt::Debug>(
    pos: UVec2,
    items: &[T],
    selected: u8,
) -> Vec<u8> {
    let mut buf = BytesBuf::of([cursor::kill_to_term_end()]);
    if let Some(selected) = index_safe(items.len(), selected) {
        buf.extend(items.iter().take(MAX_ROWS).map(T::as_ref).enumerate().map(|(i, item)| {
            if i == selected {
                Cow::Owned(paint_selected(item))
            } else {
                Cow::Borrowed(item)
            }
        }));
    } else {
        buf.push_slice(b"No matches");
    }
    let mut buf = buf.join("\r\n".as_bytes());
    buf.push(b'\r');
    buf.extend_from_slice(&cursor::move_up(items.len().clamp(1, MAX_ROWS) as _));
    buf.extend_from_slice(&cursor::move_right(pos.x - 1));
    buf
}

impl Completer {
    fn present(&mut self, current_word: &str) -> IoResult<()> {
        self.current_selection = self
            .current_selection
            .take()
            .filter(|sel| sel.word_hash == utils::hash(current_word));
        let current_selection = self
            .current_selection
            .get_or_insert_with(|| Selection::new(current_word));

        let pos = cursor::get_cursor_pos()?;
        let response = suggest_completions(pos, &current_selection.items, current_selection.index);
        write(&response)?;
        Ok(())
    }
    pub fn next(&mut self, current_word: &str, direction: SelectionDirection) -> IoResult<()> {
        if let Some(ref mut selection) = self.current_selection {
            match direction {
                SelectionDirection::Down => {
                    selection.index = selection.index.wrapping_add(1)
                }
                SelectionDirection::Up => {
                    selection.index = selection.index.wrapping_sub(1)
                }
            }
        }
        self.present(current_word)
    }
    pub fn current_completion(&self) -> Option<CompletionInfo> {
        let current_selection = self.current_selection.as_ref()?;
        let total_items = current_selection.items.len();
        let index = index_safe(total_items, current_selection.index)?;
        let item = &current_selection.items[index];
        Some(CompletionInfo {
            item,
            total_items,
        })
            
    }
    pub fn clear(&mut self) -> IoResult<()> {
        self.unselect();
        let UVec2 {x, ..} = cursor::get_cursor_pos()?;
        let mut buf = BytesBuf::of([b"\n\r", cursor::kill_to_term_end()]);
        buf.extend([cursor::move_up(1), cursor::move_right(x-1)]);
        write(&buf.join(b""))?;
        Ok(())
    }
    pub fn unselect(&mut self) {
        self.current_selection = None;
    }
}
