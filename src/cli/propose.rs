use std::path::PathBuf;

use anyhow::Result;

use crate::workspace::Workspace;

/// Prepare a local source branch with this package's changes staged; never push.
#[derive(clap::Args)]
pub(super) struct Args {
    name: String,
    #[arg(long)]
    output: PathBuf,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.propose(&self.name, &self.output)
    }
}
