use anyhow::{Result, ensure};

use crate::workspace::Workspace;

/// Install a package and its transitive dependencies.
#[derive(clap::Args)]
pub(super) struct Args {
    /// Package name or SOURCE/NAME from gnosis list.
    name: String,
    /// Git repository alias; inferred when the package has a unique source.
    #[arg(long)]
    source: Option<String>,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        let (name, source) = super::package_selector(&self.name);
        ensure!(
            source.is_none() || self.source.is_none() || source == self.source.as_deref(),
            "package source {source:?} conflicts with --source {:?}",
            self.source
        );
        workspace.add(name, source.or(self.source.as_deref()))
    }
}
