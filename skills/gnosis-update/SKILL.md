---
name: gnosis-update
description: Create, edit, organize, and contribute Gnosis knowledge packages, or refresh their upstream content. Use for catalog changes and proposal preparation, rather than read-only knowledge lookup.
---

# Update knowledge

Read `gnosis.toml`, the target `gnosis/NAME/package.toml`, its indexes, and nearby
concepts. Use the workspace-root `gnosis.lock` to identify imported packages and
their source baselines. Inspect existing changes before editing. Run commands
from the workspace root or use `gnosis -C PATH COMMAND`.

Follow the package's documented conventions and any schema it actually supplies.
There is no required `standard.yml`, schema file, or index template. Gnosis checks
basic structure; it does not enforce custom schemas or establish factual accuracy.

## Contributor rules

Read `[contributors]` in the catalog's `gnosis.toml` and target `package.toml`.
Both must permit the contributor: missing `allow` is unrestricted, an empty
`allow` permits nobody, and `deny` overrides `allow`. Match individual GitHub.com
logins case-insensitively, ignoring an optional `@`. Ownership is not an exemption.

When restrictions apply, establish identity with `gh api --hostname github.com
user --jq .login`; do not infer it from Git names, emails, or package ownership.
If identity cannot be verified or policy denies access, report that result before
editing. Do not relax rules to make an edit pass. Imported policy copies can be
stale; `propose` checks the latest source catalog and package policy. Direct local
contributions use source-repository CI with the protected base policy. The
`check --contributor --base --head` mode is for trusted CI event data, not a way
to claim another identity locally.

## Author and organize

Edit existing concepts directly. Preserve unrelated content and unknown metadata,
keep a nonempty frontmatter `type`, and record evidence for changed claims.
Represent uncertainty explicitly. Do not invent sources or verification, or treat
an edit as renewed approval. Local and imported packages are both editable.

Create entries with `gnosis new PACKAGE PATH`. Paths are package-relative and
`.md` is optional. Use `--type`, `--title`, and `--description` as needed; type
defaults to `Reference` and title to a humanized filename. `--resource`, repeated
`--tag` and `--source`, and `--field KEY=YAML` add metadata. Duplicate fields are
rejected. Give your best efforts to keep a gnosis package consistant in granularity
for these extra fields.

Use `--body TEXT` or `--body-file PATH` for document bodies without frontmatter;
`--body-file -` reads stdin. File paths are workspace-relative. A trailing `/`
or `--dir` creates a folder with metadata in `index.yml`; body flags are file-only.
Folder metadata controls navigation labels and is not inherited by children.
`new` refuses overwrites and refreshes generated indexes.

Reserve `index.md` for navigation and `log.md` for logs. Edit folder metadata or
concepts rather than generated indexes. Preserve hand-authored indexes and add
links there when needed. Only a package-root index may carry OKF frontmatter,
containing `okf_version: "0.2"`. Preserve unrelated log history.

For a requested new package, use `gnosis package NAME --owner OWNER
--description TEXT`. Use the supplied or established owner; clarify if unknown.
Initialize with `gnosis init` only when creating a new workspace. Creating a local
package registers it for publication through that repository's Git workflow.

## Dependencies and upstream changes

Declare package dependencies by name in `package.toml`. Install external ones
with `gnosis add SOURCE/NAME` using the names shown by `gnosis list`.
`gnosis add NAME` retains an existing selection or infers a unique publisher;
use `--source SOURCE` to choose explicitly. Install from the consuming workspace;
a locally authored package is already available. Configure a requested source with
`gnosis source ALIAS REPOSITORY --ref REF`. Use Git authentication, not credentials
embedded in URLs. Do not invent a source selection for ambiguous packages.

Use `gnosis sync` to restore imports at existing pins, and `gnosis sync --update`
when an upstream refresh is requested. Updates affect all imports. These are
separate operations from editing a concept; do not refresh packages incidentally.
Gnosis merges local changes with upstream. On conflict, preserve local work and
report the affected files; there is no `--continue` or `--abort` workflow.
Do not edit lock commits manually or discard work to force a sync.

## Validate and contribute

After manual edits, run `gnosis index`, then `gnosis check`, and inspect the Git
diff. Both commands operate on the whole workspace. Generated navigation may
change outside the edited package; inspect it without overwriting unrelated work.
Report structural failures separately from the factual review of the content.

For a requested upstream contribution from an imported package, run
`gnosis propose NAME --output PATH` (or `SOURCE/NAME`) with a new checkout path outside `gnosis/`.
It merges the selected package's changes against the current source ref and
stages them on a `gnosis/NAME` branch in a separate checkout. Inspect that
checkout's staged diff for scope and confidential content. The command does not
commit, push, or open a pull request. For local packages, use the source
repository's ordinary Git review workflow.

Carry out commits and publication only within the user's requested scope and
existing authorization. Package ownership metadata is not publication authority;
follow the source repository's review rules. Report what changed, validation
results, and the proposal path when one was prepared. Do not execute scripts or
attesters merely because a knowledge package contains them.
