use std::{fs::DirEntry, path::{Path, PathBuf}, os::unix::prelude::OsStrExt, ffi::OsStr};

use bstr::{BString, ByteVec, ByteSlice};
use color_eyre::eyre::Context;

use crate::{utils, YshResult, shell_println};

use super::CompletionProvider;

fn format_filename(entry: DirEntry) -> BString {
    let file_type = entry.file_type().expect("Failed to query file informaton");
    let file_name = entry.file_name();
    let mut file_name = BString::from(Vec::from_os_string(file_name).expect("Got invalid filename"));
    if file_type.is_dir() {
        // Append a slash if it is a directory
        file_name.push(b'/');
    }
    if file_name.find_byteset(b" \t").is_some() {
        // Surround filename with quotes if it contains spaces/tabs
        file_name.insert(0, b'"');
        file_name.push(b'"');
    }
    file_name
}

#[derive(Default, Debug, Clone)]
pub struct FileProvider {
    cwd: PathBuf,
    items: Vec<BString>
}

impl<'a> CompletionProvider<'a> for FileProvider {
    type Error = std::io::Error;
    type Item = BString;
    fn provide(&mut self, current_word: &str) -> Result<(), Self::Error> {
        let folder = Path::new(current_word);
        let filename = utils::path_filename(folder).unwrap_or_default();
        self.cwd = utils::path_parent(folder).unwrap_or(Path::new(".")).into();
        self.items = std::fs::read_dir(&self.cwd)?
            .filter_map(Result::ok)
            .map(format_filename)
            .filter(|f| f.starts_with(filename.as_bytes()))
            .collect();
        self.items.sort();
        Ok(())
    }
    fn items(&self) -> &[Self::Item] {
        &self.items
    }
    fn accept(&self, item: &Self::Item) -> BString {
        if self.cwd == Path::new(".") {
            return item.clone();
        }
        Vec::from_path_buf(self.cwd.join(item.to_os_str().unwrap())).unwrap().into()
    }
}
