use anyhow::Result;

use crate::workspace::Workspace;

/// Restore imported packages at locked commits; use --update for newer knowledge.
#[derive(clap::Args)]
pub(super) struct Args {
    #[arg(long)]
    update: bool,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.sync(self.update)
    }
}
