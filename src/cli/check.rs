use anyhow::Result;

use crate::workspace::Workspace;

/// Check local OKF and package structure without changing knowledge.
#[derive(clap::Args)]
pub(super) struct Args {}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.check()
    }
}
