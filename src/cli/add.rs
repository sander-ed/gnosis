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
        let (name, source) = match self.name.split_once('/') {
            Some((alias, name)) => {
                ensure!(
                    self.source.as_deref().is_none_or(|source| source == alias),
                    "package source {alias:?} conflicts with --source {:?}",
                    self.source.as_deref().unwrap_or_default()
                );
                (name, Some(alias))
            }
            None => (self.name.as_str(), self.source.as_deref()),
        };
        workspace.add(name, source)
    }
}
