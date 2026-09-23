# Contributor policy

Load for restricted edits or contribution preparation. This reference owns policy
interpretation for both `gnosis-update` and `gnosis-publish`; policy permission,
content approval, and publication authorization are independent.

## Establish the applicable rules

Read `[contributors]` in each target `gnosis/NAME/package.toml`. There is no
catalog-wide contributor policy. Missing `allow` permits actors not denied;
`allow = []` permits nobody. Deny overrides allow, including a team deny that
matches an individually allowed user. Ownership is not an exemption.

Rules are GitHub.com usernames or `@org/team-slug`, matched case-insensitively
with an optional leading `@`. Teams include nested members and identity-provider
groups synced to GitHub teams. Invalid or unreadable rules are blockers, not
unrestricted access.

For local authoring, inspect the local package policy; `new` enforces it, but
direct file editing does not. Apply the same check before direct edits.
For imported contribution preparation, inspect local restrictions and let
`propose` enforce the latest source package policy. A stale local policy is not
authority to bypass the source check.

## Authenticate and resolve membership

When `allow` is present or `deny` is nonempty, establish the actor using:

```sh
gh api --hostname github.com user --jq .login
```

For relevant team rules, use the complete, paginated members API, substituting
the actual organization and team slug:

```sh
gh api --hostname github.com 'orgs/ORG/teams/TEAM/members?per_page=100' \
  --paginate --jq '.[].login'
```

Use organization `read:org` or **Members: read** access. Only a successful,
complete listing can establish non-membership. Authentication failures,
inaccessible teams, incomplete pagination, or membership errors block the
operation; never interpret them as an empty team or no restriction. Evaluate
denies before allows; matching an allow cannot bypass an unresolved deny.
Do not require network identity checks when no restriction exists.

## Preserve authority boundaries

Do not modify policy to authorize the current operation, substitute a Git author
or owner field for authenticated identity, or equate local checks with upstream
owner approval. Never embed credentials in source URLs.

For direct source contributions, required source CI checks changed packages
against base-branch policy. See [gnosis-cli](../../gnosis-cli/SKILL.md) for the
trusted-input contributor-check mode; it is not a local identity proof or
structural validation. Protect required checks and require owner review for
policy changes and new packages without a base policy. Follow repository access
controls and review rules; local skill checks cannot replace them.

Report the applicable restriction and any authentication, membership, or
authorization blocker before writing or preparing a contribution. Do not weaken
the policy or fall back to manual edits after a refusal.
