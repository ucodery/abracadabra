use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
struct Language {
    name: String,
    present: bool,
    extensions: Vec<OsString>,
    develop: bool,
    build_files: Vec<OsString>,
    env_entry: String,
    dev_env_entry: String,
    post_env_commands: Vec<String>,
}

impl Language {
    fn inspect(&mut self, path: &Path) {
        if let Some(ext) = path.extension() {
            if self.extensions.iter().any(|e| e == ext) {
                self.present = true;
            }
        }

        if let Some(file) = path.file_name() {
            if self.build_files.iter().any(|b| b == file) {
                self.develop = true;
            }
        }
    }

    fn env_needed(&self) -> Option<String> {
        (self.present || self.develop).then_some(self.env_entry.clone())
    }

    fn dev_env_needed(&self) -> Option<String> {
        self.develop.then_some(self.dev_env_entry.clone())
    }

    fn new_env_commands(&self) -> Option<Vec<String>> {
        self.develop.then_some(self.post_env_commands.clone())
    }
}

#[derive(Debug)]
struct Languages {
    c: Language,
    go: Language,
    perl: Language,
    python: Language,
    rust: Language,
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

impl Languages {
    fn find_from_filesystem(fs_start: &dyn AsRef<Path>) -> Self {
        let mut new = Self {
            c: Language {
                name: String::from("C"),
                present: false,
                extensions: vec![OsString::from("c"), OsString::from("h")],
                develop: false,
                build_files: vec![],
                env_entry: String::from("use flake \"github:the-nix-way/dev-templates?dir=c-cpp\""),
                dev_env_entry: String::new(),
                post_env_commands: Vec::new(),
            },
            go: Language {
                name: String::from("Go"),
                present: false,
                extensions: vec![OsString::from("go")],
                develop: false,
                build_files: vec![
                    OsString::from("go.mod"),
                    OsString::from("go.sum"),
                    OsString::from("go.work"),
                ],
                env_entry: String::from("use flake \"github:the-nix-way/dev-templates?dir=go\""),
                dev_env_entry: String::new(),
                post_env_commands: Vec::new(),
            },
            perl: Language {
                name: String::from("Perl"),
                present: false,
                extensions: vec![
                    OsString::from("pl"),
                    OsString::from("pm"),
                    OsString::from("pod"),
                ],
                develop: false,
                build_files: vec![
                    OsString::from("Makefile.pl"),
                    OsString::from("Build.pl"),
                    OsString::from("cpanfile"),
                ],
                env_entry: String::from("use flake \"github:the-nix-way/dev-templates?dir=perl\""),
                dev_env_entry: String::new(),
                post_env_commands: Vec::new(),
            },
            python: Language {
                name: String::from("Python"),
                present: false,
                extensions: vec![
                    OsString::from("py"),
                    OsString::from("pyx"),
                    OsString::from("pyw"),
                    OsString::from("pyi"),
                    OsString::from("ipy"),
                    OsString::from("ipynb"),
                ],
                develop: false,
                build_files: vec![
                    OsString::from("pyproject.toml"),
                    OsString::from("setup.py"),
                    OsString::from("setup.cfg"),
                    OsString::from("Pipfile"),
                ],
                env_entry: String::from(
                    "use flake \"github:the-nix-way/dev-templates?dir=python\"\n\
                     layout python",
                ),
                dev_env_entry: String::from("export PYTHONSTARTUP=~/.config/python/config.py"),
                post_env_commands: vec![
                    String::from("direnv exec . python -m pip install -U pip"),
                    String::from(
                        "direnv exec . python -m pip install ~/.config/python/requirements.txt",
                    ),
                    String::from("direnv exec . python -m pip install -e ."),
                ],
            },
            rust: Language {
                name: String::from("Rust"),
                present: false,
                extensions: vec![OsString::from("rs")],
                develop: false,
                build_files: vec![OsString::from("Cargo.toml")],
                env_entry: String::from("use flake \"github:the-nix-way/dev-templates?dir=rust\""),
                dev_env_entry: String::new(),
                post_env_commands: Vec::new(),
            },
        };
        for entry in WalkDir::new(fs_start)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter_map(|e| e.ok())
        {
            new.c.inspect(entry.path());
            new.go.inspect(entry.path());
            new.perl.inspect(entry.path());
            new.python.inspect(entry.path());
            new.rust.inspect(entry.path());
        }
        new
    }

    fn format_env_file(&self) -> Option<String> {
        let format = vec![
            self.c.env_needed(),
            self.go.env_needed(),
            self.perl.env_needed(),
            self.python.env_needed(),
            self.rust.env_needed(),
            self.c.dev_env_needed(),
            self.go.dev_env_needed(),
            self.perl.dev_env_needed(),
            self.python.dev_env_needed(),
            self.rust.dev_env_needed(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

        Some(format).filter(|s| !s.is_empty())
    }

    fn finish_env(&self) -> std::io::Result<()> {
        Command::new("direnv").args(["allow", "."]).status()?;
        for cmd in self
            .c
            .new_env_commands()
            .iter()
            .chain(
                self.go.new_env_commands().iter().chain(
                    self.perl.new_env_commands().iter().chain(
                        self.python
                            .new_env_commands()
                            .iter()
                            .chain(self.rust.new_env_commands().iter()),
                    ),
                ),
            )
            .flatten()
        {
            try_command(cmd);
        }
        Ok(())
    }
}

fn try_command(cmd: &str) {
    let mut parts = cmd.split(" ");
    let Some(program) = parts.next() else { return };
    let mut process = &mut Command::new(program);
    process = process.args(parts.collect::<Vec<_>>());

    eprintln!("{:?}", process);
    let _ = process.spawn();
    // if !quiet
    // process.status();

    println!("{}", cmd);
}

fn main() -> std::io::Result<()> {
    // TODO: HashMap
    // TODO: capture current_dir once and reuse
    // TODO: look for .git; walk up too
    // TODO: use gitignore
    // TODO: --dry-run
    // TODO: --quiet; --verbose
    let cur_dir = env::current_dir()?;
    let languages = Languages::find_from_filesystem(&cur_dir);
    let format = languages.format_env_file();

    if let Some(format) = format {
        let envrc = Path::new(".envrc");
        if let Ok(true) = envrc.try_exists() {
            eprintln!("Would write:\n{}", format);
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Existing .envrc in the way",
            ));
        }
        eprintln!("Creating new .envrc file");
        let mut envrc = File::create(envrc)?;
        envrc.write_all(format.as_bytes())?;

        languages.finish_env()?;
    }
    Ok(())
}
