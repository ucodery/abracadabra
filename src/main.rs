use clap::Parser;

mod action;
mod env;
mod run;
mod path_match;

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
    let cur_dir = std::env::current_dir()?;

    let env = env::EnvBuilder::new().from_fs(cur_dir);

    let runner = run::CommandLevel::new(!cli.dry_run, cli.verbose, cli.quiet);

    if env.build(!cli.dry_run)? {
        env.setup(runner)?
    } else {
        eprintln!("Found no necessary dev environment to create");
    }
    Ok(())
}
