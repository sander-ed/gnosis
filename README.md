# gnosis

A Go CLI for Git-backed knowledge packages. Agents author the knowledge; gnosis
manages discovery, package metadata, dependencies, validation, indexes, and
source-repository proposals. Each direct child directory of `gnosis/` is an
[Open Knowledge Format v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
bundle with one accountable owner.

## Install

Requires Go 1.27.1+, Git with `git subtree` and sparse-checkout support, and a
configured Git author identity. GitHub CLI (`gh`) is needed only for publishing
pull requests. Git SSH keys and credential helpers handle private repositories;
gnosis does not store credentials. GitHub Enterprise works through Git and
`gh auth login --hostname HOST`.

```sh
go build -o bin/gnosis ./cmd/gnosis
./bin/gnosis --help
# Or install into GOBIN / GOPATH/bin (which must be on PATH):
go install ./cmd/gnosis
```

Use `bin/gnosis`, not `go build -o gnosis`: `gnosis/` is the knowledge directory.
The old `--input` / `-i` echo flags remain available, but invoking gnosis without
arguments now displays command help. `--version` reports the build version;
releases can set it with `-ldflags "-X main.version=v1.0.0"`.

## Author a knowledge base

Inside an existing Git repository:

```sh
gnosis init
gnosis package init platform --owner @my-org/platform --description "Platform knowledge"
# A person is also a valid single owner:
gnosis package init architecture --owner @my-user --dependency platform
```

`init` creates `gnosis.yml`, an empty `registry.yml`, `gnosis.lock`, the root
index, and three meta skills under `.agents/skills/`. Use `--skills=false` to
skip skills or `gnosis skills` to install them later. Existing data and customized
skills are never overwritten. Package initialization does not invent concepts.

```text
gnosis/
  gnosis.yml
  registry.yml
  gnosis.lock
  index.md
  platform/
    package.yml
    standard.yml
    frontmatter.schema.json
    index.tmpl
    index.md
    concepts/
      services.md
```

A manifest can be `package.yml`, `package.yaml`, or `package.json`, but not more
than one. YAML/JSON metadata rejects unknown keys, duplicate keys, and multiple
documents. Concept frontmatter, in contrast, preserves and accepts unknown keys.

```yaml
gnosis_version: 1
package: platform
version: 1.0.0
description: Platform knowledge
owner:
  team: "@my-org/platform" # Or user: "@my-user", never both
dependencies: []
```

Names are lowercase letters/digits separated by single hyphens, at most 64
characters. Every package directory must match its manifest name. Owner handles
are explicit GitHub identities, not unqualified business role names. Versions
are descriptive strings; dependencies resolve by registry name and are pinned
to Git commits, **not** solved using semantic-version ranges.

## Package-local standards

Every package carries its own editable `standard.yml`:

```yaml
gnosis_version: 1
schema: frontmatter.schema.json
index_template: index.tmpl
required_files: []
required_headings: ["# Summary"]
max_words: 600
```

`schema` points to a JSON Schema (default: draft 2020-12) for concept frontmatter.
`required_headings` requires exact heading lines in each concept's body.
`max_words` counts whitespace-separated body tokens, including Markdown; zero
disables the limit. `required_files` lists package-relative files. Paths and
schema references must remain inside the package; remote schema fetching and
symlinks are not supported. Package files are limited to 16 MiB each, with a
256 MiB aggregate import limit. Discovery metadata and generated management
files also have a 16 MiB limit; oversized registry updates leave existing data
unchanged.

The baseline OKF rule always requires a nonempty string `type`. Optional trust,
provenance, and lifecycle fields are not required by the default policy. Unknown
types, additional fields, and broken links are allowed. A stricter schema is a
**package policy**, not an extra OKF requirement.

```markdown
---
type: Reference
title: Services
description: How the platform services relate.
---
# Summary

Agent-authored, source-backed knowledge belongs here.
```

```sh
gnosis index platform
gnosis validate platform
gnosis index       # All packages and the root directory index
gnosis validate    # All packages, registry, lock and dependency graph
```

`index.tmpl` is a Go text template with `.Root`, `.Title`, `.Description`, and
`.Entries` (each has `.Title`, `.Description`, `.URL`). Entries are escaped and
ordered deterministically. Retain the generated-file marker from the scaffold.
Root package indexes may contain only `okf_version` frontmatter; nested indexes
and `log.md` do not contain frontmatter. `log.md` date headings use
`## YYYY-MM-DD`. Indexing never rewrites concept metadata or prose.

Hand-authored package indexes require explicit `gnosis index NAME --force`
before replacement. A hand-authored root `gnosis/index.md` must be moved aside
explicitly. Missing indexes are allowed by validation, as OKF permits.
The referenced index template, however, must exist, be a regular file, and have
valid Go template syntax; validation and imports enforce this before indexing.

## Discover and install packages

```sh
gnosis registry add git@github.com:my-org/knowledge.git --ref main --path gnosis
gnosis add                 # Sorted discovery list
gnosis add --json          # Machine-readable registry
git add gnosis
git commit -m "Configure knowledge registry"
gnosis add platform       # Imports platform and its transitive dependencies
gnosis list --json        # Installed manifests
```

Discovery reads direct-child manifests and an optional `registry.yml`,
`registry.yaml`, or `registry.json` at `--path`. Use `--path .` for bundles at the
repository root, or a specific package path for a single bundle. Re-running
`registry add` refreshes descriptions and discoveries; it does not silently
remove entries or redirect an existing name to a different source.

For a central registry, use `gnosis registry import registry.json` (YAML also
works). Its top-level shape is a mapping:

```yaml
platform:
  description: Platform knowledge
  owner: "@my-org/platform"
  source:
    type: git
    repository: git@github.com:my-org/knowledge.git
    path: gnosis/platform
    default_branch: main
```

Source paths are relative to the source repository root. A registry discovered
from Git may use `repository: .` for that same repository. An imported file must
use explicit Git URLs or absolute local repository paths. Registry definitions
must agree with any manifests discovered at the same location. Discovery is a
local snapshot, not a hosted registry service.

Imports use native Git subtrees **with package history**, not nested repositories
or submodules. Dependency cycles, missing names, bad manifests, and invalid
content are detected before imports begin. Only the selected packages and their
dependencies enter the consumer working tree/history. Git uses partial clone
and sparse checkout where supported, but subtree splitting can still require
repository-wide commit/tree metadata; this is not a guarantee of
package-only network transfer.

## Update, restore, and recover

Commit your changes before importing, updating, restoring, or proposing.
Gnosis never automatically stashes, resets, or force-pushes.

```sh
gnosis pull platform       # Update this imported package
gnosis pull                # Update all imported packages
gnosis restore             # Install missing packages at exact locked revisions
gnosis status --short
```

`gnosis.lock` records each package's source, source commit, split subtree commit,
version, and dependencies. Existing dependency pins stay fixed unless explicitly
pulled (or included in `pull` without names). An explicit, committed registry
source edit is honored by the next pull of that package, including a changed
repository, path, or branch. Unselected dependency pins remain unchanged;
restore always uses the locked source, not an edited registry.
Imports and successful updates
create subtree commits and separate lock/index commits. A Git clone of a consumer
already contains its packages; restore is for missing package directories.
Restore does not reset existing packages or discard local edits. Source history
must still make locked commits available, but the original branch name need not
exist.

Updates use real Git merges:

```sh
# After a conflicting pull:
git status
# Resolve the conflict files, then:
git add gnosis/platform
gnosis pull --continue
# Or abandon the still-uncommitted merge:
gnosis pull --abort
```

The lock remains unchanged until that package merge succeeds. A pending operation
is recorded under the Git directory and blocks other package mutations.
Both automatic completion and continuation validate the merged dependency graph
before advancing the package lock. An invalid graph retains the old lock and
pending record: repair the manifests, then continue.
Projected lock dependencies are also checked before imports or updates begin.
If updating only one package would create a cycle among pins, pull the affected
dependencies together; local manifest edits do not override source pins.
Continuation validates the result and commits the lock/index. Once the merge is
committed, finish with `--continue`; abort will not reset commits. If a
multi-package operation stops, earlier successful packages remain committed;
finish/abort the pending package, then rerun the original command for the rest.
A crash can leave `gnosis/operation.lock` inside the Git directory: inspect its
PID and ensure no operation is running before manually removing that exact file.
Cancellation terminates internal Git/`gh` process trees on Unix and Windows and
bounds pipe draining. An interrupted Git command can leave partial staged
changes, including a hook interrupted before Git records a merge. Inspect
`git status` and the retained pending operation; gnosis never resets those
changes automatically.

`gnosis rebase -X ours origin/main` and `gnosis merge ...` pass arguments to Git
at the **consumer repository root**. They affect the whole repository, not a
single package. Native Git remains available for all other commands.

## Propose source changes

```sh
# Edit concepts, regenerate indexes, validate, and commit in the consumer.
gnosis propose platform --title "Clarify service ownership"
```

By default, this prepares a retained checkout under the consumer Git directory's
`gnosis/proposals/`. It applies only the selected package's committed changes,
using a three-way patch against the latest source branch, and creates a commit
on a `gnosis/...` branch. Unrelated consumer files are never proposed. Nothing
is pushed. Conflicting proposals retain their checkout for ordinary Git recovery.

```sh
gnosis propose platform --publish --title "Clarify service ownership" --body "Evidence and rationale"
```

`--publish` pushes a new proposal branch and uses `gh pr create`. It requires
write access to the source repository plus authenticated `gh`; it never pushes
to the source default branch or merges the PR. If PR creation fails after push,
the error identifies the retained checkout so you can retry `gh pr create`.
For read-only upstream access, prepare locally and use Git/`gh` to fork and
publish from that checkout; automatic fork orchestration is not implemented.
Locally authored packages use their repository's normal branch/PR workflow.

## Ownership and agent skills

`gnosis codeowners` maintains a marked block in the effective GitHub CODEOWNERS
file, preserving surrounding rules. Only locally authored packages receive
rules; imported package ownership is enforced in its **source** repository.
Later CODEOWNERS rules win, so review any rules following the generated block.

On GitHub, separately enable required code-owner reviews, require your validation
checks, restrict bypasses, and protect CODEOWNERS itself. Owners must have write
access, and teams must be visible. A CODEOWNERS file alone does not require
approval. The CLI does not change server-side repository settings.

Bundled [Agent Skills](https://agentskills.io/specification):
`gnosis-navigate` (progressive discovery and evidence), `gnosis-update`
(package-local authoring policy), and `gnosis-cli` (operations and recovery).
They contain no business-specific knowledge. Skills are instructions, not
permission to publish changes or execute package-provided code.

The [gnosis-architecture package](gnosis/gnosis-architecture/index.md) records
the confirmed requirements, design decisions, and primary research sources.

## Development

```text
cmd/gnosis/                  Executable entry point and signal handling
internal/cli/                Cobra commands, I/O, and exit codes
internal/knowledge/          Manifests, policies, OKF validation, and indexes
internal/knowledge/assets/   Embedded package scaffolding
internal/workspace/          Git operations, registry, sync, proposals, and ownership
internal/testutil/           Shared test fixtures and subprocess helpers
tests/integration/           End-to-end CLI workflows
skills/                     Canonical, embedded agent skill definitions
gnosis/                     The project's knowledge base
```

Unit tests live beside their packages. CLI workflows cross the public command
interface in `tests/integration/`. Dependency direction is
`cmd/gnosis -> cli -> workspace -> knowledge`; the CLI also uses knowledge
contracts directly. Knowledge-format code does not depend on Cobra or Git.

```sh
go test ./...
go vet ./...
go build -o bin/gnosis ./cmd/gnosis
bin/gnosis validate
```

Integration tests use temporary local repositories; they cover transitive
imports, repeated pulls, pinned restores, native conflict recovery, source
proposal isolation, and publication via a stub `gh` without contacting GitHub.
Use `gnosis completion --help` for shell completion generation. Normal output
goes to stdout and errors to stderr. Exit codes: 0 success, 1 operation failure,
2 usage error, 130 cancellation; Git passthrough preserves Git's exit code.
Place `-C DIRECTORY` before passthrough commands so it selects the repository
rather than becoming a native Git argument.
