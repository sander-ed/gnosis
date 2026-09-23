mod add;
mod check;
mod new;
mod propose;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::workspace::Workspace;

#[derive(Parser)]
#[command(version, about)]
pub(crate) struct Cli {
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
        #[arg(short = 'd', long, default_value = "")]
        description: String,
    },
    New(Box<new::Args>),
    /// Register a Git repository containing published knowledge packages.
    Source {
        /// Local alias for this package repository, such as team.
        name: String,
        /// Git URL or local repository path containing published packages.
        repository: String,
        #[arg(long = "ref", default_value = "HEAD")]
        reference: String,
    },
    /// List available packages as SOURCE/NAME for use with gnosis add.
    List,
    Add(add::Args),
    /// Remove a package requirement and prune unused imported dependencies.
    Remove {
        /// Local package or direct import; accepts NAME or SOURCE/NAME.
        name: String,
        /// Allow deleting local packages or discarding edits to removed imports.
        #[arg(long)]
        force: bool,
    },
    /// Restore imported packages at locked commits; use --update for newer knowledge.
    Sync {
        #[arg(long)]
        update: bool,
    },
    Check(check::Args),
    /// Refresh local navigation after manual edits; never fetch or change concepts.
    Index,
    Propose(propose::Args),
}

impl Cli {
    pub(crate) fn run(self) -> Result<()> {
        let workspace = Workspace::open(&self.directory)?;
        match self.command {
            Command::Init => workspace.init(),
            Command::Package {
                name,
                owner,
                description,
            } => workspace.package(&name, owner, description),
            Command::New(args) => args.run(&workspace),
            Command::Source {
                name,
                repository,
                reference,
            } => workspace.source(&name, repository, reference),
            Command::List => workspace.list(),
            Command::Add(args) => args.run(&workspace),
            Command::Remove { name, force } => {
                let (name, source) = package_selector(&name);
                workspace.remove(name, source, force)
            }
            Command::Sync { update } => workspace.sync(update),
            Command::Check(args) => args.run(&workspace),
            Command::Index => workspace.index(),
            Command::Propose(args) => args.run(&workspace),
        }
    }
}

fn package_selector(value: &str) -> (&str, Option<&str>) {
    match value.split_once('/') {
        Some((source, name)) => (name, Some(source)),
        None => (value, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_definitions_are_consistent() {
        Cli::command().debug_assert();
    }
}
