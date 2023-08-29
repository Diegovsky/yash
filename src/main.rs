#![feature(trait_alias)]
#![feature(variant_count)]
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf}, io::Read, borrow::Cow,
};

use bstr::{ByteVec, ByteSlice};

pub type Vec2 = glam::u32::UVec2;

mod read_line;
mod utils;
mod term_state;
mod prompt;
mod config;

pub type YshResult<T> = color_eyre::Result<T>;

#[macro_export]
macro_rules! shell_print {
    ($fmt:expr $(, $expr:expr)* $(,)?) => {{
        let txt = format!($fmt, $($expr),*).replace('\n', "\r\n");
        $crate::write(txt.as_bytes()).expect("Failed to print");
    }};
}

#[macro_export]
macro_rules! shell_println {
    ($fmt:expr $(, $expr:expr)* $(,)?) => {
        $crate::shell_print!(concat!($fmt, "\n") $(, $expr)*)
    };
}

pub fn write(bytes: &[u8]) -> nix::Result<()> {
    if bytes.len() == 0 {
        return Ok(());
    }
    let mut written = 0;
    loop {
        match nix::unistd::write(nix::libc::STDOUT_FILENO, &bytes[written..]) {
            Ok(n) => written += n,
            Err(nix::Error::EAGAIN) => continue,
            Err(e) => break Err(e),
        }
        if written >= bytes.len() {
            break Ok(());
        }
    }
}

fn read(buf: &mut [u8]) -> Result<usize, nix::Error> {
    debug_assert!(buf.len() > 0);
    let n = match nix::unistd::read(nix::libc::STDIN_FILENO, buf) {
        Ok(n) => n,
        Err(nix::errno::Errno::EAGAIN) => 0,
        r => r?,
    };
    Ok(n)
}

mod builtins;

#[derive(Debug, Default)]
pub struct Shell {
    exit_code: Option<i32>,
    cwd: PathBuf,
    term_state: term_state::TermState,
    read_line: read_line::ReadLine,
    vars: HashMap<String, String>,
    builtins: HashMap<String, builtins::Builtin>,
    builtin_recursive_count: usize,
    oneshot_var: Option<(String, String)>
}

impl Shell {
    pub fn init(term_state: term_state::TermState) -> YshResult<Self> {
        let mut this = Self {
            term_state,
            builtins: builtins::native_builtins(),
            ..Default::default()
        };
        this.change_directory(".")?;
        this.term_state.put_new()?;
        Ok(this)
    }
    pub fn register_builtin(&mut self, builtin: builtins::Builtin) {
        self.builtins
            .insert(builtin.name.to_string(), builtin);
    }

    pub fn change_directory(&mut self, path: impl AsRef<Path>) -> YshResult<()> {
        let path = path.as_ref().canonicalize()?;
        std::env::set_current_dir(&path)?;
        std::env::set_var("CWD", &path);
        self.cwd = path;
        Ok(())
    }

