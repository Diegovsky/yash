use std::{path::Path, os::unix::prelude::OsStrExt, ffi::OsStr};


#[macro_export]
macro_rules! binformat {
    ($($tt:tt)*) => {{
        use ::std::io::Write;
        let mut buf = Vec::with_capacity(16);
        write!(buf, $($tt)*).unwrap();
        buf
    }};
}

pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

pub fn char_at(s: &str, index: usize) -> Option<usize> {
    let (i, _)= s.char_indices().nth(index)?;
    Some(i)
}


pub fn path_parent(path: &Path) -> Option<&Path> {
    if path.as_os_str().as_bytes().ends_with(b"/") {
        return Some(path);
    } else {
        let parent = path.parent();
        if parent == Some(Path::new("")) {
            return None
        }
        parent
    }
}

pub fn path_filename(path: &Path) -> Option<&OsStr> {
    if path.as_os_str().as_bytes().ends_with(b"/") {
        return Some(Default::default());
    } else {
        path.file_name()
    }
}
