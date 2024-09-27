use std::{borrow::Cow, collections::hash_map::Entry};

use color_eyre::eyre::eyre;

use crate::{Shell, command::Command};

pub type Result = color_eyre::Result<()>;

#[derive(Clone)]
pub enum Action {
    Alias{ cmd: String, extra_args: Vec<String> },
    Fn(fn(&mut Shell, Command)->Result),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            Self::Fn(_) => "Fn",
            Self::Alias{ .. } => "Alias"
        };
        write!(f, "Action::{:?}{{ {} }}", variant, match self { Self::Alias{cmd,..} => cmd.as_ref(), _ => "..." })
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alias{cmd, extra_args} => write!(f, "{} {}", cmd, extra_args.join(" ")),
            Self::Fn(_) => write!(f, "<builtin>"),
        }
        
    }
}

impl Action {
    pub fn call(&self, shell: &mut Shell, command: Command)->Result {
        if shell.builtin_recursive_count >= 16 {
            shell.builtin_recursive_count = 0;
            return Err(eyre!("Too many layers deep!"));
        }
        match self {
            Self::Fn(f) => f(shell, command),
            Self::Alias{cmd, extra_args} => {
                let mut args = extra_args.clone();
                args.extend_from_slice(&command.args);

                let cmd = Command {command: cmd.clone(), args, ..command};
                shell.builtin_recursive_count += 1;
                let r = shell.execute(cmd);
                shell.builtin_recursive_count = 0;
                r
            },
        }
    }
}

#[derive(Debug)]
pub struct Builtin {
    pub action: Action,
    pub name: String,
}

impl Builtin {
    pub fn new_fn(name: String, action: fn(&mut Shell, Command)->Result) -> Self {
        Self { action: Action::Fn(action), name }
    }
    pub fn new_alias(name: String, cmd: String, extra_args: Vec<String>) -> Self {
        Self { action: Action::Alias{cmd, extra_args}, name }
    }
}

impl std::fmt::Display for Builtin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.name, self.action)
    }
}

pub fn get_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .expect("could not get username")
}

pub fn get_home() -> String {
    std::env::var("HOME")
        .unwrap_or_else(|_|{
            let mut home = String::default();
            #[cfg(not(any(target_os = "macos")))]
            home.push_str("/home/");
            #[cfg(target_os = "macos")]
            get_home.push_str("/Users/");
            #[cfg(not(target_os = "haiku"))]
            {
                home.push_str(&get_username());
            }
            home
        }
    )
}

macro_rules! ensure_arg {
    ($args:expr, $n:expr) => {
        match $args.get($n) {
            Some(arg) => arg,
            None => return Err(eyre!("Missing argument")),
        }
    };
}

/* Functions that implement the builtins themselves: */

/// Change current directory
pub fn cd(shell: &mut Shell, command: Command) -> Result {
    let path = command.args.get(0)
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(get_home()));
    if let Err(e) = shell.change_directory(path.as_str()) {
        return Err(eyre!("'{:?}': {}", path, e))?;
    }
    Ok(())
}

/// Quits the shell
pub fn exit(shell: &mut Shell, command: Command) -> Result {
    let args = command.args;
    let code = args.get(0)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    shell.exit(code);
    Ok(())
}

/// Lists, creates or deletes aliases
pub fn alias(shell: &mut Shell, command: Command) -> Result {
    let args = command.args;
    // usage: alias
    // print all aliases
    if args.len() == 0 {
        for builtin in shell.builtins.values() {
            shell_println!("{}", builtin);
        }
    }
    for arg in args {
        match arg.split_once('=') {
            Some((name, cmd)) => {
                if cmd.is_empty() {
                    // usage: alias name=
                    // Delete alias
                    match shell.builtins.entry(name.to_owned()) {
                       Entry::Occupied(b) if matches!(b.get().action, Action::Alias{..}) => { b.remove(); },
                        _ => shell_println!("Alias '{}' not found.", name),
                    }
                } else {
                    // usage: alias name=cmd
                    // Creates aliases
                    let mut args = shell_word_split::split(cmd)?;
                    let cmd = args.remove(0);
                    shell.register_builtin(Builtin::new_alias(name.to_owned(), cmd, args));
                }
            },
            // usage: alias name
            // Print alias if it exists
            None => {
                if let Some(builtin) = shell.builtins.get(&arg) {
                    shell_println!("{}", builtin);
                } else {
                    shell_println!("\"{}\" is not an alias", arg)
                }
            },
        }
    }
    Ok(())
}

/// Debug command to set the cursor position on-screen
pub fn set_pos(_shell: &mut Shell, command: Command) -> Result {
    let args = command.args;
    let x: u8 = ensure_arg!(args, 0).parse()?;
    let y: u8 = ensure_arg!(args, 1).parse()?;
    crate::write(&crate::read_line::cursor::set_position(x, y))?;
    Ok(())
}

/// Executes a program and exits
pub fn exec(shell: &mut Shell, command: Command) -> Result {
    shell.execute_program(command.shift())?;
    shell.exit(0);
    Ok(())
}

/// Debug command to recompile the shell and run it
pub fn r(shell: &mut Shell, command: Command) -> Result {
    exec(shell, Command{ command: String::new(), args: vec!["cargo".to_string(), "run".to_string()], ..command })
}

/// Executes a file as a shell script
pub fn source(shell: &mut Shell, command: Command) -> Result {
    let args = command.args;
    let path = ensure_arg!(args, 0);
    let path = std::path::Path::new(path);
    shell.source_file(path)?;
    Ok(())
}

/// Run a command without triggering a builtin
pub fn command(shell: &mut Shell, command: Command) -> Result {
    shell.execute_program(command.shift())?;
    Ok(())
}

pub fn export(shell: &mut Shell, command: Command) -> Result {
    for arg in command.args {
        match arg.split_once('=') {
            Some((name, val)) => std::env::set_var(name, val),
            None => {
                let name = arg;
                if let Some(v) = shell.get_var(&name) {
                    std::env::set_var(name, v);
                }
            },
        }
    }
    Ok(())
}

macro_rules! register_builtins {
    ($($name:ident),*) => {
        pub fn native_builtins() -> std::collections::HashMap<String, Builtin> {
            [
                $(Builtin::new_fn(stringify!($name).to_string(), $name)),*
            ].into_iter()
                .map(|b| (b.name.clone(), b))
                .collect()
        
        }
    };
}

register_builtins!(
    cd,
    exit,
    alias,
    command,
    exec,
    set_pos,
    source,
    export,
    r
);

