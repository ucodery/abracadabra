use std::process::Command;

pub struct CommandLevel {
    check: bool,
    execute: bool,
    verbose: bool,
    quiet: bool,
}

impl CommandLevel {
    pub fn new(execute: bool, verbose: bool, quiet: bool) -> Self {
        Self {
            check: false,
            execute,
            verbose,
            quiet,
        }
    }

    pub fn check(&self) -> Self {
        Self {
            check: true,
            execute: self.execute,
            verbose: self.verbose,
            quiet: self.quiet,
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
            let status = if self.quiet {
                command.output()?.status
            } else {
                if self.verbose {
                    eprintln!("Executing: {:?}", command);
                }
                command.status()?
            };
            if !status.success() {
                if self.verbose {
                    eprintln!(
                        "Command execution failed [{}]",
                        status
                            .code()
                            .map(|s| s.to_string())
                            .unwrap_or(" ".to_owned())
                    );
                }
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
            eprintln!("Would execute: {:?}", command);
        }
        Ok(())
    }
}
