use nix::sys::termios::{
    self, InputFlags, LocalFlags, OutputFlags, SpecialCharacterIndices, Termios,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TermState {
    old: Option<Termios>,
    new: Option<Termios>,
}

impl TermState {
    pub fn new(current: Termios) -> Self {
        let mut new = current.clone();
        new.input_flags &= !(InputFlags::BRKINT
            | InputFlags::BRKINT
            | InputFlags::ICRNL
            | InputFlags::INPCK
            | InputFlags::ISTRIP
            | InputFlags::IXON);
        new.output_flags &= !OutputFlags::OPOST;
        new.local_flags &=
            !(LocalFlags::ECHO | LocalFlags::IEXTEN | LocalFlags::ICANON | LocalFlags::ISIG);
        new.control_chars[SpecialCharacterIndices::VMIN as usize] = 0;
        new.control_chars[SpecialCharacterIndices::VTIME as usize] = 1;
        Self {
            new: Some(new), old: Some(current)
        }
    }
    fn put_termios(termios: &Option<Termios>) -> nix::Result<()> {
        if let Some(termios) = termios.as_ref() {
            return termios::tcsetattr(nix::libc::STDIN_FILENO, termios::SetArg::TCSANOW, termios)
        }
        Ok(())
    }
    pub fn put_new(&self) -> nix::Result<()> {
        Self::put_termios(&self.new)
    }
    pub fn put_old(&self) -> nix::Result<()> {
        Self::put_termios(&self.old)
    }
}
