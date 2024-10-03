use glam::UVec2;

use crate::utils::BytesBuf;
use crate::widget::GridStyle;
use crate::{widget, write};
use crate::utils;

use std::io::Result as IoResult;

use self::files::FileProvider;

use super::cursor;

mod files;

use bstr::{BString, ByteSlice};

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

#[derive(Default, Debug, Copy, Clone)]
struct Selection {
    index: u8,
    items_shown: u8,
    word_hash: u64,
}

impl Selection {
    fn new(current_word: &str) -> Selection {
        Selection {
            word_hash: utils::hash(current_word),
            items_shown: 1,
            index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionInfo {
    item: BString,
}

impl CompletionInfo {
    pub fn item(&self) -> &str {
        self.item.to_str().unwrap()
    }
}

#[derive(Default, Debug)]
pub struct Completer {
    current_selection: Option<Selection>,
    file_provider: FileProvider,
}

impl Completer {
    fn present(&mut self, current_word: &str) -> IoResult<()> {
        // Rough caching mechanism to prevent recomputing the completion everytime
        self.current_selection = self
            .current_selection
            .take()
            .filter(|sel| sel.word_hash == utils::hash(current_word));
        let current_selection = match self.current_selection {
            Some(ref mut sel) => sel,
            None => {
                self.file_provider.provide(current_word)?;
                self.current_selection.insert(Selection::new(current_word))
            }
        };
        let pos = cursor::get_cursor_pos()?;
        let size = cursor::terminal_size()?;
        let items = self.file_provider.items();
        let response = widget::grid(pos, size, items, current_selection.index, GridStyle::default());
        current_selection.items_shown = response.elements_shown;
        write(&response.response)?;
        Ok(())
    }
    pub fn next(&mut self, current_word: &str, direction: SelectionDirection) -> IoResult<()> {
        if let Some(ref mut selection) = self.current_selection {
            let Selection { index: index_ref, items_shown, .. } = selection;
            let items_shown = *items_shown;
            let index = *index_ref;
            *index_ref = match direction {
                SelectionDirection::Down => if index < items_shown-1 {
                    index+1
                } else {
                     0
                },
                SelectionDirection::Up => if index > 0 {
                    index-1
                } else {
                    items_shown-1
                }
            }
        }
        self.present(current_word)
    }
    pub fn current_completion(&self) -> Option<CompletionInfo> {
        let current_selection = self.current_selection.as_ref()?;
        let items = self.file_provider.items();
        let item = self.file_provider.accept(items.get(current_selection.index as usize)?);
        Some(CompletionInfo { item })
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
