use std::{path::Path, os::unix::prelude::OsStrExt, ffi::OsStr, io::BufRead};


#[macro_export]
macro_rules! binformat {
    ($($tt:tt)*) => {{
        use ::std::io::Write;
        let mut buf = Vec::with_capacity(16);
        write!(buf, $($tt)*).unwrap();
        buf
    }};
}

#[macro_export]
macro_rules! static_regex {
    ($expr:expr) => {{
        use regex::Regex;
        static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        REGEX.get_or_init(|| {
            Regex::new($expr).unwrap()
        })
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

pub fn read_file(p: impl AsRef<std::path::Path>) -> std::io::Result<Vec<String>> {
    let file = match std::fs::File::open(p) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => Err(e)?,
    };
    let st = std::io::BufReader::new(file);
    Ok(st.lines()
        .filter_map(|s| s.ok())
        .filter(|s| !s.is_empty())
        .map(|s| String::from(s))
        .collect())
}


