use std::{path::Path, os::unix::prelude::OsStrExt, ffi::OsStr, io::BufRead, borrow::Cow};


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

pub fn hash<V>(value: &V) -> u64
where
    V: std::hash::Hash + ?Sized,
{
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
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

/// Allows to push slices or vecs of bytes into a buffer and join them later.
/// Like a `StringBuilder` but for bytes.
#[derive(Debug, Clone, Default)]
pub struct BytesBuf<'a> {
    buf: Vec<Cow<'a, [u8]>>
}

impl<'a> BytesBuf<'a> {
    /// Creates an empty [`Self`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Creates a [`Self`] with elements inside.
    pub fn of<T: Into<Cow<'a, [u8]>>>(elements: impl IntoIterator<Item = T>) -> Self {
        Self { buf: elements.into_iter().map(T::into).collect() }
    }
    /// Pushes a slice into the end of the buffer.
    /// A conveninece method for pushing byte literals.
    pub fn push_slice(&mut self, item: &'a [u8]) {
        self.buf.push(item.into())
    }
    /// Pushes a vec or slice into the end of the buffer.
    pub fn push<T: Into<Cow<'a, [u8]>>>(&mut self, item: T) {
        self.buf.push(item.into())
    }
    /// Inserts a vec or slice into the buffer at the given index.
    pub fn insert<T: Into<Cow<'a, [u8]>>>(&mut self, index: usize, item: T) -> &mut Self {
        self.buf.insert(index, item.into());
        self
    }
    /// Joins the contents of the buffer into a single vec.
    pub fn join(&self, sep: impl AsRef<[u8]>) -> Vec<u8> {
        self.buf.join(sep.as_ref())
    }
}

impl<'a, T> std::iter::Extend<T> for BytesBuf<'a> where T: Into<Cow<'a, [u8]>> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.buf.extend(iter.into_iter().map(T::into))
    }
}

impl<'a, T> std::iter::FromIterator<T> for BytesBuf<'a> where T: Into<Cow<'a, [u8]>> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self { buf: iter.into_iter().map(T::into).collect() }
    }
}

