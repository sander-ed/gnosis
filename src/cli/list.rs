use anyhow::Result;

use crate::workspace::Workspace;

/// List packages published by configured sources.
#[derive(clap::Args)]
pub(super) struct Args {}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.list()
    }
}
