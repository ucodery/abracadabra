use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use clap::Parser;
use glob::Pattern;
use indexmap::IndexSet;
use walkdir::WalkDir;

#[derive(PartialEq, Eq, Hash)]
enum PathMatch {
    Extension(OsString),
    Name(OsString),
    Glob(Pattern),
}

impl PathMatch {
    fn matches(&self, path: &Path) -> bool {
        match self {
            PathMatch::Extension(ext) => {
                if let Some(e) = path.extension() {
                    e == ext
                } else {
                    false
                }
            }
            PathMatch::Name(ful) => {
                if let Some(f) = path.file_name() {
                    f == ful
                } else {
                    false
                }
            }
            PathMatch::Glob(glb) => glb.matches_path(path),
        }
    }
}

#[derive(Clone)]
enum Action {
    EnvBuild(String),
    EnvRun(String),
    Skip,
}

fn c_actions() -> Vec<Action> {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=c-cpp\"",
    ))]
}

fn go_actions() -> Vec<Action> {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=go\"",
    ))]
}

fn perl_actions() -> Vec<Action> {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=perl\"",
    ))]
}

fn python_actions() -> Vec<Action> {
    vec![
        Action::EnvBuild(String::from(
            "use flake \"github:the-nix-way/dev-templates?dir=python\"",
        )),
        Action::EnvBuild(String::from("layout python")),
    ]
}

fn python_dev_actions() -> Vec<Action> {
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
        .collect::<Vec<_>>()
}

fn rust_actions() -> Vec<Action> {
    vec![Action::EnvBuild(String::from(
        "use flake \"github:the-nix-way/dev-templates?dir=rust\"",
    ))]
}

fn make_actions() -> HashMap<PathMatch, Vec<Action>> {
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
    .collect::<HashMap<_, _>>()
}

fn build_actions(
    actions: HashMap<PathMatch, Vec<Action>>,
    fs_start: &dyn AsRef<Path>,
) -> (Vec<String>, Vec<String>) {
    let mut build_steps = IndexSet::new();
    let mut run_steps = IndexSet::new();

    let mut walk = WalkDir::new(fs_start).into_iter();
    loop {
        let entry = match walk.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(entry)) => entry,
        };
        let mut stop_recurse = false;
        let mut build_additions = Vec::new();
        let mut run_additions = Vec::new();

        for (path_match, actions) in actions.iter() {
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

    (
        build_steps.into_iter().collect::<Vec<_>>(),
        run_steps.into_iter().collect::<Vec<_>>(),
    )
}

fn finish_env(
    commands: Vec<String>,
    root: &Path,
    execute: bool,
    verbose: bool,
    quiet: bool,
) -> std::io::Result<()> {
    let here = root.to_str().ok_or(std::io::Error::new(
        std::io::ErrorKind::Other,
        "non-utf-8 pwd",
    ))?;
    try_command(
        &("direnv allow ".to_owned() + here),
        true,
        execute,
        verbose,
        quiet,
    )?;
    for cmd in commands {
        try_command(&cmd, false, execute, verbose, quiet)?;
    }
    Ok(())
}

fn try_command(
    cmd: &str,
    check: bool,
    execute: bool,
    verbose: bool,
    quiet: bool,
) -> std::io::Result<()> {
    let mut parts = cmd.split(" ");

    let Some(program) = parts.next() else {
        return Ok(());
    };
    let mut command = &mut Command::new(program);
    command = command.args(parts.collect::<Vec<_>>());

    if execute {
        let status = if quiet {
            command.output()?.status
        } else {
            if verbose {
                eprintln!("Executing: {:?}", command);
            }
            command.status()?
        };
        if !status.success() {
            if verbose {
                eprintln!(
                    "Command execution failed [{}]",
                    status
                        .code()
                        .map(|s| s.to_string())
                        .unwrap_or(" ".to_owned())
                );
            }
            if check {
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

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    dry_run: bool,

    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    #[arg(short, long)]
    quiet: bool,
}

fn main() -> std::io::Result<()> {
    // TODO: look for .git; walk up too
    // TODO: use gitignore
    let cli = Cli::parse();
    let cur_dir = env::current_dir()?;

    let actions = make_actions();
    let (build_steps, run_steps) = build_actions(actions, &cur_dir);

    if !build_steps.is_empty() {
        let envrc = cur_dir.join(".envrc");
        if let Ok(true) = envrc.try_exists() {
            let msg = format!("Existing {} in the way", envrc.display());
            if cli.dry_run {
                eprint!("Would error: ");
                eprintln!("{}", msg);
            } else {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, msg));
            }
        }

        let format = build_steps.join("\n");
        if cli.dry_run {
            eprintln!("Would create new file: {}", envrc.display());
            eprintln!("Would write out contents:\n{}\n", format);
        } else {
            eprintln!("Creating new file: {}", envrc.display());
            let mut envrc = File::create(envrc)?;
            envrc.write_all(format.as_bytes())?;
        }

        finish_env(run_steps, &cur_dir, !cli.dry_run, cli.verbose, cli.quiet)?;
    } else if cli.verbose {
        eprintln!("Found no Actions necessary, not creating a dev environment");
    }
    Ok(())
}
