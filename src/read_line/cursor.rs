use std::io::Write;
use std::num::NonZeroU32;

use bstr::ByteSlice;
use nix::pty::Winsize;

use crate::{write, read, Vec2, binformat, shell_println};

#[must_use]
pub fn move_left(times: u32) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(16);
    if times != 0 {
        write!(buf, "\x1b[{}D", times).unwrap();
    }
    buf
}

#[must_use]
pub fn move_right(times: u32) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(16);
    if times != 0 {
        write!(buf, "\x1b[{}C", times).unwrap();
    }
    buf
}

#[must_use]
pub fn move_down(times: u32) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(16);
    if times != 0 {
        write!(buf, "\x1b[{}B", times).unwrap();
    }
    buf
}

#[must_use]
pub fn move_up(times: u32) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(16);
    if times != 0 {
        write!(buf, "\x1b[{}A", times).unwrap();
    }
    buf
}

#[must_use]
pub fn set_position(x: u8, y: u8) -> Vec<u8> {
    binformat!("\x1b[{};{}H", y, x)
}

#[must_use]
pub fn kill_line() -> &'static [u8] {
    b"\x1b[K"
}

#[must_use]
pub fn kill_to_term_end() -> &'static [u8] {
    b"\x1b[J"
}

#[must_use]
pub const fn bell() -> &'static [u8] {
    b"\x07"
}

#[must_use]
pub fn get_cursor_pos() -> nix::Result<Vec2> {
    write(b"\x1b[6n")?;
    let mut buf = vec![0u8; 16];
    let mut i = 0;
    loop {
        let read = read(&mut buf[i..])?;
        if read == 0 { continue }
        i += read;
        if let Some(new_i) = buf.find(&b"R") {
            i = new_i;
            break;
        }
    }
    let nums = &buf[2..i]
        .split_str(&b";")
        .map(|i| i.to_str().unwrap())
        .map(|i| i.parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    let x = nums[0];
    let y = nums[1];
    Ok(Vec2::new(y, x))
}

mod ioctl {
    use super::*;
    nix::ioctl_read_bad!(getwinsz, nix::libc::TIOCGWINSZ, Winsize);
}

#[must_use]
pub fn terminal_size() -> nix::Result<Vec2> {
    unsafe {
        let mut winsz = std::mem::MaybeUninit::<Winsize>::uninit();
        ioctl::getwinsz(nix::libc::STDOUT_FILENO, winsz.as_mut_ptr())?;
        let winsz = winsz.assume_init();
        Ok(Vec2::new(winsz.ws_col as u32, winsz.ws_row as u32))
    }
}

