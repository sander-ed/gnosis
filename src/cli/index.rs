use anyhow::Result;

use crate::workspace::Workspace;

/// Refresh local navigation after manual edits; never fetch or change concepts.
#[derive(clap::Args)]
pub(super) struct Args {}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.index()
    }
}
