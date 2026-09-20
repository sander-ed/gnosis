use std::path::PathBuf;

use anyhow::Result;

use crate::okf::ConceptMetadata;
use crate::workspace::{NewEntry, Workspace};

/// Create a local OKF document or navigation directory and refresh indexes.
#[derive(clap::Args)]
#[command(after_help = "\
Examples:
  gnosis new platform migration-checklist --type Playbook --description \"Migration steps\"
  gnosis new platform migrations/ -T Playbook -t \"Migrations\" -d \"Migration guidance\"
  gnosis new platform migrations/checklist.md --body-file draft.md
  gnosis new platform findings --body-file -

Files and folders default to type Reference and a title derived from their name.
A trailing / also selects directory mode. Folder metadata lives in index.yml;
its title and description appear in the parent index.md.
No prompts, network access, commits, or publication. Existing paths are never overwritten.")]
pub(super) struct Args {
    /// Existing local or imported package.
    package: String,
    /// Package-relative path; .md is optional for files.
    path: String,
    /// Create a knowledge folder with a generated index instead of a file.
    #[arg(long, conflicts_with_all = ["body", "body_file"])]
    dir: bool,
    /// Knowledge type (default: Reference); custom types are allowed.
    #[arg(short = 'T', long = "type", value_name = "TYPE")]
    kind: Option<String>,
    /// Display title (default: humanized file or folder name).
    #[arg(short = 't', long)]
    title: Option<String>,
    /// Description used in navigation indexes.
    #[arg(short = 'd', long)]
    description: Option<String>,
    /// Canonical resource represented by this concept.
    #[arg(long)]
    resource: Option<String>,
    /// Add a tag; repeat for multiple tags.
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<String>,
    /// Add a provenance source resource; repeat for multiple sources.
    #[arg(long = "source", value_name = "RESOURCE")]
    sources: Vec<String>,
    /// Add a top-level metadata field with a YAML value; repeat for multiple fields.
    #[arg(long = "field", value_name = "KEY=YAML")]
    fields: Vec<String>,
    /// Markdown body text (default: a heading using the title).
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read Markdown body from a workspace-relative file, or - for stdin.
    #[arg(long, value_name = "PATH|-")]
    body_file: Option<PathBuf>,
}

impl Args {
    pub(super) fn run(self, workspace: &Workspace) -> Result<()> {
        workspace.new_entry(
            &self.package,
            &self.path,
            NewEntry {
                directory: self.dir,
                metadata: ConceptMetadata {
                    kind: self.kind,
                    title: self.title,
                    description: self.description,
                    resource: self.resource,
                    tags: self.tags,
                    sources: self.sources,
                    fields: self.fields,
                },
                body: self.body,
                body_file: self.body_file,
            },
        )
    }
}
