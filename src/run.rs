use log::{debug,info};
use std::process::Command;

pub struct CommandControl {
    check: bool,
    execute: bool,
    output: bool,
}

impl CommandControl {
    pub fn new(execute: bool, output: bool) -> Self {
        Self {
            check: false,
            execute,
            output,
        }
    }

    pub fn check(&self) -> Self {
        Self {
            check: true,
            execute: self.execute,
            output: self.output,
        }
    }

    pub fn try_command(&self, cmd: &str) -> std::io::Result<()> {
        let mut parts = cmd.split(" ");

        let Some(program) = parts.next() else {
            return Ok(());
        };
        let mut command = &mut Command::new(program);
        command = command.args(parts.collect::<Vec<_>>());

        if self.execute {
            let status = if self.output {
                command.output()?.status
            } else {
                debug!("Executing: {:?}", command);
                command.status()?
            };
            if !status.success() {
                debug!(
                    "Command execution failed [{}]",
                    status
                        .code()
                        .map(|s| s.to_string())
                        .unwrap_or(" ".to_owned())
                );
                if self.check {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Necessary command failed [{}] {:?}",
                            status
                                .code()
                                .map(|s| s.to_string())
                                .unwrap_or(" ".to_owned()),
                            command
                        ),
                    ));
                }
            }
        } else {
            info!("Would execute: {:?}", command);
        }
        Ok(())
    }
}
