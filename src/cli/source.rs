use anyhow::Result;

use crate::workspace::Workspace;

/// Register a Git repository containing published knowledge packages.
#[derive(clap::Args)]
pub(super) struct Args {
    /// Local alias for this package repository, such as team.
    name: String,
    /// Git URL or local repository path containing published packages.
    repository: String,
    #[arg(long = "ref", default_value = "HEAD")]
    reference: String,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.source(&self.name, self.repository, self.reference)
    }
}
