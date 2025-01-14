use log::{warn,error};
use std::process::ExitCode;

use clap::Parser;

mod action;
mod env;
mod run;
mod path_match;

#[derive(Parser)]
struct Cli {
    #[arg(long, conflicts_with = "quiet")]
    dry_run: bool,

    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    #[arg(short, long)]
    quiet: bool,
}

fn _main() -> std::io::Result<()> {
    // TODO: look for .git; walk up too
    // TODO: use gitignore
    let cli = Cli::parse();

    let mut log = colog::default_builder();
    if cli.verbose {
        // can specify --verbose --dry-run; check verbose first to get level
        log.filter(None, log::LevelFilter::Debug);
    } else if cli.dry_run {
        log.filter(None, log::LevelFilter::Info);
    } else if cli.quiet {
        log.filter(None, log::LevelFilter::Error);
    } else {
        log.filter(None, log::LevelFilter::Warn);
    }
    log.init();

    let cur_dir = std::env::current_dir()?;

    let env = env::EnvBuilder::new().from_fs(cur_dir);

    let runner = run::CommandControl::new(!cli.dry_run, cli.quiet);

    if env.build(!cli.dry_run)? {
        env.setup(runner)?
    } else {
        warn!("Found no necessary dev environment to create");
    }
    Ok(())
}

fn main() -> ExitCode {
    match _main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{}", error);
            ExitCode::FAILURE
        },
    }
}
