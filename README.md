# Gnosis

A Git-backed package manager for [OKF 0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
knowledge. Install editable packages, pin upstream commits, and contribute local
changes back to their source repositories.

## Install

Requires a recent stable Rust toolchain and Git 2.38 or later. Supported platforms
are Linux and macOS. Source access uses your existing Git authentication.

```sh
cargo install --path . --locked
gnosis --help
```

To build without installing, run `cargo build --locked` and use
`target/debug/gnosis`.

Commands operate in the current directory. Use `gnosis -C /path/to/project COMMAND`
to select another workspace; parent directories are not searched.

## Create a package

In a Git repository without an existing Gnosis workspace:

```sh
gnosis init
gnosis package platform --owner @my-org/platform --description "Platform knowledge"
gnosis new platform ownership --title "Service ownership"
```

Edit `gnosis/platform/ownership.md`, then run:

```sh
gnosis index
gnosis check
git add gnosis.toml gnosis.lock gnosis
git commit -m "Add platform knowledge"
```

Publish through the repository's normal Git workflow. Only packages listed in
`gnosis.toml` are discoverable; imported dependencies are not republished.
The `owner` field identifies the responsible person or team. Enforce reviews
through your repository's access controls and branch protection.

## Install packages

In a consumer repository:

```sh
gnosis init
gnosis source team git@github.com:my-org/knowledge.git --ref main
gnosis list
gnosis add platform --source team
```

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

To contribute changes from an imported package:

```sh
gnosis propose platform --output ../platform-proposal
git -C ../platform-proposal diff --cached
```

This prepares a separate source checkout on `gnosis/platform`, with only that
package's changes staged. Review the diff, then commit, push, and open a pull
request through your normal workflow. Check for confidential content before
publishing. Gnosis does not commit, push, or open pull requests.
Locally authored packages use their own repository's review workflow.

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

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Startup and error reporting |
| `src/cli/` | Subcommand arguments, help, and execution adapters |
| `src/workspace/` | Authoring, sources, dependency resolution, sync, proposals, and transactions |
| `src/metadata.rs` | Manifest, lock, package, and source metadata |
| `src/okf.rs` | Concept metadata and document validation |
| `src/navigation.rs` | Generated indexes |
| `src/tree.rs` | File trees and path validation |
| `src/git.rs` | Git snapshots, merges, and proposal checkouts |

Each subcommand owns its arguments and `run` method in `src/cli/`. Register new
commands in `src/cli/mod.rs` and put their business logic in the appropriate
workspace module. Shared domain code should not depend on Clap types.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Integration tests use temporary local Git repositories.

The optional [agent skill](skills/gnosis/SKILL.md) covers package navigation,
editing, and contribution review. Copy it into your agent's skill directory to
use it.
