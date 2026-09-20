use anyhow::Result;

use crate::workspace::Workspace;

/// List available packages as SOURCE/NAME for use with gnosis add.
#[derive(clap::Args)]
pub(super) struct Args {}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.list()
    }
}
