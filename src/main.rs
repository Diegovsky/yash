#![feature(trait_alias)]
#![feature(variant_count)]
#![feature(if_let_guard)]
use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
};

use color_eyre::eyre::WrapErr;

pub type Vec2 = glam::u32::UVec2;

mod command;
mod config;
mod prompt;
mod read_line;
mod signals;
mod strings;
mod term_state;
mod utils;

mod debug;

use command::Command;

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
    () => {
        $crate::shell_print!("\n")
    };
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
    signals: signals::Signals,
    oneshot_var: Option<(String, String)>,
}

impl Shell {
    pub fn init(term_state: term_state::TermState) -> YshResult<Self> {
        let mut this = Self {
            term_state,
            builtins: builtins::native_builtins(),
            signals: signals::Signals::init(),
            ..Default::default()
        };
        this.change_directory(".")?;
        this.term_state.put_new()?;
        Ok(this)
    }
    pub fn register_builtin(&mut self, builtin: builtins::Builtin) {
        self.builtins.insert(builtin.name.to_string(), builtin);
    }

    pub fn change_directory(&mut self, path: impl AsRef<Path>) -> YshResult<()> {
        let path = path.as_ref().canonicalize()?;
        std::env::set_current_dir(&path)?;
        std::env::set_var("CWD", &path);
        self.cwd = path;
        Ok(())
    }

    pub fn execute(&mut self, cmd: Command) -> YshResult<()> {
        match self.builtins.get(&cmd.command).map(|b| b.action.clone()) {
            Some(action) => action.call(self, cmd)?,
            None => self.execute_program(cmd)?,
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
        self.vars
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok())
    }

    fn try_command_or_var<'a>(&mut self, mut cmd: Command) -> Option<Command> {
        let parts = cmd.command.splitn(2, '=').collect::<Vec<_>>();
        if parts.len() == 1 {
            return Some(cmd);
        }
        let (name, value) = (parts[0].to_string(), parts[1].to_string());
        if cmd.args.len() == 0 {
            // we got: NAME=VALUE
            self.set_var(name, value);
            None
        } else {
            // we got: NAME=VALUE <command>
            self.oneshot_var = Some((name, value));
            cmd.command = cmd.args.remove(0);
            Some(cmd)
        }
    }

    pub fn execute_line(&mut self, cmd: &str) -> YshResult<()> {
        let cmd = self.expand_vars(&cmd);
        let cmd = Command::parse(&cmd)?;
        let Some(cmd) = self.try_command_or_var(cmd) else {
            return Ok(());
        };
        self.execute(cmd)?;
        Ok(())
    }

    pub fn read_line(&mut self) -> YshResult<()> {
        shell_print!("{}", self.get_prompt());
        match self.read_line.read_line()? {
            read_line::Execute::Exit => return Ok(()),
            read_line::Execute::Command(cmd) => self.execute_line(&cmd)?,
            read_line::Execute::Cancel => (),
        };
        Ok(())
    }

    pub fn main_loop(&mut self) -> YshResult<()> {
        while self.exit_code.is_none() {
            if let Err(e) = self.read_line() {
                shell_println!("{}", e);
            }
            debug::render_debug_text()?;
        }
        Ok(())
    }

    pub fn source_file(&mut self, filename: impl AsRef<Path>) -> YshResult<()> {
        let filename = filename.as_ref();
        let file = std::fs::File::open(filename)
            .wrap_err_with(|| format!("Failed to open file '{}'", filename.display()))?;
        let file = std::io::BufReader::new(file);
        for l in file.lines() {
            let l = l.wrap_err_with(|| format!("Failed to read file '{}'", filename.display()))?;
            self.execute_line(&l)?
        }
        Ok(())
    }
    pub fn run(&mut self) -> YshResult<i32> {
        match config::get_history() {
            Ok(history) => self.read_line = read_line::ReadLine::new_with_history(history),
            Err(e) => shell_println!("Failed to open history file: {}", e),
        }
        match config::get_yashfile() {
            Ok(lines) => {
                for line in lines {
                    match self.execute_line(&line) {
                        Ok(()) => (),
                        Err(e) => {
                            shell_println!("error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => shell_println!("Failed to open history file: {}", e),
        }

        self.main_loop().expect("Mainloop quit");

        // Exit
        let history_path = config::get_history_file();
        std::fs::create_dir_all(history_path.parent().unwrap())?;
        std::fs::write(history_path, self.read_line.history().join("\n"))
            .expect("Failed to save history");

        self.term_state.put_old().unwrap();
        Ok(self.exit_code.unwrap_or_default())
    }
}

fn main() {
    std::panic::set_hook({
        let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::new().into_hooks();
        eyre_hook.install().unwrap();
        Box::new(move |panic_info| {
            term_state::restore();
            println!("{}", panic_hook.panic_report(panic_info));
        })
    });
    let mut shell = Shell::init(term_state::get_termstate()).expect("Failed to init shell");
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
        let shell = mock_shell();
        std::env::set_var("FOO", "fool");
        assert_eq!(shell.expand_vars("echo $FOO"), "echo fool");
    }
}
