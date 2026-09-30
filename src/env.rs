use log::{debug, info, warn};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use indexmap::IndexSet;
use walkdir::WalkDir;

use crate::action::*;
use crate::path_match::PathMatch;
use crate::run::CommandControl;

#[derive(Debug, PartialEq)]
pub struct EnvBuilder(HashMap<PathMatch, Actions>);

impl EnvBuilder {
    pub fn raw_new(hm: HashMap<PathMatch, Actions>) -> Self {
        Self(hm)
    }

    pub fn from_fs(&self, fs_start: PathBuf) -> Env {
        let mut build_steps = IndexSet::new();
        let mut run_steps = IndexSet::new();

        let mut walk = WalkDir::new(&fs_start).into_iter();
        loop {
            let entry = match walk.next() {
                None => break,
                Some(Err(_)) => continue,
                Some(Ok(entry)) => entry,
            };
            let mut stop_recurse = false;
            let mut build_additions = Vec::new();
            let mut run_additions = Vec::new();

            for (path_match, actions) in self.0.iter() {
                if path_match.matches(entry.path()) {
                    for action in actions {
                        match action {
                            Action::Skip => {
                                stop_recurse = true;
                            }
                            Action::EnvBuild(build) => {
                                build_additions.push(build.clone());
                            }
                            Action::EnvRun(run) => {
                                run_additions.push(run.clone());
                            }
                        }
                    }
                }
            }

            if stop_recurse {
                if entry.file_type().is_dir() {
                    walk.skip_current_dir();
                } // else just don't act on the file
            } else {
                for build in build_additions {
                    build_steps.insert(build);
                }
                for run in run_additions {
                    run_steps.insert(run);
                }
            }
        }

        Env {
            root: fs_start,
            build_steps: build_steps.into_iter().collect::<Vec<_>>(),
            run_steps: run_steps.into_iter().collect::<Vec<_>>(),
        }
    }
}

pub struct Env {
    root: PathBuf,
    build_steps: Vec<String>,
    run_steps: Vec<String>,
}

impl Env {
    pub fn build(&self, mutate: bool) -> std::io::Result<bool> {
        if self.build_steps.is_empty() {
            return Ok(false);
        }

        let envrc = self.root.join(".envrc");
        if let Ok(true) = envrc.try_exists() {
            let msg = format!("Existing {} in the way", envrc.display());
            if !mutate {
                warn!("Would error: {}", msg);
            } else {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, msg));
            }
        }

        let format = self.build_steps.join("\n");
        if !mutate {
            info!("Would create new file: {}", envrc.display());
            info!("Would write out contents:\n{}", format);
        } else {
            debug!("Creating new file: {}", envrc.display());
            let mut envrc = File::create(envrc)?;
            envrc.write_all(format.as_bytes())?;
        }
        Ok(true)
    }

    pub fn setup(&self, runner: CommandControl) -> std::io::Result<()> {
        let here = self.root.to_str().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "non-utf-8 pwd",
        ))?;
        runner
            .check()
            .try_command(&("direnv allow ".to_owned() + here))?;
        for cmd in &self.run_steps {
            runner.try_command(cmd)?;
        }
        Ok(())
    }
}
