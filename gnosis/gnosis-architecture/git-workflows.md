---
type: Architecture Decision
title: Git synchronization and proposals
description: Native subtrees and three-way source patches preserve Git semantics without publishing consumer content.
sources:
  - resource: https://github.com/git/git/blob/master/contrib/subtree/git-subtree.adoc
    title: Official git-subtree documentation
  - resource: https://git-scm.com/docs/git-clone
    title: Git clone and partial clone
  - resource: https://git-scm.com/docs/git-sparse-checkout
    title: Sparse checkout
  - resource: https://cli.github.com/manual/gh_pr_create
    title: GitHub CLI pull request creation
---

# Context

Submodules identify whole repository commits, not arbitrary package subtrees.
Separate sparse checkouts would change normal clone, commit, and review
semantics in consumer repositories. Copying snapshots and writing a custom
merge engine would lose Git's well-understood conflict handling.

# Decision

Use the installed Git CLI, preserving its authentication and merge behavior.
A source's selected path is split with `git subtree split`; the resulting
package history is fetched into the consumer and added or merged under
`gnosis/NAME`. Splits of the same history are deterministic. Imports include
package history rather than squashing it, enabling ancestry checks during
recovery. Unrelated source package content does not enter consumer history.

Resolution checks the complete requested dependency closure before mutation.
Repositories are reused within one resolution run. Partial clones and sparse
checkouts reduce materialized source content where supported, but splitting
requires source commit/tree history. Package-only network transfer is not
promised. Temporary source clones are removed when the operation ends.

Mutating Git workflows require a clean worktree, an existing initial commit,
and a configured author identity. No automatic stash, hard reset, or force push
is performed. Imports and updates create subtree commits and lock/index commits.
Operation locks prevent concurrent Gnosis mutations, but cannot prevent a user
from running Git concurrently.

Before a subtree operation, a pending record stores its package, starting HEAD,
and intended lock entry. Conflicts leave normal Git conflict files and preserve
the prior lock. Resolve and stage, then run `gnosis pull --continue`; or use
`gnosis pull --abort` while the merge is uncommitted. Continuation checks ancestry
and structure before recording the lock. An already committed merge is never
reset by abort. A stopped multi-package run keeps prior successful packages;
rerun the original command after recovering its pending package.

`restore` imports missing package directories using exact lock commits without
consulting branch tips for version selection. It does not overwrite existing
packages. Source history must retain the locked commits. Ordinary clones of a
consumer already contain its vendored files.

Proposals compute a binary-capable diff between the locked package tree and the
consumer's committed package tree. Only this diff is applied with
`git apply --3way --index` into the correct source path on the latest source
default branch. This preserves unrelated upstream changes and excludes unrelated
consumer changes. Conflicts remain in an isolated, retained source checkout.

Proposal preparation creates a `gnosis/...` branch and commit, without pushing.
Explicit `--publish` pushes that branch and calls `gh pr create`. It never
updates the default branch or merges the PR. Publication needs source write
access; read-only consumers can use ordinary Git/gh fork workflows from the
prepared checkout. Retained checkouts support recovery when pushing or PR
creation fails.

`gnosis merge`, `gnosis rebase`, and `gnosis status` operate on the complete
consumer repository. They are not per-package history rewrites. For example,
`gnosis rebase -X ours origin/main` forwards native flags and semantics.
