use std::{ffi::{OsStr, OsString}, cell::RefCell, rc::Rc};

use bstr::{BString, ByteVec};
use color_eyre::eyre::{eyre, ContextCompat};
use derive_more::From;

use crate::Shell;

pub type Args<'a, 'b: 'a> = &'a [&'b str];
pub type Result = color_eyre::Result<()>;
pub trait BuiltinFn = Fn(&mut Shell, Args)->Result;
// pub type BuiltinFn = Fn(&mut Shell, Args)->Result;

type ClosureHolder = Rc<dyn BuiltinFn + 'static>;

#[derive(Clone)]
pub enum Action {
    Closure(ClosureHolder),
    Alias{ cmd: String, extra_args: Vec<String> },
    Fn(fn(&mut Shell, Args)->Result),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            Self::Closure(_) => "Closure",
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
            Self::Fn(_) | Self::Closure(_) => write!(f, "<builtin>"),
        }
        
    }
}

impl Action {
    pub fn call(&self, shell: &mut Shell, args: Args)->Result {
        if shell.builtin_recursive_count >= 16 {
            shell.builtin_recursive_count = 0;
            return Err(eyre!("Too many layers deep!"));
        }
        match self {
            Self::Closure(c) => c(shell, args),
            Self::Fn(f) => f(shell, args),
            Self::Alias{cmd, extra_args} => {
                let mut new_args = extra_args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                new_args.extend_from_slice(args);
                shell.builtin_recursive_count += 1;
                let r = shell.execute(cmd, &new_args);
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
    pub fn new_closure(name: String, action: impl BuiltinFn + 'static ) -> Self {
        Self { action: Action::Closure(Rc::new(action)), name }
    }
    pub fn new_fn(name: String, action: fn(&mut Shell, Args)->Result) -> Self {
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

/* Functions that implement the aliases themselves: */

pub fn cd(shell: &mut Shell, args: Args) -> Result {
    let path = args.get(0).map(|s| String::from(*s)).unwrap_or_else(|| get_home());
    if let Err(e) = shell.change_directory(&path) {
        return Err(eyre!("'{:?}': {}", path, e))?;
    }
    Ok(())
}

pub fn exit(shell: &mut Shell, args: Args) -> Result {
    let code = args.get(0)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    shell.exit(code);
    Ok(())
}

pub fn alias(shell: &mut Shell, args: Args) -> Result {
    // alias
    // print all aliases
    if args.len() == 0 {
        for builtin in shell.builtins.values() {
            shell_println!("{}", builtin);
        }
    }
    for arg in args {
        match arg.split_once('=') {
            // alias name=cmd
            // Creates aliases
            Some((name, cmd)) => {
                let mut args = Shell::split_whitespace(cmd)?;
                let cmd = args.remove(0);
                shell.register_builtin(Builtin::new_alias(name.to_owned(), cmd, args));
            },
            // alias name
            // Print alias if it exists
            None => {
                if let Some(builtin) = shell.builtins.get(*arg) {
                    shell_println!("{}", builtin);
                }
            },
        }
    }
    Ok(())
}

pub fn set_pos(shell: &mut Shell, args: Args) -> Result {
    let x: u8 = ensure_arg!(args, 0).parse()?;
    let y: u8 = ensure_arg!(args, 1).parse()?;
    crate::write(&crate::read_line::cursor::set_position(x, y))?;
    Ok(())
}

pub fn exec(shell: &mut Shell, args: Args) -> Result {
    let cmd = ensure_arg!(args, 0);
    let args = &args[1..];
    shell.execute_program(cmd, args)?;
    shell.exit(0);
    Ok(())
}

pub fn r(shell: &mut Shell, _args: Args) -> Result {
    exec(shell, &["cargo", "run"])
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
    exec,
    set_pos,
    r
);

