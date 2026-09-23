---
name: gnosis-update
description: Applies specified, authorized Gnosis catalog content changes, scaffolds entries with the CLI, preserves metadata, and validates local results. Use for approved drafts or precise change briefs, not unstructured intake, source extraction, installation, refresh, or publication.
---

# Apply a catalog change

## Change brief

Receive these as short labeled sections in a message, reusing existing context:

- Workspace, target package or placement objective, and requested operations.
- Approved content/draft or a sufficiently explicit intended change.
- Supporting evidence and metadata/convention constraints.
- Caller-supplied approval context. An author handoff must satisfy
  [gnosis-author](../gnosis-author/SKILL.md)'s applicable gates. A direct, precise
  user instruction can authorize its specified correction without an interview.
- If creating a package: explicitly approved name, owner, and agreed purpose.

Resolve missing substantive decisions with the caller. Route new, unstructured
knowledge once to `gnosis-author`; do not invent the design or call intake back
during approved execution. An agent's preference is not user authorization.
For relayed approvals, use `gnosis-author`'s approval-scope rules.

You may realize a precise brief as content and choose an exact suitable location.
For an approved final draft, only non-substantive formatting, location, and
metadata-preservation adjustments are allowed. If execution requires new claims
or sources, changed meaning, dropped claims, or materially different organization,
stop and return the proposed deviation to the caller/author for review.

## Dependencies

Use [gnosis-navigate](../gnosis-navigate/SKILL.md) for placement, conventions,
overlap, and existing evidence. Reuse a still-current navigation result.
Load [gnosis-cli](../gnosis-cli/SKILL.md) when command syntax is needed.
Invoke dependencies by exact skill name if the host supports it; otherwise read
their linked instructions and perform that role. Preserve the brief and approval
state, avoid repeated reads, and report a missing dependency as a broken
installation rather than improvise its procedure.

## Apply

1. Confirm the brief and selected workspace. Inspect the Git worktree and target
   files before writing; distinguish existing edits from this task's changes.
   Use the lock only to identify imported upstream baselines, not content approval.
2. Preserve package language, granularity, custom types/schemas, unknown
   frontmatter, existing provenance, and unrelated work. Follow conventions the
   package actually supplies; do not invent required schema files or templates.
3. Inspect each affected package's contributor restrictions before any edit,
   including direct edits. When restricted, load and apply
   [contributor policy](references/contributor-policy.md). A denial or inability
   to establish permission blocks writing; content approval is a separate concern.
4. For an explicitly approved new local package, use `gnosis package` first.
   Require its approved name and owner; explain that creation registers it as a
   locally published package in the manifest. Do not guess ownership, initialize
   a missing workspace, or install an external package as a fallback.
5. Scaffold new knowledge documents and navigation folders with `gnosis new`,
   using `gnosis-cli` for arguments. Never substitute `mkdir`, `touch`, a manual
   new Markdown file, or a sidecar metadata file to bypass scaffolding or a CLI
   refusal. If the CLI is unavailable or the destination exists, report the
   blocker; do not overwrite it or silently reinterpret the operation.
6. Directly edit the scaffold or existing document to apply the approved content.
   Helpers do not replace content editing. Edit existing folder `index.yml` for
   labels when needed. Keep concept `type` nonempty.
7. Record genuine evidence for changed claims and expose uncertainty. Do not
   invent sources, dates, identities, or verification events. Preserve historical
   verification without implying it covers new claims. Content approval and a
   recent edit do not establish factual verification.
8. Run `gnosis index`, then plain `gnosis check`. Both are workspace-wide.
   Inspect the full diff, including navigation outside the target package.
   Separately check changed links and supplied package conventions; structural
   checking does not cover them or factual correctness.
9. Return the receipt below. On failure, identify partial writes and the blocker;
   do not claim completion merely because content was written.

## Navigation and scope safeguards

Generated indexes are not authored prose: edit their inputs and regenerate.
Preserve hand-authored indexes; add relevant links when the CLI reports it
preserved them. Only a package-root index may carry OKF frontmatter, containing
`okf_version: "0.2"`; preserve that distinction for nested navigation.

Reserve `index.md` and `log.md` rather than using them as concept names. Preserve
existing log history; do not require logs where no convention establishes them.
Folder metadata is not inherited by child concepts.

There is no move/delete command. Deletion, relocation, bulk reorganization, and
workspace initialization require separately scoped tasks, not implied steps in
an addition. Temporary drafts belong outside the catalog. Do not run package
scripts, SQL, executors, or attesters merely because source material includes them.

Local edits to imported packages are allowed. Authoring ends at validated local
changes: no installation, refresh, proposal, commit, push, or PR as a finishing
step. Route an explicitly separate upstream request to
[gnosis-publish](../gnosis-publish/SKILL.md). Never reset, stash, delete, or
overwrite unrelated work to force success.

## Change receipt

Report changed paths and material changes; scaffolding performed; structural
checks and separate link/convention checks; unresolved gaps; unrelated changes
left intact; and any failure or partial completion. Distinguish a pre-existing
workspace-wide failure from the target change, but do not call a failed
validation successful.
