# Gnosis

A small package manager for collaborative [OKF 0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
knowledge. Git repositories publish named packages. Projects install editable
copies, pin their upstream commits, and propose improvements back to the source.

Gnosis is written in Rust. This implementation replaces the earlier Go
experiment; there is no Go toolchain dependency or compatibility layer for its
commands and YAML management files.

## Design decisions

The design follows the requirements clarified during the rewrite:

| Requirement | Decision |
| --- | --- |
| Several packages from different locations in one project | Named Git sources, one `gnosis/` directory, one manifest and lock |
| Versions indicate freshness, not compatibility | Exact Git commit pins; no semver or PubGrub, and no release tags required |
| Projects can edit their own installed knowledge immediately | Ordinary vendored files; uncommitted edits are supported |
| The original business owner approves returned knowledge | Prepare a source branch; repository review rules control acceptance |
| Package schemas are entirely user-defined | Preserve arbitrary non-executable regular files; no schema/template engine |
| CLI handles OKF metadata and indexes, not knowledge authoring | Check basic structure and read metadata to generate indexes; never rewrite concept frontmatter or prose |
| Agents need ordinary navigation, not a custom runtime | Markdown, nested indexes, and one optional agent skill |

From [uv](https://docs.astral.sh/uv/concepts/projects/sync/) we borrow explicit
requirements, exact locks, deliberate upgrades, and separation of source
selection from installation. [PubGrub](https://github.com/pubgrub-rs/pubgrub)
solves compatibility constraints; these requirements only need graph traversal.
Adding a solver without a compatibility model would make this proof of concept
larger without improving it.

Installed content is intentionally editable, unlike a conventional immutable
package cache. The lock records the **upstream baseline**, not a checksum of local
knowledge. Consumer Git records local work and approval of incoming diffs.
Source Git records canonical content and approval of contributions. These are
two different reviews.

## Build

Requires a recent stable Rust toolchain and Git 2.38+ (`merge-tree --write-tree`).
Gnosis currently targets macOS and Linux. It uses the installed Git CLI and existing Git
SSH/credential configuration; it does not store credentials.

```sh
brew install rust                 # macOS, if needed
cargo build --locked
target/debug/gnosis --help

# Optional: install the executable onto PATH.
cargo install --path . --locked
```

Commands below assume the executable is on PATH as `gnosis`. Use
`gnosis -C /path/to/project COMMAND` to select a project explicitly. The CLI does
not search parent directories. If the former Go executable is still installed
elsewhere on PATH, remove it or use `target/debug/gnosis` explicitly.

This repository is already a knowledge workspace. Its `gnosis.toml` publishes
the architecture package and the locally authored `beno-migrering` package;
do not run `init` over it. Run `cargo run -- check` to inspect it.

## Publish a source

Start in a new or existing source Git repository, without an existing `gnosis/`
directory:

```sh
gnosis init
gnosis package platform --owner @my-org/platform --description "Platform knowledge"
```

Scaffold a concept, then write its content using your editor or agent:

```sh
gnosis new platform ownership --title "Service ownership" \
  --description "How services are assigned to teams."
```

You can also create an ordinary Markdown file manually. For example:

```markdown
---
type: Reference
title: Service ownership
description: How services are assigned to teams.
sources:
  - resource: https://example.com/ownership-policy
---
# Service ownership

Knowledge and evidence belong here.
```

Save that as `gnosis/platform/ownership.md`, then:

```sh
gnosis index
gnosis check
git add gnosis.toml gnosis.lock gnosis
git commit -m "Publish platform knowledge"
# Review and push through the source repository's ordinary workflow.
```

Creating a package lists it in `gnosis.toml`:

```toml
format = 1
packages = ["platform"]

[sources]

[dependencies]
```

Only names in `packages` are published by that repository. Imported dependencies
are not implicitly republished. There is no registry server or separate
publication command: a package becomes discoverable when its source ref
contains the catalog entry and package files.

The package itself has `gnosis/platform/package.toml`:

```toml
name = "platform"
owner = "@my-org/platform"
description = "Platform knowledge"
dependencies = []
```

`owner` is an accountable identity, not a grant of permissions or a claim of
approval. Prefer the source repository's actual GitHub user or team. Gnosis does
not verify account membership.

## Author knowledge

`new` creates content **inside an existing package**. Files are the default;
`.md` is optional and parent directories are created as needed:

```sh
gnosis new beno-migrering migration-checklist \
  --type Playbook \
  --title "Migration checklist" \
  --description "Steps and checks for a migration."

gnosis new beno-migrering migrations --dir \
  -T Playbook -t "Migrations" -d "Migration guidance"
gnosis new beno-migrering migrations/source-mapping.md --type Reference
```

The first command creates `gnosis/beno-migrering/migration-checklist.md`.
The directory command creates `gnosis/beno-migrering/migrations/index.md`
and `index.yml` containing its metadata.
A trailing `/` also selects directory mode, so `new beno-migrering migrations/`
is equivalent to `new beno-migrering migrations --dir`.

Folders accept the same metadata flags as files. Their title and description
appear in the parent index just like a document's:

```sh
gnosis new beno-migrering snowflake/ \
  -T "Best Practise" -t "Snowflake" \
  -d "Guidance when the target platform is Snowflake"
```

The package index contains:

```markdown
* [Snowflake](snowflake/index.md) - Guidance when the target platform is Snowflake
```

Folder metadata is stored in `snowflake/index.yml`, a Gnosis extension alongside
the standard OKF index. The generated `snowflake/index.md` uses the title as its
heading and includes the description. Reserved nested OKF indexes remain free
of frontmatter. Type, tags, sources, and custom fields are retained in
`index.yml`, not discarded or inherited by child documents.

You can edit `index.yml` manually and run `gnosis index` to update navigation.
Folders without that file still work and use their directory name as the label.
`--body` and `--body-file` remain file-only: folder indexes are generated from
their metadata and children.

### Metadata and content

The same fields go into document frontmatter or folder `index.yml`.

| Flag | Metadata |
| --- | --- |
| `-T`, `--type TYPE` | `type`; defaults to `Reference`, including support for custom types |
| `-t`, `--title TEXT` | `title`; defaults to the file/folder name with hyphens/underscores replaced by spaces and the first letter capitalized |
| `-d`, `--description TEXT` | `description`, used in navigation |
| `--resource URI` | `resource`, the asset represented by the concept |
| `--tag TAG` | Adds an item to `tags`; repeat the flag for multiple tags |
| `--source RESOURCE` | Adds `{resource: ...}` to `sources`; repeat for multiple sources |
| `--field KEY=YAML` | Sets a custom or additional top-level field using a YAML value |

The common flags are encoded as strings, so punctuation and multiline
descriptions are safely serialized. `--field` also supports numbers, mappings,
and lists; quote its argument for the shell. For example:

```sh
gnosis new beno-migrering source-mapping \
  --tag migration --tag sql \
  --source /evidence.md \
  --field 'status=draft' \
  --field 'custom={region: eu}'
```

A field cannot be supplied twice, including through both a named flag and
`--field`. Default type/title values apply only when not supplied. No timestamps,
verification, sources, or factual prose are invented.

By default, a concept contains its frontmatter and a heading using its title.
There are three equally supported authoring modes:

| Mode | Workflow |
| --- | --- |
| Human using an editor | Run `new`, then edit the created file normally |
| Agent or script | Supply flags and `--body`, or provide Markdown through `--body-file PATH` / `--body-file -` |
| Fully manual | Create valid OKF files in an editor, then run `index` and `check` |

```sh
# Literal Markdown body.
gnosis new beno-migrering observation --body "An observation with supporting evidence."

# Body from a file, relative to the selected workspace (-C).
gnosis new beno-migrering findings --body-file draft.md

# Body from stdin; no prompt or editor is launched.
printf '# Findings\n\nEvidence-backed content.\n' |
  gnosis new beno-migrering findings-from-agent --body-file -
```

Body input is Markdown **without frontmatter** and is preserved as provided.
`--body` and `--body-file` are mutually exclusive. File/stdin bodies must be
UTF-8 and fit the existing file-size limit.

`new` works on both local and imported packages without contacting their
sources. It refreshes the affected package's generated indexes, never
overwrites an existing destination, and leaves the manifest, lock, and Git
staging unchanged. Hand-authored indexes are preserved; the command reports
ancestor indexes that may need a navigation link added manually.

Use the commands according to their purpose:

| Command | When to use it |
| --- | --- |
| `new` | Create a document or navigation directory and refresh its package's indexes |
| `index` | Refresh local navigation after editing or adding files manually; no fetching |
| `check` | Inspect basic OKF/package structure without modifying knowledge |
| `sync` / `sync --update` | Restore imported packages at locked commits / merge newer upstream knowledge |

Sync rebuilds indexes for imported packages as part of installation; it is not
the authoring command for locally owned packages. No authoring mode enforces a
custom package schema or verifies factual accuracy. Run `check`, inspect the
files and Git diff, and follow the package's review conventions.

## Install and collaborate

In a consumer project:

```sh
gnosis init
gnosis source team git@github.com:my-org/knowledge.git --ref main
gnosis source public https://github.com/example/knowledge.git --ref main
gnosis list
gnosis add platform --source team
```

Result:

```text
gnosis.toml                   # Published packages, sources, direct imports
gnosis.lock                   # Source locations, exact commits, dependency graph
gnosis/
  index.md                    # Navigation across packages
  platform/
    package.toml
    index.md
    ownership.md
.gnosis/                      # Ignored operation locks and transient write staging
```

Commit `gnosis.toml`, `gnosis.lock`, and `gnosis/` to the consumer repository.
A plain Git clone then has the knowledge immediately. Alternatively, a project
with just the committed manifest and lock can reconstruct packages with
`gnosis sync`. Locked commits must still be obtainable from the source.

Edit installed concepts directly. Add, correct, reorganize, or delete knowledge
as your package's own conventions permit. No append-only or protected-section
policy is imposed. Custom schemas and additional metadata belong to the package
and are enforced by your own tools, not by this CLI.

```sh
gnosis index
gnosis check
git diff
# Commit locally when appropriate. No upstream approval is needed for local work.
```

### Sync and version provenance

```sh
gnosis sync                   # Keep existing pins; restore missing packages
gnosis sync --update          # Refresh all imported packages and merge local edits
git diff                     # Review knowledge changes AND lock changes
```

Ordinary sync does not advance unchanged pins just because a source has newer
commits. Manifest/source changes cause dependency resolution; existing pins are
retained where their source selection is unchanged. Updating is workspace-wide
currently, not per-package.

For each imported package the lock records the source alias, repository, ref,
exact commit, and dependency names. This records the package versions present
when a project implements something. Commit the lock alongside that
implementation. Authors of packages should similarly commit the locks in their
source projects to retain their authoring context.

Dependencies are names in `package.toml`; they mean "make this other knowledge
available", not "guarantee its semantic compatibility". Consumer resolution does
not import or impose the source project's lock. Each consumer chooses its own
pins. Cyclic references are allowed because packages contain no build/install
steps.

For a dependency published by several configured sources, gnosis fails rather
than guessing. Select it explicitly with:

```sh
gnosis add core --source team
gnosis add platform --source team
```

That direct selection also controls transitive references to `core`. One name
identifies one installed package. Existing locked source selections are retained;
public sources do not silently replace private ones. Sources mentioned inside a
downloaded project's own manifest are never automatically followed. An unavailable
configured source is an error, not a reason to fall back to another.

For locally authored packages, declare dependencies in `package.toml` and install
external ones with `gnosis add`; other locally authored packages can be referenced
directly. `check` requires every declared dependency to exist.

### Merging local and upstream knowledge

Sync computes:

```text
base     = package at its previous locked upstream commit
local    = current editable package, including uncommitted changes
upstream = package at the newly selected source commit

result   = Git three-way merge(base, local, upstream)
```

Git performs merges in an isolated temporary repository. There is no consumer
subtree history, automatic stash, consumer commit, index staging, or force reset.
Generated indexes are removed from merge inputs and rebuilt afterward;
hand-authored indexes participate in the merge like other content.

All selected packages are prepared and checked before writing the workspace. A
merge conflict or invalid document leaves all packages, the manifest, and the
lock unchanged. Git's conflict report names the affected files. Reconcile local
content against the source manually, or first preserve the local work elsewhere,
then retry. Gnosis deliberately has no `--continue`, `--abort`, automatic conflict
resolution, or persistent conflict editor.

An unused dependency is removed only if it has no local changes relative to its
old baseline. Otherwise sync stops and asks you to preserve that work. Removing
an entire installed directory is treated as a request to restore it, not as a
dependency removal; edit `[dependencies]` in the manifest to remove a direct
requirement.

### Return knowledge to its owner

```sh
gnosis propose platform --output ../platform-proposal
git -C ../platform-proposal diff --cached
```

This creates a normal source-repository checkout and a `gnosis/platform` branch
based on the current configured source ref. It applies only the selected
package's delta from its locked baseline, using the same three-way merge.
Changes are staged, **not committed or pushed**. The consumer remains untouched.

Inspect the staged diff for correctness and confidentiality. Then use ordinary
Git and your hosting provider's PR workflow:

```sh
git -C ../platform-proposal commit -m "Propose platform knowledge improvement"
git -C ../platform-proposal push -u origin gnosis/platform
# Open a PR and request the source package owner's review.
```

For public upstreams without write access, point the proposal checkout at your
fork and open a PR from it. Gnosis does not automate forks or PR creation.
The source branch is never updated by `gnosis propose`.

On GitHub, source maintainers must configure package CODEOWNERS rules **and**
required owner reviews/branch protection, protect those rules, and restrict
bypasses. The manifest owner string and a CODEOWNERS file alone cannot enforce
approval. Publication to Git is the permission mechanism for team, organization,
and public sharing; no separate access-control layer is implemented here.

Package-only diffs prevent unrelated files from being included, but cannot
detect confidential information written into that package's prose. A human
review before pushing is the privacy gate. Do not allow an agent to auto-push
proposals to public sources.

After an approved contribution is merged:

```sh
gnosis sync --update
```

Already-incorporated local changes converge without being appended twice.
Unrelated local findings remain local.

## OKF and agent behavior

`check` validates UTF-8 Markdown, concept YAML mappings with a nonempty string
`type`, reserved-index frontmatter placement, and package/dependency structure.
It also checks folder `index.yml` mappings and their type/title/description fields.
Unknown concept types and fields are accepted. Broken links are allowed.
It is a **basic structural check**, not a full validator of every optional OKF
metadata family, custom schemas, factual accuracy, freshness, or verification.

`index` reads document frontmatter and optional folder `index.yml` titles and
descriptions to generate deterministic nested indexes with
escaped labels and encoded links. It never changes concept bytes. Missing indexes
and indexes carrying the current or previous gnosis marker are managed; hand-authored indexes are
preserved. To opt a hand-authored index into generation, move it aside first.

Copy [the agent skill](skills/gnosis/SKILL.md) into your agent's skill directory
(for example `.agents/skills/gnosis/SKILL.md`). Skill installation is deliberately
manual. It covers navigation, local editing, evidence, and the approval workflow.
No LLM, agent SDK, search server, or embedding database is required.

## Internal architecture

One crate, four modules:

| Module | Interface and responsibility |
| --- | --- |
| `main` | CLI arguments, dispatch, diagnostics |
| `model` | Manifest/lock/package contracts, file trees, basic OKF checks, derived indexes |
| `git` | Exact Git snapshots, native three-way merging, source checkout preparation |
| `workspace` | Source discovery, dependency closure, operation sequencing, safe writeback |

The useful seam is a package file tree plus its upstream revision. Git details
stay behind that interface. There is no backend trait with one implementation,
plugin system, service layer, or async runtime. A solver or persistent cache can
be introduced later if real workloads justify it.

`git` preserves ordinary authentication for source fetching. Temporary merge
repositories disable global/system Git configuration, hooks, and filesystem
monitor commands. Package scripts, executors, and schema code are never run.
Only regular, non-executable package files are supported; symlinks and submodules
are rejected, as are path collisions on case-insensitive filesystems. Limits are
16 MiB per file and 256 MiB per package/local knowledge tree.

Workspace mutations take an OS file lock, stage replacement files on the same
filesystem, and roll back ordinary write failures. They do not provide
filesystem-wide atomicity or a power-loss durability guarantee. A process crash
can retain a `.gnosis/transaction-*` directory; the next command stops rather
than guessing. Its `paths.txt` maps numeric suffixes to workspace paths,
`old-N` contains previous content, and `new-N` contains prepared content not yet
installed. Inspect and recover those exact paths, then move the transaction
directory outside `.gnosis/` before retrying. Do not remove the entire state
directory to bypass recovery.

Do not edit workspace files concurrently with gnosis operations. Pre-write
checks catch changes during preparation, but cannot lock an editor or ordinary
Git commands.

## Deliberate limits

No hosted registry, semver solving, release automation, migration, global cache,
JSON output, custom validation engine, automated publishing, or Git command
wrappers. Source repositories use the fixed `gnosis/NAME/package.toml` layout
and one selected ref per source. Independently released subpackages/tags and
multiple installations of the same name are not supported.

Sources are cloned into temporary bare repositories per command, deduplicated
within that command. Only selected package files are installed, but Git fetching
is **not** package-only: source history may be downloaded. There is no offline
restore without available Git objects. Plain vendored files remain readable
without gnosis or network access.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Integration tests use temporary local Git repositories, not a hosting account.

See [Rust CLI architecture](ARCHITECTURE.md) for the code map, design rationale,
and instructions for adding commands.

The current design is also documented in the
[architecture knowledge package](gnosis/gnosis-architecture/index.md).
