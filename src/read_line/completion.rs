use bstr::BStr;
use color_eyre::eyre::Context;
use glam::UVec2;

use crate::{sdbg, utils};
use crate::utils::BytesBuf;
use crate::write;
use std::borrow::Cow;

pub(self) use std::io::Result as IoResult;

use self::files::FileProvider;

use super::cursor;

mod files;

use bstr::{BString, ByteSlice, ByteVec};

#[derive(Default, Debug)]
pub struct Completer {
    current_selection: Option<Selection>,
    file_provider: FileProvider,
}

trait CompletionProvider<'a> {
    type Error: std::error::Error + Send + Sync + 'static;
    type Item: AsRef<[u8]> + 'a;
    fn provide(&mut self, current_word: &str) -> Result<(), Self::Error>;
    fn items(&self) -> &[Self::Item];
    fn accept(&self, item: &Self::Item) -> BString {
        BString::from(item.as_ref())
    }
}

pub enum SelectionDirection {
    Up,
    Down,
}

#[derive(Default, Debug)]
struct Selection {
    index: u8,
    word: String,
    word_hash: u64,
}

impl Selection {
    fn new(current_word: &str) -> Selection {
        Selection {
            word_hash: utils::hash(current_word),
            word: current_word.to_owned(),
            index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionInfo {
    item: BString,
    pub total_items: usize,
}

impl CompletionInfo {
    pub fn item(&self) -> &str {
        self.item.to_str().unwrap()
    }
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
        buf.extend(
            items
                .iter()
                .take(MAX_ROWS)
                .map(T::as_ref)
                .enumerate()
                .map(|(i, item)| {
                    if i == selected {
                        Cow::Owned(paint_selected(item))
                    } else {
                        Cow::Borrowed(item)
                    }
                }),
        );
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
        // Rough caching mechanism to prevent recomputing the completion everytime
        self.current_selection = self
            .current_selection
            .take()
            .filter(|sel| sel.word_hash == utils::hash(current_word));
        let current_selection = match self.current_selection {
            Some(ref sel) => sel,
            None => {
                self.file_provider.provide(current_word)?;
                &*self.current_selection.insert(Selection::new(current_word))
            }
        };
        let pos = cursor::get_cursor_pos()?;
        let items = self.file_provider.items();
        let response = suggest_completions(pos, items, current_selection.index);
        write(&response)?;
        Ok(())
    }
    pub fn next(&mut self, current_word: &str, direction: SelectionDirection) -> IoResult<()> {
        if let Some(ref mut selection) = self.current_selection {
            match direction {
                SelectionDirection::Down => selection.index = selection.index.wrapping_add(1),
                SelectionDirection::Up => selection.index = selection.index.wrapping_sub(1),
            }
        }
        self.present(current_word)
    }
    pub fn current_completion(&self) -> Option<CompletionInfo> {
        let current_selection = self.current_selection.as_ref()?;
        let items = self.file_provider.items();
        let total_items = items.len();
        let index = index_safe(items.len(), current_selection.index)?;
        let item = self.file_provider.accept(&items[index]);
        Some(CompletionInfo { item, total_items })
    }
    pub fn clear(&mut self) -> IoResult<()> {
        self.unselect();
        let UVec2 { x, .. } = cursor::get_cursor_pos()?;
        let mut buf = BytesBuf::of([b"\n\r", cursor::kill_to_term_end()]);
        buf.extend([cursor::move_up(1), cursor::move_right(x - 1)]);
        write(&buf.join(b""))?;
        Ok(())
    }
    pub fn unselect(&mut self) {
        self.current_selection = None;
    }
}
