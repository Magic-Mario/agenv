mod checksum;
mod commands;
mod lock;
mod manifest;
mod resolver;
mod store;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agenv", version, about = "reproducible agent environments")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create agenv.toml in the current directory
    Init,
    /// Add a skill dependency
    Add {
        name: Option<String>,
        #[arg(long = "source")]
        source: String,
        #[arg(long = "ref")]
        git_ref: Option<String>,
        #[arg(long = "path")]
        path: Option<String>,
    },
    /// Resolve and materialize skills into each harness's skills directory
    Install,
    /// Reconcile skill directories with the manifest (adopt unknown skills)
    Sync {
        /// Delete skills not in the manifest instead of adopting them
        #[arg(long)]
        prune: bool,
    },
    /// Report drift between manifest, lock, and installed content
    Status,
    /// Re-resolve skills to the latest commit of their ref
    Update { name: Option<String> },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init => commands::init::run(),
        Command::Add {
            name,
            source,
            git_ref,
            path,
        } => commands::add::run(
            name.as_deref(),
            &source,
            git_ref.as_deref(),
            path.as_deref(),
        ),
        Command::Install => commands::install::run(),
        Command::Sync { prune } => commands::sync::run(prune),
        Command::Status => commands::status::run(),
        Command::Update { name } => commands::update::run(name.as_deref()),
    };

    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
