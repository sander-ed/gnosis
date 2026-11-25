use anyhow::{Result, ensure};

use super::select;
use crate::workspace::Workspace;

/// Install packages and their transitive dependencies.
///
/// Without NAME, or given a repository, choose packages interactively: ←/→ switches
/// source, space toggles, a toggles all, and enter installs the selection, updating
/// already installed packages to the latest commit of their source.
#[derive(clap::Args)]
pub(super) struct Args {
    /// Package name or SOURCE/NAME from gnosis list, or a Git URL or path (./, /) of
    /// a repository to choose packages from. Omit to choose from all sources.
    name: Option<String>,
    /// Git repository alias; inferred when the package has a unique source. With a
    /// repository, the alias to register it under (default: its name).
    #[arg(long, requires = "name")]
    source: Option<String>,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        match self.name {
            Some(name) if !is_repository(&name) => {
                let (name, source) = super::package_selector(&name);
                ensure!(
                    source.is_none() || self.source.is_none() || source == self.source.as_deref(),
                    "package source {source:?} conflicts with --source {:?}",
                    self.source
                );
                workspace.add(name, source.or(self.source.as_deref()))
            }
            repository => {
                ensure!(
                    select::interactive(),
                    "choosing packages needs an interactive terminal; pass SOURCE/NAME from gnosis list instead"
                );
                workspace.add_selected(repository.map(|r| (r, self.source)), select::choose)
            }
        }
    }
}

/// Repositories are URLs or paths; package selectors are NAME or SOURCE/NAME.
fn is_repository(value: &str) -> bool {
    value.contains([':', '\\']) || value.starts_with(['.', '/'])
}

#[cfg(test)]
mod tests {
    use super::is_repository;

    #[test]
    fn repositories_are_distinguished_from_package_selectors() {
        for repository in [
            "https://github.com/my-org/knowledge",
            "git@github.com:my-org/knowledge.git",
            "./knowledge",
            "../knowledge",
            "/srv/knowledge",
            r"C:\knowledge",
        ] {
            assert!(is_repository(repository), "{repository}");
        }
        for selector in ["platform", "team/platform", "team/platform/extra"] {
            assert!(!is_repository(selector), "{selector}");
        }
    }
}
