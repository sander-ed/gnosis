mod git;
mod model;
mod workspace;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(short = 'C', long, default_value = ".")]
    directory: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a knowledge workspace (does not initialize or commit Git).
    Init,
    /// Create a locally authored, published package.
    Package {
        name: String,
        #[arg(long)]
        owner: String,
        #[arg(short = 'd', long, default_value = "")]
        description: String,
    },
    /// Create a local OKF document or navigation directory and refresh indexes.
    New(Box<NewArgs>),
    /// Register a Git repository containing published knowledge packages.
    Source {
        name: String,
        repository: String,
        #[arg(long = "ref", default_value = "HEAD")]
        reference: String,
    },
    /// List packages published by configured sources.
    List,
    /// Install a package and its transitive dependencies.
    Add {
        name: String,
        #[arg(long)]
        source: String,
    },
    /// Restore imported packages at locked commits; use --update for newer knowledge.
    Sync {
        #[arg(long)]
        update: bool,
    },
    /// Check local OKF and package structure without changing knowledge.
    Check,
    /// Refresh local navigation after manual edits; never fetch or change concepts.
    Index,
    /// Prepare a local source branch with this package's changes staged; never push.
    Propose {
        name: String,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Args)]
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
struct NewArgs {
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

fn run(cli: Cli) -> Result<()> {
    let workspace = workspace::Workspace::open(&cli.directory)?;
    match cli.command {
        Command::Init => workspace.init(),
        Command::Package {
            name,
            owner,
            description,
        } => workspace.package(&name, owner, description),
        Command::New(arguments) => {
            let NewArgs {
                package,
                path,
                dir,
                kind,
                title,
                description,
                resource,
                tags,
                sources,
                fields,
                body,
                body_file,
            } = *arguments;
            workspace.new_entry(
                &package,
                &path,
                workspace::NewEntry {
                    directory: dir,
                    metadata: model::ConceptMetadata {
                        kind,
                        title,
                        description,
                        resource,
                        tags,
                        sources,
                        fields,
                    },
                    body,
                    body_file,
                },
            )
        }
        Command::Source {
            name,
            repository,
            reference,
        } => workspace.source(&name, repository, reference),
        Command::List => workspace.list(),
        Command::Add { name, source } => workspace.add(&name, &source),
        Command::Sync { update } => workspace.sync(update),
        Command::Check => workspace.check(),
        Command::Index => workspace.index(),
        Command::Propose { name, output } => workspace.propose(&name, &output),
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
