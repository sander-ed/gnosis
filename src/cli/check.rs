use anyhow::Result;

use crate::workspace::Workspace;

/// Check local structure, or authorize a committed contribution against base policy.
#[derive(clap::Args)]
#[command(
    long_about = "Without options, validate local OKF and package structure.

With --contributor, --base, and --head, check changed package paths against contributor rules from the trusted base commit. This CI mode does not validate document structure or execute files from either revision. The caller must supply the authenticated PR author and trusted base/head revisions; Git author names are not authentication."
)]
pub(super) struct Args {
    /// GitHub login from a trusted CI event (not a Git commit author).
    #[arg(long, requires_all = ["base", "head"])]
    contributor: Option<String>,
    /// Trusted target-branch revision containing the policy to enforce.
    #[arg(long, requires = "contributor")]
    base: Option<String>,
    /// Proposed commit to compare with the base.
    #[arg(long, requires = "contributor")]
    head: Option<String>,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        match (self.contributor, self.base, self.head) {
            (Some(actor), Some(base), Some(head)) => {
                workspace.check_contribution(&actor, &base, &head)
            }
            _ => workspace.check(),
        }
    }
}
