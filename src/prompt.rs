use std::{borrow::Cow, collections::HashMap};

use regex::{Captures, Regex};

use crate::Shell;

struct Prefix(yansi_term::Style);

impl std::fmt::Display for Prefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.write_prefix(f)?;
        Ok(())
    }
}

pub fn replace_colors(text: &str) -> Cow<str> {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let color_regex =
        REGEX.get_or_init(|| Regex::new(r#"%(?<mode>[F])\{#(?<color>[[:xdigit:]]{6})\}"#).unwrap());
    color_regex.replace_all(text, |captures: &regex::Captures| {
        let mode = &captures["mode"];
        match mode {
            "F" => {
                let color = &captures["color"];
                let color = u32::from_str_radix(color, 16).unwrap();
                let get_part = |shift: u32| ((color >> shift) & 0xFF) as u8;
                let color = yansi_term::Color::RGB(get_part(16), get_part(8), get_part(0));
                Prefix(color.normal()).to_string()
            }
            _ => unreachable!(),
        }
    })
}

const DEFAULT_PROMPT: &str = "%F{#ff8080}%n@%m %h%f $ ";

pub fn get_prompt(shell: &Shell) -> String {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = REGEX.get_or_init(|| Regex::new(r#"%([nmhf])\b"#).unwrap());
    let home = crate::builtins::get_home();
    let cwd = shell.cwd.to_string_lossy().replace(&home, "~");
    let username = crate::builtins::get_username();
    let hostname = match nix::unistd::gethostname() {
        Ok(h) => h.to_string_lossy().into_owned(),
        Err(_) => String::from("?"),
    };
    let replaces_table: HashMap<&str, String> = [
        ("n", username),
        ("m", hostname),
        ("h", cwd),
        ("f", String::from("\x1B[0m")),
    ]
    .into_iter()
    .collect();
    let prompt_fmt = shell.get_var("PS1").unwrap_or(DEFAULT_PROMPT);
    let args_replaced = regex.replace_all(&prompt_fmt, |captures: &Captures| {
        &replaces_table[&captures[1]]
    });
    replace_colors(&args_replaced).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_with_red_simple() {
        let text = replace_colors("%F{#FF0000}test");
        assert_eq!(text, "\x1b[38;2;255;0;0mtest");
    }
    #[test]
    fn replace_with_red_medium() {
        let text = replace_colors("%F{#FF0000}Hi, this is a test");
        assert_eq!(text, "\x1b[38;2;255;0;0mHi, this is a test");
    }
    #[test]
    fn replace_fail() {
        let text = replace_colors("%F{#not valid :D} test %f");
        assert_eq!(text, "%F{#not valid :D} test %f");
    }
    #[test]
    fn replace_fail_little() {
        let text = replace_colors("%F{#}test%f");
        assert_eq!(text, "%F{#}test%f");
    }
    #[test]
    fn replace_fail_big() {
        let text = replace_colors("%F{#deadbeef}test%f");
        assert_eq!(text, "%F{#deadbeef}test%f");
    }
    #[test]
    fn replace_mixed() {
        let text =
            replace_colors("%F{#FF0000}I am red!%f%F{#00FF00}I am green!%f%F{#0000FF}I am blue!%f");
        assert_eq!(text, "\x1b[38;2;255;0;0mI am red!%f\x1b[38;2;0;255;0mI am green!%f\x1b[38;2;0;0;255mI am blue!%f");
    }
}
