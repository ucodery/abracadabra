use std::collections::HashMap;
use log::{debug,info,warn};
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use glob::Pattern;
use indexmap::IndexSet;
use walkdir::WalkDir;

use crate::action::*;
use crate::path_match::PathMatch;
use crate::run::CommandControl;

fn c_actions() -> Actions {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=c-cpp\"",
    ))]
}

fn go_actions() -> Actions {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=go\"",
    ))]
}

fn perl_actions() -> Actions {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=perl\"",
    ))]
}

fn python_actions() -> Actions {
    vec![
        Action::EnvBuild(String::from(
            "use flake \"github:the-nix-way/dev-templates?dir=python\"",
        )),
        Action::EnvBuild(String::from("layout python")),
    ]
}

fn python_dev_actions() -> Actions {
    python_actions()
        .into_iter()
        .chain([
            Action::EnvBuild(String::from(
                "export PYTHONSTARTUP=~/.config/python/config.py",
            )),
            Action::EnvRun(String::from("direnv exec . pip install -U pip")),
            Action::EnvRun(String::from(
                "direnv exec . pip install -r ~/.config/python/requirements.txt",
            )),
            Action::EnvRun(String::from("direnv exec . pip install -e .")),
        ])
        .collect()
}

fn rust_actions() -> Actions {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=rust\"",
    ))]
}

pub struct EnvBuilder(HashMap<PathMatch, Actions>);

impl EnvBuilder {
    pub fn new() -> Self {
        Self(
            [
                (
                    PathMatch::Glob(Pattern::new(".*").unwrap()),
                    vec![Action::Skip],
                ),
                (PathMatch::Extension(OsString::from("c")), c_actions()),
                (PathMatch::Extension(OsString::from("h")), c_actions()),
                (PathMatch::Extension(OsString::from("go")), go_actions()),
                (PathMatch::Name(OsString::from("go.mod")), go_actions()),
                (PathMatch::Name(OsString::from("go.sum")), go_actions()),
                (PathMatch::Name(OsString::from("go.work")), go_actions()),
                (PathMatch::Extension(OsString::from("pl")), perl_actions()),
                (PathMatch::Extension(OsString::from("pm")), perl_actions()),
                (PathMatch::Extension(OsString::from("pod")), perl_actions()),
                (
                    PathMatch::Name(OsString::from("Makefile.pl")),
                    perl_actions(),
                ),
                (PathMatch::Name(OsString::from("Build.pl")), perl_actions()),
                (PathMatch::Name(OsString::from("cpanfile")), perl_actions()),
                (PathMatch::Extension(OsString::from("py")), python_actions()),
                (
                    PathMatch::Extension(OsString::from("pyx")),
                    python_actions(),
                ),
                (
                    PathMatch::Extension(OsString::from("pyw")),
                    python_actions(),
                ),
                (
                    PathMatch::Extension(OsString::from("pyi")),
                    python_actions(),
                ),
                (
                    PathMatch::Extension(OsString::from("ipy")),
                    python_actions(),
                ),
                (
                    PathMatch::Extension(OsString::from("ipynb")),
                    python_actions(),
                ),
                (
                    PathMatch::Name(OsString::from("pyproject.toml")),
                    python_dev_actions(),
                ),
                (
                    PathMatch::Name(OsString::from("setup.py")),
                    python_dev_actions(),
                ),
                (
                    PathMatch::Name(OsString::from("setup.cfg")),
                    python_dev_actions(),
                ),
                (
                    PathMatch::Name(OsString::from("Pipfile")),
                    python_dev_actions(),
                ),
                (PathMatch::Extension(OsString::from("rs")), rust_actions()),
                (
                    PathMatch::Name(OsString::from("Cargo.toml")),
                    rust_actions(),
                ),
            ]
            .into_iter()
            .collect(),
        )
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