    pub fn execute_program(&mut self, cmd: &str, args: &[&str]) -> std::io::Result<()> {
        self.term_state.put_old()?;
        let result: std::io::Result<()> = (|| {
            let mut process = std::process::Command::new(cmd);
            if let Some(pair) = self.oneshot_var.take() {
                process.env(pair.0, pair.1);
            }
            let mut child = match process.args(args).spawn() {
                Ok(c) => c,
                Err(e) => match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        shell_print!("{}: command not found\n", cmd);
                        return Ok(());
                    }
                    _ => return Err(e),
                },
            };
            child.wait()?;
            Ok(())
        })();
        self.term_state.put_new()?;
        result
    }

    pub fn execute_builtin(&mut self, name: &str, args: &[&str]) -> Option<YshResult<()>> {
        let Some(action) = self.builtins.get(name).map(|b| b.action.clone()) else {
            return None;
        };
        let result = action.call(self, args);
        Some(result)
    }

    pub fn execute(&mut self, cmd: &str, args: &[&str]) -> YshResult<()> {
        if let Some(res) = self.execute_builtin(cmd, args) {
            return res
        } else {
            self.execute_program(cmd, args)?;
        }
        Ok(())
    }

    pub fn exit(&mut self, code: i32) {
        self.exit_code = Some(code);
    }

    pub fn get_prompt(&self) -> String {
        prompt::get_prompt(self)
    }

    pub fn set_var(&mut self, name: String, value: String) {
        self.vars.insert(name, value);
    }
    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }

    pub fn get_var_or_env(&self, name: &str) -> Option<String> {
        self.vars.get(name)
        .cloned()
        .or_else(|| std::env::var(name).ok())
    }

    pub fn expand_vars<'a>(&self, text: &'a str) -> Cow<'a, str> {
        use regex::Regex;
        static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let regex = REGEX.get_or_init(|| {
            Regex::new(r#"\$(\w+)"#).unwrap()
        });
        regex.replace_all(text, move |captures: &regex::Captures| {
            self.get_var_or_env(&captures[1]).unwrap_or_default()
        })
    }

    fn try_command_or_var<'a>(&mut self, iter: &mut impl Iterator<Item = &'a str>) -> Option<&'a str> {
        let parts = iter.next()?.splitn(2, '=').collect::<Vec<_>>();
        if parts.len() == 1 {
            return Some(parts[0])
        }
        let (name, value) = (parts[0].to_string(), parts[1].to_string());
        match iter.next() {
            Some(c) => { 
                self.oneshot_var = Some((name, value));
                Some(c)
            },
            None => {
                self.set_var(name, value);
                return None
            }
        }
    }

    pub fn split_whitespace(text: &str) -> YshResult<Vec<String>> {
        let mut split = shell_words::split(text)?;
        if split.len() == 0 {
            split.push(String::from(""))
        }
        Ok(split)
    }

    pub fn read_line(&mut self) -> YshResult<()> {
        shell_print!("{}", self.get_prompt());
        match self.read_line.read_line()? {
            read_line::Command::Exit => return Ok(()),
            read_line::Command::Execute(program) => {
                let program = self.expand_vars(&program);
                let args = Self::split_whitespace(&program)?;
                let mut args = args.iter().map(|s| s.as_str());
                if let Some(cmd) = self.try_command_or_var(&mut args) {
                    let args = args.collect::<Vec<_>>();
                    self.execute(cmd, &args)?;
                }
            },
            read_line::Command::Cancel => (),
        };
        Ok(())
    }

    pub fn main_loop(&mut self) -> YshResult<()> {
        while self.exit_code.is_none() {
            if let Err(e) = self.read_line() {
                shell_println!("{}", e);
            } 
        }
        Ok(())
    }
    pub fn run(&mut self) -> YshResult<i32> {
        match config::get_history() {
            Ok(history) => self.read_line = read_line::ReadLine::new_with_history(history),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => shell_println!("Failed to open history file: {}", e),
        }

        self.main_loop().expect("Mainloop quit");

        // Exit
        let history_path = config::get_history_file();
        std::fs::create_dir_all(history_path.parent().unwrap())?;
        std::fs::write(history_path, self.read_line.history().join("\n")).expect("Failed to save history");

        self.term_state.put_old().unwrap();
        Ok(self.exit_code.unwrap_or_default())
    }
}

fn main() {
    let old_termios =
        nix::sys::termios::tcgetattr(nix::libc::STDIN_FILENO).expect("Failed to raw terminal");
    std::panic::set_hook({
        let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::new().into_hooks();
        eyre_hook.install().unwrap();
        // This weird hack is needeed necause the `nix` wrapper does not implement `Send`.
        let old_termios: nix::libc::termios = old_termios.clone().into();
        Box::new(move |panic_info| {
            // Hack undone :)
            let old_termios = old_termios.into();
            let _ = nix::sys::termios::tcsetattr(
                nix::libc::STDIN_FILENO,
                nix::sys::termios::SetArg::TCSANOW,
                &old_termios,
            );
            println!("{}", panic_hook.panic_report(panic_info));
        })
    });
    let mut shell = Shell::init(term_state::TermState::new(old_termios)).expect("Failed to init shell");
    std::process::exit(shell.run().unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_shell() -> Shell {
        Shell::init(Default::default()).unwrap()
    }

    #[test]
    fn get_var_or_env() {
        let mut shell = mock_shell();
        shell.set_var("FOO".into(), "fool".into());
        assert_eq!(shell.get_var_or_env("FOO"), Some("fool".into()));
    }

    #[test]
    fn expand_var_simple() {
        let mut shell = mock_shell();
        shell.set_var("FOO".into(), "fool".into());
        assert_eq!(shell.expand_vars("you are a $FOO"), "you are a fool");
    }

    #[test]
    fn expand_var_command_simple() {
        let mut shell = mock_shell();
        shell.set_var("CWD".into(), "/home".into());
        assert_eq!(shell.expand_vars("echo $CWD"), "echo /home");
    }

    #[test]
    fn expand_env_command_simple() {
        let mut shell = mock_shell();
        std::env::set_var("FOO", "fool");
        assert_eq!(shell.expand_vars("echo $FOO"), "echo fool");
    }
}
