use anyhow::Result;

use crate::workspace::Workspace;

/// Create a knowledge workspace (does not initialize or commit Git).
#[derive(clap::Args)]
pub(super) struct Args {}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.init()
    }
}
