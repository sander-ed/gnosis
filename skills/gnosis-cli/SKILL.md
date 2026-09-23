---
name: gnosis-cli
description: Explains Gnosis command syntax, flags, effects, and limitations. Use for CLI help and command selection, including package installation reference; it does not execute workflows, decide knowledge content, or grant authorization.
---

# Gnosis CLI reference

Receive a command/capability question; return a short factual reference or the
relevant help command. Loading this skill does not execute commands. Workflow
intent and approval belong to the calling skill, not this reference.

Commands use the current directory or `gnosis -C PATH COMMAND`
(`--directory PATH`); parent directories are not searched. Use `gnosis --help`
and `gnosis COMMAND --help` for the installed version. In a source checkout,
rebuild with `cargo build --locked` before comparing `target/debug/gnosis` help
with `src/cli/`; an old binary is not evidence of the current source interface.

## Action space

Every synopsis below follows `gnosis` (and optional workspace selection).

| Command synopsis | Effect and limits |
| --- | --- |
| `init` | Initializes a knowledge workspace; does not initialize or commit Git. |
| `package NAME --owner OWNER [-d TEXT]` | Creates and registers a local, published package. `-d/--description` defaults to empty. Does not install an external dependency. |
| `new PACKAGE PATH [OPTIONS]` | Scaffolds a document or navigation folder in an existing local or imported package; refreshes generated package navigation. |
| `source NAME REPOSITORY [--ref REF]` | Registers a Git source under a local alias; ref defaults to `HEAD`. Uses Git authentication, not credentials embedded in URLs. |
| `list` | Queries configured sources for published packages as `SOURCE/NAME`, not installed catalog content. |
| `add NAME [--source SOURCE]` | Installs a package and transitive dependencies. Also accepts `SOURCE/NAME`; retains an existing selection or infers a unique publisher. Ambiguous publishers require an explicit source. |
| `remove NAME [--force]` | Removes a local package or direct import; also accepts `SOURCE/NAME` for imports. Prunes unused imported dependencies, retains shared/explicit imports and pins. Local packages require `--force`; edited imports are protected without it. |
| `sync [--update]` | Restores imports; retains existing pins under unchanged selections. Manifest/source changes can trigger resolution. `--update` refreshes all imports; there is no package selector. |
| `check` | Checks basic OKF and package structure across the workspace; see the separate CI mode below. |
| `index` | Refreshes generated navigation across the workspace without fetching or changing concepts; preserves hand-authored indexes. |
| `propose NAME --output PATH` | Prepares an imported package's changes in a separate source checkout; also accepts `SOURCE/NAME` matching its lock entry. Does not commit, push, or open a PR. |

`add` installs packages; it does not author knowledge. New-knowledge intake is
[gnosis-author](../gnosis-author/SKILL.md). Installation and workspace setup have
no owning workflow in this suite.

`remove` leaves configured sources intact and never changes upstream. Ordinary
import removal checks locked baselines; `--force` skips those checks and can
discard edits in every pruned import, offline. It does not bypass local
contributor rules or dependency validation. A transitive-only import cannot be
removed directly. If another import needs the target, only its direct requirement
is removed. Local removal does not prune dependencies. To keep an import needed
by local knowledge, `add` it explicitly before removing its former parent.
Manual catalog indexes are preserved; obsolete links need manual cleanup.

## Authoring arguments

`new PACKAGE PATH`: paths are package-relative; `.md` is optional for documents.
A trailing `/` or `--dir` creates a navigation folder. Parent directories are
created as needed. Existing destinations are never overwritten; there is no
editor prompt.

| Flag | Value/effect |
| --- | --- |
| `-T/--type TYPE` | Nonempty knowledge type; defaults to `Reference`; custom types are supported. |
| `-t/--title TEXT` | Defaults to a humanized file/folder name. |
| `-d/--description TEXT` | Description used in navigation. |
| `--resource URI` | Resource represented by the concept. |
| `--tag TAG` | Repeat for multiple tags. |
| `--source RESOURCE` | Repeat for multiple provenance sources. |
| `--field KEY=YAML` | Repeat for custom metadata; duplicate fields are rejected. |
| `--body TEXT` | Literal Markdown body, excluding frontmatter. |
| `--body-file PATH\|-` | Body from a workspace-relative file, or stdin with `-`; excludes frontmatter. |

Body flags are mutually exclusive and file-only; default body is a title
heading. Metadata goes into concept frontmatter or folder `index.yml`; folder
metadata is not inherited. Restricted contributor policies can require
authenticated GitHub access. Manual OKF creation is supported by Gnosis itself;
CLI-only scaffolding is a skill-suite policy, not a product limitation.

```sh
gnosis package platform --owner @example/team --description "Platform knowledge"
gnosis new platform operations/ --title "Operations"
gnosis new platform operations/checklist --type Playbook --body-file draft.md
gnosis index
gnosis check
```

## Validation and upstream effects

Plain `check` validates dependencies, UTF-8 Markdown, required concept types,
reserved-index frontmatter, and folder metadata; it accepts unknown types and
fields. It does not validate links, custom schemas, factual truth, or approval.

`check --contributor LOGIN --base REF --head REF` is a distinct CI authorization
mode; all three arguments are required together. It checks changed package
paths against trusted base policy, not document structure. The caller supplies
authenticated PR identity and trusted revisions; Git authors or a locally
asserted login are not authentication.

Sync three-way merges the prior upstream baseline, local content, and selected
upstream revision. Conflicts or invalid documents leave the workspace and lock
unchanged; there are no `--continue` or `--abort` commands.

`propose` reads the source from the lock, merges onto its current ref, and stages
only the selected package on `gnosis/NAME`. Output must be a new path outside
`gnosis/`, relative to the selected workspace. The consuming workspace and lock
remain unchanged. Locally authored packages use ordinary source-repository Git
review instead. See [gnosis-publish](../gnosis-publish/SKILL.md) for that workflow.
