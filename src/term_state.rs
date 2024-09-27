use nix::sys::termios::{
    self, InputFlags, LocalFlags, OutputFlags, SpecialCharacterIndices, Termios,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TermState {
    old: Option<Termios>,
    new: Option<Termios>,
}

pub struct OldStateToken<'a>(&'a TermState);

impl Drop for OldStateToken<'_> {
    fn drop(&mut self) {
        let _ = self.0.put_new().unwrap();
    }
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
            new: Some(new),
            old: Some(current),
        }
    }
    fn put_termios(termios: &Option<Termios>) -> nix::Result<()> {
        if let Some(termios) = termios {
            return termios::tcsetattr(nix::libc::STDIN_FILENO, termios::SetArg::TCSADRAIN, termios);
        }
        Ok(())
    }
    pub fn put_new(&self) -> nix::Result<()> {
        Self::put_termios(&self.new)
    }
    pub fn put_old(&self) -> nix::Result<()> {
        Self::put_termios(&self.old)
    }

    pub fn put_old_token(&self) -> nix::Result<OldStateToken> {
        self.put_old()?;
        Ok(OldStateToken(self))
    }
}

fn get_termios() -> nix::Result<Termios> {
    termios::tcgetattr(nix::libc::STDIN_FILENO)
}

static OLD_TERMIOS: std::sync::OnceLock<nix::libc::termios> = std::sync::OnceLock::new();

pub fn get_termstate() -> TermState {
    let old_termios = get_termios().expect("Failed to get raw terminal");
    // This weird hack is needeed necause the `nix` wrapper does not implement `Send`.
    OLD_TERMIOS.set(old_termios.clone().into()).unwrap();
    TermState::new(old_termios)
}

pub fn restore() {
    if let Some(termios) = OLD_TERMIOS.get() {
        // Hack undone :)
        let old_termios = (*termios).into();
        let _ = nix::sys::termios::tcsetattr(
            nix::libc::STDIN_FILENO,
            nix::sys::termios::SetArg::TCSANOW,
            &old_termios,
        );
    }
}
