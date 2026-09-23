mod add;
mod check;
mod index;
mod init;
mod list;
mod new;
mod package;
mod propose;
mod source;
mod sync;

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
    Init(init::Args),
    Package(package::Args),
    New(Box<new::Args>),
    Source(source::Args),
    List(list::Args),
    Add(add::Args),
    Sync(sync::Args),
    Check(check::Args),
    Index(index::Args),
    Propose(propose::Args),
}

impl Cli {
    pub(crate) fn run(self) -> Result<()> {
        let workspace = Workspace::open(&self.directory)?;
        match self.command {
            Command::Init(args) => args.run(&workspace),
            Command::Package(args) => args.run(&workspace),
            Command::New(args) => args.run(&workspace),
            Command::Source(args) => args.run(&workspace),
            Command::List(args) => args.run(&workspace),
            Command::Add(args) => args.run(&workspace),
            Command::Sync(args) => args.run(&workspace),
            Command::Check(args) => args.run(&workspace),
            Command::Index(args) => args.run(&workspace),
            Command::Propose(args) => args.run(&workspace),
        }
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
