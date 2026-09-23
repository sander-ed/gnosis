use std::path::PathBuf;

use anyhow::Result;

use crate::workspace::Workspace;

/// Prepare an imported package's edits for review in its source repository.
#[derive(clap::Args)]
#[command(
    long_about = "Prepare an imported package's local changes in a separate source checkout.

Run from the consuming workspace after editing files under gnosis/NAME. The source is read from gnosis.lock. Gnosis fetches the current source ref, merges your edits against it, and stages only this package on branch gnosis/NAME. Your workspace and lock remain unchanged.

This prepares a proposal; it does not commit, push, or open a pull request. Contributor rules are checked against the current source package policy using your authenticated GitHub CLI account. Locally authored packages use ordinary Git branches and pull requests in their own repository.",
    after_help = "Example:
  gnosis propose ed-sql-prinsipper --output ../sql-proposal
  git -C ../sql-proposal diff --cached

Review the staged changes, then commit, push branch gnosis/ed-sql-prinsipper, and open a pull request in the source repository. If the branch already exists remotely, choose a new name with git branch -m before pushing. Conflicts leave the consuming workspace unchanged; reconcile the reported files and retry."
)]
pub(super) struct Args {
    /// Installed package name or SOURCE/NAME; source must match its lock entry.
    name: String,
    /// New checkout directory, relative to -C; must not exist or be inside gnosis/.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        let (name, source) = super::package_selector(&self.name);
        workspace.propose(name, source, &self.output)
    }
}
