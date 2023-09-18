use crate::YshResult;

use std::process::Stdio;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialAction {
    Redir{ to: String },
    Pipe{ next_command: Box<Command> }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Command {
    pub command: String,
    pub args: Vec<String>,
    pub special_action: Option<SpecialAction>
}

impl Command {
    pub fn prepare_to_execute(self) -> std::io::Result<Vec<std::process::Command>> {
        let mut cmd = std::process::Command::new(self.command);
        cmd.args(self.args);
        match(self).special_action {
            Some(SpecialAction::Redir { to }) => { cmd.stdout(std::fs::File::create(to)?); },
            Some(SpecialAction::Pipe { next_command }) => {
                let mut cmd_string = next_command.prepare_to_execute()?;
                cmd_string.last_mut().unwrap().stdin(Stdio::piped());
                cmd.stdout(Stdio::piped());
                cmd_string.push(cmd);
                return Ok(cmd_string);
            }
            None => (),
        }
        Ok(vec![cmd])
    }
    pub fn parse_args(mut args: Vec<String>) -> YshResult<Self> {
        if args.is_empty() {
            return Ok(Self::default())
        }
        let command = args.remove(0);
        match args.iter().position(|a| a.starts_with(">") || a.starts_with("|")) {
            Some(special_id) => {
                let mut extra_args: Vec<_> = args.drain(special_id..).collect();
                let special = extra_args.remove(0);
                match special.as_bytes()[0] {
                    b'>' => Ok(Command {command, args, special_action: Some(SpecialAction::Redir { to: extra_args.remove(0) })}),
                    b'|' => Ok(Command {command, args, special_action: Some(SpecialAction::Pipe { next_command: Box::new(Command::parse_args(extra_args)?) })}),
                    _ => unreachable!()
                }
            },
            _ => Ok(Command {command, args, special_action: None})
        }
    }
    pub fn parse(line: &str) -> YshResult<Self> {
        Self::parse_args(shell_word_split::split(line)?)
    }
}

