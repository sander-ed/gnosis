use anyhow::Result;

use crate::workspace::Workspace;

/// Create a locally authored, published package.
#[derive(clap::Args)]
pub(super) struct Args {
    name: String,
    #[arg(long)]
    owner: String,
    #[arg(short = 'd', long, default_value = "")]
    description: String,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.package(&self.name, self.owner, self.description)
    }
}
