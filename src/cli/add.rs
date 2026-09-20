use anyhow::Result;

use crate::workspace::Workspace;

/// Install a package and its transitive dependencies.
#[derive(clap::Args)]
pub(super) struct Args {
    name: String,
    #[arg(long)]
    source: String,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.add(&self.name, &self.source)
    }
}
