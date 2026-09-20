# Contributor policy

Set catalog-wide rules in the publishing repository's `gnosis.toml`, or rules
for one package in `gnosis/NAME/package.toml`:

```toml
[contributors]
allow = ["@sander-ed"]
deny = ["@blocked-user"]
```

The same fields work for locally authored and imported packages. Imported
packages carry package rules with them; `propose` reads the current publishing
repository's catalog and package rules before preparing a checkout.

- No `allow` field means anyone not denied by that policy may contribute.
- `allow = []` permits nobody. Entries are individual GitHub.com usernames,
  case-insensitive, with an optional `@`. Teams, emails, and wildcards are not supported.
- A deny entry wins over an allow entry. Catalog and package rules must both pass;
  a package cannot relax a catalog restriction.
- `owner` identifies the reviewer. It does not bypass contributor rules or grant
  GitHub access. Include the owner in `allow` if they should also contribute.

## Local checks

`new` and `package` check the workspace's policy before writing. `new` also checks
its package's policy. For imports, these local checks use the editable package
copy; authoritative source rules are checked again by `propose` against the
latest source revision, even if the consumer has not synced.

Restricted operations require GitHub CLI and an authenticated account:

```sh
gh auth login --hostname github.com
```

Gnosis obtains the account through `gh api user`, rather than Git's editable
`user.name` or `user.email`. Catalogs without restrictions need no GitHub CLI.
Reading, installing, syncing, and regenerating indexes are not contributor gates.
Plain `gnosis check` validates structure and policy syntax, not identity.

Local checks are feedback, not an access-control boundary: files and Git commits
can be created without Gnosis. Enforce acceptance in the source repository's CI
and branch rules. Local packages use that same CI path when contributed directly.

## Required CI check

Use a trusted Gnosis binary to compare the proposed commits with the target
branch policy:

```sh
gnosis check --contributor "$PR_AUTHOR" --base "$BASE_SHA" --head "$HEAD_SHA"
```

Supply the PR author's login from the hosting provider's authenticated event,
not commit metadata or a value submitted by the contributor. `--contributor` is
an input for trusted CI; accepting it in a local command does not authenticate it.
This mode authorizes changed catalog paths only, not document validity. Fetch
enough history to find the merge base: changed paths come from the PR diff, while
rules come from the supplied base revision. Root
policy applies to all catalog changes, and each changed package uses its policy
from the base commit, including when its files are deleted or renamed. New
packages use the base catalog policy. Policy edits in the proposed commit cannot
relax the policy used for that check.

[The GitHub Actions example](contributors.yml) runs trusted workflow code from
the base repository, fetches the PR revision without checking it out, and reads
its Git objects as data. It checks the PR author, not every commit author or
co-author. Repository controls must govern who can push to proposal branches.

To enable it in a catalog repository:

1. Commit the rules to the protected base branch and copy the example to
   `.github/workflows/contributors.yml`.
2. Set repository variable `GNOSIS_REV` to a reviewed full commit SHA of
   `sander-ed/gnosis` containing this feature. If that tool repository is private,
   set `GNOSIS_READ_TOKEN` to a token with read-only access to it.
3. Require the `Gnosis contributors` check on the target branch, restrict direct
   pushes and bypasses, and require owner review for policy and workflow changes.

Do not build Gnosis from the PR being checked or execute any proposed package
scripts. The example does not check out the PR head. This follows GitHub's
[guidance for pull_request_target](https://docs.github.com/en/actions/reference/security/securely-using-pull_request_target).
Use protected repository rules to keep contributors from replacing the gate;
[CODEOWNERS alone does not require approval](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners).

This feature checks policy but does not provision GitHub permissions or change
private-repository access. CI and branch protection must be configured separately.
