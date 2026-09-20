---
name: gnosis-cli
description: Use the gnosis CLI to initialize packages, discover registries, lock dependencies, synchronize Git subtrees, and prepare source pull requests.
---

# Operate gnosis

Run `gnosis --help` and subcommand help for exact options.

- `gnosis init` creates the knowledge root and installs these meta skills.
- `gnosis package init NAME --owner @org/team --description "..."` scaffolds a
  locally owned package. Use `--dependency NAME` for dependencies.
- `gnosis registry add URL --ref main --path gnosis` discovers packages from a
  Git repository. `gnosis registry import FILE` imports a YAML/JSON registry.
- `gnosis add` lists discoveries; `gnosis add NAME` installs a dependency closure.
- `gnosis pull [NAME]` updates tracked imports. `gnosis restore` installs missing
  packages at their lockfile commits, not current branch tips.
- `gnosis index [NAME]`, `gnosis validate [NAME]`, and `gnosis codeowners` manage
  structure. Review generated CODEOWNERS and configure required owner reviews
  on GitHub; the file alone does not enforce approvals.
- `gnosis propose NAME --title "..."` prepares an isolated proposal. Add
  `--publish` to push a proposal branch and create a PR with authenticated `gh`.

Commit pending changes before add/pull/restore/propose. Import/update operations
create Git subtree and lock commits. Never silently stash or discard changes.
If a pull conflicts, inspect `git status`, resolve and stage conflicts, then run
`gnosis pull --continue`. `gnosis pull --abort` invokes native merge abort while
a merge is active. Do not manually remove pending operation metadata.

`gnosis merge ...`, `gnosis rebase ...`, and `gnosis status ...` forward to Git
at the **consumer repository root**. They do not merge or rebase one package.
Use native Git flags after the command, e.g. `gnosis rebase -X ours origin/main`.
These commands do not bypass a pending gnosis package operation.

Exit code 0 means success, 2 means usage error, 1 means an operation failed,
and 130 means cancellation. Native passthrough commands retain Git exit codes.
Do not include credentials in registry URLs; use Git SSH/credential helpers
and `gh auth login` (including `--hostname` for GitHub Enterprise).
