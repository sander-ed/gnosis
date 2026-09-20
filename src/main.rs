mod git;
mod model;
mod workspace;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(short = 'C', long, default_value = ".")]
    directory: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a knowledge workspace (does not initialize or commit Git).
    Init,
    /// Create a locally authored, published package.
    Package {
        name: String,
        #[arg(long)]
        owner: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Register a Git repository containing published knowledge packages.
    Source {
        name: String,
        repository: String,
        #[arg(long = "ref", default_value = "HEAD")]
        reference: String,
    },
    /// List packages published by configured sources.
    List,
    /// Install a package and its transitive dependencies.
    Add {
        name: String,
        #[arg(long)]
        source: String,
    },
    /// Restore locked content, or merge upstream updates into local knowledge.
    Sync {
        #[arg(long)]
        update: bool,
    },
    /// Validate basic OKF structure and package metadata.
    Check,
    /// Generate missing indexes and refresh gnosis-generated indexes.
    Index,
    /// Prepare a local source branch with this package's changes staged; never push.
    Propose {
        name: String,
        #[arg(long)]
        output: PathBuf,
    },
}

fn run(cli: Cli) -> Result<()> {
    let workspace = workspace::Workspace::open(&cli.directory)?;
    match cli.command {
        Command::Init => workspace.init(),
        Command::Package {
            name,
            owner,
            description,
        } => workspace.package(&name, owner, description),
        Command::Source {
            name,
            repository,
            reference,
        } => workspace.source(&name, repository, reference),
        Command::List => workspace.list(),
        Command::Add { name, source } => workspace.add(&name, &source),
        Command::Sync { update } => workspace.sync(update),
        Command::Check => workspace.check(),
        Command::Index => workspace.index(),
        Command::Propose { name, output } => workspace.propose(&name, &output),
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
