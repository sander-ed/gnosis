# Gnosis

**Package and share team knowledge through Git.**

Gnosis is a command-line tool for keeping reusable knowledge alongside your
code. Package your team's engineering standards, playbooks, or reference
material in one Git repository, then install them into the projects that need
them as editable Markdown files.

- **Reuse knowledge across projects.** Install a package and its dependencies
  from a Git repository.
- **Keep versions predictable.** A lock file records the exact upstream commits
  your project uses.
- **Make local changes.** Edit installed knowledge in your project, then merge
  upstream updates when you choose.
- **Contribute improvements back.** Prepare changes for review in the source
  repository using your normal Git and pull request workflow.

Gnosis uses [Open Knowledge Format (OKF) 0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
for structured Markdown knowledge. Your files stay in Git; Gnosis does not
require a separate knowledge server.

## Install

### Homebrew

Install on macOS or Linux with [Homebrew](https://brew.sh):

```sh
brew tap sander-ed/tap https://github.com/sander-ed/sander-ed-tap.git
brew install sander-ed/tap/gnosis
gnosis --help
```

The first command adds the public tap; the second builds Gnosis from source.
Homebrew handles the Rust build toolchain and Git dependency for you.
Installation uses HTTPS; no GitHub account, token, or SSH key is required.

For corporate certificate errors or an older Cargo installation taking
precedence, see the [tap's troubleshooting guide](https://github.com/sander-ed/sander-ed-tap#troubleshooting).

To upgrade:

```sh
brew update
brew upgrade sander-ed/tap/gnosis
```

Prefer to build it yourself? See [Build from source](#build-from-source).

## Quick start

Create your first knowledge package inside an existing Git repository:

```sh
gnosis init
gnosis package platform --owner @my-org/platform --description "Platform knowledge"
gnosis new platform ownership --title "Service ownership"
```

Open `gnosis/platform/ownership.md` and write your first piece of knowledge.
Then refresh navigation, check the package, and commit it:

```sh
gnosis index
gnosis check
git add gnosis.toml gnosis.lock gnosis
git commit -m "Add platform knowledge"
```

Push the repository to share the package. Other projects can then install it
using the steps below.

## How it works

A **workspace** is a project directory with a `gnosis.toml` manifest. A
**package** is a named collection of knowledge under `gnosis/`. A **source** is
a Git repository that publishes packages for other projects to install.

Gnosis tracks installed packages in `gnosis.lock`, including their exact
upstream commits. Installed files are part of your project: you can read, edit,
and commit them like any other Markdown. Updating from upstream is explicit,
and Gnosis merges those updates with your local edits.

Commands operate in the current directory. Use `gnosis -C /path/to/project COMMAND`
to select another workspace; parent directories are not searched.

Only packages listed in a source's `gnosis.toml` are discoverable; imported
dependencies are not republished. Replace `@my-org/platform` in the example with
the person or team responsible for your package. The required `--owner` identifies
that owner; enforce reviews through your repository's access controls and branch
protection.

## Install shared knowledge

Run these commands in the project that will consume the knowledge. A source is
a Git repository that publishes packages; `team` is its local alias, used to
locate content and future updates. A package already authored in the current
workspace is available without installation.

```sh
gnosis init
gnosis source team git@github.com:my-org/knowledge.git --ref main
gnosis list
gnosis add team/platform
```

Copy a `SOURCE/NAME` from `gnosis list` directly into `gnosis add`. You can also
use `gnosis add platform`: it retains an existing source selection or infers the
only configured repository publishing that package. If several sources match,
choose one with `team/platform` or `platform --source team`.

Commit the manifest, lock, and installed knowledge:

```text
gnosis.toml              # Local packages, sources, and direct dependencies
gnosis.lock              # Upstream commits and resolved dependency graph
gnosis/
  index.md
  platform/
    package.toml
    index.md
    ownership.md
.gnosis/                 # Ignored operation locks and transaction files
```

Installed files are editable. The lock records their upstream baseline; local
changes are tracked by your project's Git repository.

## Write knowledge

`new` creates a document or navigation folder inside an existing local or
imported package:

```sh
gnosis new platform checklist --type Playbook --description "Deployment steps"
gnosis new platform operations/ --title "Operations"
gnosis new platform findings --body-file draft.md
```

Document paths may omit `.md`. A trailing `/` or `--dir` creates a folder.
Parent directories are created as needed; existing destinations are never
overwritten. New entries refresh the package's generated indexes.

| Flag | Meaning |
| --- | --- |
| `-T`, `--type TYPE` | Knowledge type; defaults to `Reference` |
| `-t`, `--title TEXT` | Display title; defaults to a humanized file or folder name |
| `-d`, `--description TEXT` | Description shown in navigation |
| `--resource URI` | Resource represented by the concept |
| `--tag TAG` | Tag; repeat for multiple values |
| `--source RESOURCE` | Provenance source; repeat for multiple values |
| `--field KEY=YAML` | Custom metadata, such as `--field 'status=draft'` |
| `--body TEXT` | Literal Markdown body |
| `--body-file PATH` | Read a body from a workspace-relative file; `-` reads stdin |

Metadata goes into document frontmatter or folder `index.yml`. Custom types and
fields are supported; duplicate fields are rejected. Folder titles and
descriptions appear in navigation, but metadata is not inherited by children.

Body input excludes frontmatter and is preserved as supplied. `--body` and
`--body-file` are mutually exclusive and apply only to documents. The default
body is a heading using the title.

You can also edit or create OKF files manually. Run `gnosis index` afterward.
Hand-authored indexes are preserved; add links to them manually when needed.
`index.md` and `log.md` are reserved filenames.

## Sync and contribute

```sh
gnosis sync                   # Restore packages using existing pins
gnosis sync --update          # Fetch newer revisions and merge local changes
git diff
```

Ordinary sync retains pins when source selections are unchanged. Manifest or
source changes trigger dependency resolution. Updates apply to all imported
packages.

Package dependencies are names in `package.toml`. They describe required
knowledge, without version compatibility constraints. Cycles are supported.
If several sources publish a dependency, select one explicitly with
`gnosis add NAME --source SOURCE`. Sources declared inside downloaded manifests
are not followed automatically.

Sync uses a three-way merge of the previous upstream baseline, local content,
and selected upstream revision. Generated indexes are rebuilt after merging.
Conflicts or invalid documents leave the workspace and lock unchanged; resolve
the reported issues before retrying. Unused dependencies with local changes
must be preserved before removal. Deleting an installed directory causes sync
to restore it; remove a direct requirement from `gnosis.toml` to stop requesting it.

To contribute changes, edit the installed package in your consuming workspace,
then prepare a separate source checkout:

```sh
gnosis propose team/platform --output ../platform-proposal
git -C ../platform-proposal diff --cached
```

`propose` gets the repository and ref from the installed package's lock entry.
It merges your changes onto the current source ref and stages only that package
on `gnosis/platform`. The output directory must be new and outside
`gnosis/`; relative paths are resolved from the selected workspace. Your consuming
workspace and lock remain unchanged. Conflicts stop preparation.

After reviewing the staged diff, complete the contribution with Git:

```sh
git -C ../platform-proposal commit -m "Clarify service ownership"
git -C ../platform-proposal push -u origin gnosis/platform
```

Open a pull request in the source repository. If that branch already exists
remotely, rename your local proposal branch before pushing. Push access is
required; contributors without it need a permitted fork. Gnosis prepares the
checkout but does not commit, push, or create the PR. If you are editing directly
in the publishing repository, use an ordinary Git branch and PR there instead.

## Contributor rules

Set rules in each package's `gnosis/NAME/package.toml`:

```toml
[contributors]
allow = ["@alice", "@my-org/platform"]
deny = ["@blocked-user", "@my-org/restricted-team"]
```

Entries are GitHub.com usernames or `@org/team-slug` groups, including nested
teams and identity-provider groups synced to GitHub teams. Matching ignores case
and an optional `@`. Deny wins; omitting `allow` permits anyone not denied, while
`allow = []` permits nobody. Ownership does not bypass these rules.

`new` checks the local package policy; `propose` checks the latest source package
policy. Restricted operations use your authenticated `gh` account. Team checks
require `read:org` or organization **Members: read** access and stop if membership
cannot be verified. See [GitHub's team API](https://docs.github.com/en/rest/teams/members#list-team-members).

For local and imported contributions, required CI can run
`gnosis check --contributor "$PR_AUTHOR" --base "$BASE_SHA" --head "$HEAD_SHA"`
using trusted event values and a trusted Gnosis binary. It checks changed packages
against base-branch policy, so proposed edits cannot remove their own restrictions.
CI team checks need a token with the same organization access. Protect that check
and require owner review for policy changes and new packages, which have no base
policy yet. Local checks do not prevent direct edits or replace GitHub permissions.

## Commands

| Command | Purpose |
| --- | --- |
| `init` | Initialize a workspace |
| `package` | Create and register a local package |
| `new` | Create a document or navigation folder |
| `source` | Configure a Git source |
| `list` | List packages published by configured sources |
| `add` | Install a package and its dependencies |
| `sync` | Restore imports, or update them with `--update` |
| `check` | Validate basic OKF and package structure |
| `index` | Refresh generated navigation |
| `propose` | Prepare an imported package's changes for upstream review |

Use `gnosis COMMAND --help` for arguments and options.

## Validation and storage

`check` validates package dependencies, UTF-8 Markdown, required concept types,
reserved-index frontmatter, and folder metadata. It accepts unknown types and
fields. It does not validate links, custom schemas, or factual accuracy.

`index` generates navigation from titles and descriptions. Concept contents and
hand-authored indexes are preserved. Remove a manual index to regenerate it.

Packages support regular, non-executable files. Symlinks, submodules, unsafe
paths, and case-insensitive path collisions are rejected. Limits are 16 MiB per
file and 256 MiB per package or local knowledge tree. Package scripts are not run.

Sources are cloned into temporary bare repositories per operation. Fetching may
download repository history, and restore requires access to the locked commits.
Sources use `gnosis/NAME/package.toml`, with one ref per source and one installed
package per name.

Workspace writes use an operation lock, staging, and rollback. Avoid concurrent
edits during an operation. Writes are not atomic across the entire filesystem
and do not guarantee recovery from power loss.

If an interrupted operation leaves `.gnosis/transaction-*`, the next command
stops. Inspect `paths.txt` to map the numbered `old-N` backups and `new-N` staged
files to their destinations. Recover the affected files, then move the transaction
directory outside `.gnosis/` before retrying. Keep recovery files until the
workspace is restored.

## Development

### Build from source

Requires a recent stable Rust toolchain and Git 2.38 or later. From a checkout of
this repository:

```sh
cargo install --path . --locked
gnosis --help
```

To build without installing, run `cargo build --locked` and use
`target/debug/gnosis`.

### Project structure

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Startup and error reporting |
| `src/cli/` | Subcommand arguments, help, and execution adapters |
| `src/workspace/` | Authoring, sources, dependency resolution, sync, proposals, and transactions |
| `src/metadata.rs` | Manifest, lock, package, and source metadata |
| `src/contributors.rs` | Contributor rules and GitHub identity checks |
| `src/okf.rs` | Concept metadata and document validation |
| `src/navigation.rs` | Generated indexes |
| `src/tree.rs` | File trees and path validation |
| `src/git.rs` | Git snapshots, merges, and proposal checkouts |

Each subcommand owns its arguments and `run` method in `src/cli/`. Register new
commands in `src/cli/mod.rs` and put their business logic in the appropriate
workspace module. Shared domain code should not depend on Clap types.

### Checks

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Integration tests use temporary local Git repositories.

## Agent skills

Five optional agent skills separate lookup, authoring, and upstream workflows:

- [gnosis-navigate](skills/gnosis-navigate/SKILL.md): find and read knowledge,
  assess provenance, and answer with citations.
- [gnosis-cli](skills/gnosis-cli/SKILL.md): command syntax, effects, and limitations.
- [gnosis-update](skills/gnosis-update/SKILL.md): apply specified content changes
  and validate the local result.
- [gnosis-author](skills/gnosis-author/SKILL.md): shape new knowledge, extract
  bounded evidence, and obtain approval before handing off to the writer.
- [gnosis-publish](skills/gnosis-publish/SKILL.md): explicitly refresh imports or
  prepare upstream contributions without authoring content.

Install all five using the checked local links in the [skills guide](skills/README.md).

## Releasing

Bump `[package].version` in `Cargo.toml`, refresh `Cargo.lock` with Cargo, and
push the changes to `main`. The **Update Homebrew tap** GitHub Actions workflow
checks each push and skips versions already packaged by the tap. For a new
version, it tests and builds the release commit, creates an immutable `vVERSION`
tag, and updates the tap's formula with that tag and exact commit.

The workflow can also be run manually to retry a failed publication. If the
release tag was already created, retries reuse and test that tagged commit;
they never move the tag. Queued runs use the latest `main` to avoid publishing
stale versions.

The source repository's `HOMEBREW_TAP_DEPLOY_KEY` Actions secret holds a dedicated
SSH deploy key with write access only to `sander-ed/sander-ed-tap`. Source tags
use the workflow's `GITHUB_TOKEN`. If the deploy key is rotated, update both the
tap's deploy key and the source repository's secret.
`skills/` is canonical; `.agents/skills/` is ignored local wiring. If you previously
installed the combined `gnosis` skill, replace it with these folders.
