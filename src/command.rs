use crate::{YshResult, shell_println};

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
    /// Shifts the arguments to the left by one, removing the command name
    pub fn shift(mut self) -> Self {
        let command =
            if self.args.len() == 0 {
                String::new()
            } else {
                self.args.remove(0)
            };
        Self {command, ..self}
    }
}

impl crate::Shell {
    pub fn execute_program(&mut self, cmd: Command) -> std::io::Result<()> {
        // This vector holds all spawned processes.
        // We wait on all of them later.
        let mut spawned = vec![];
        let _token = self.term_state.put_old_token()?;
            
        let mut pipeline = cmd.prepare_to_execute()?;
        pipeline.reverse();

        // If there is a oneshot variable, apply it to all commands in the pipeline
        if let Some(pair) = self.oneshot_var.take() {
            for p in pipeline.iter_mut() {
                p.env(&pair.0, &pair.1);
            }
        }

        let result = (|| {
            let mut last_stdout = None;
            for mut p in pipeline {
                // Link last command's stdout with current stdin.
                // This is how pipes are implemented.
                if let Some(stdout) = last_stdout.take() {
                    p.stdin(stdout);
                }

                // Spawn the program
                let name = p.get_program().to_owned();
                let mut child = match p.spawn() {
                    Ok(c) => c,
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::NotFound => {
                            shell_println!("{:?}: command not found", name);
                            return Ok(());
                        }
                        _ => return Err(e)?,
                    },
                };
                last_stdout = child.stdout.take();
                spawned.push(child);
            }
            Ok(())
        })();
        for mut p in spawned {
            // Kill everyone if any of them fails to spawn
            if result.is_err() {
                p.kill().unwrap();
            } else {
                p.wait().unwrap();
            }
        }
        result
    }
}

