---
type: Architecture Decision
title: Ownership, review, and agent boundaries
description: Source-side GitHub rules enforce approvals; meta skills guide agents but do not grant publishing authority.
sources:
  - resource: https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners
    title: GitHub CODEOWNERS and required owner reviews
  - resource: https://agentskills.io/specification
    title: Agent Skills specification
  - resource: ../../PLAN.md
    title: Single-owner and meta-skill requirements
---

# Context

The source repository is the only authority for a package's accepted content.
Consumers can make local improvements, but those improvements become canonical
only after the source owner's review and merge. Knowledge prose is not executable
authority, and agent-generated claims are not automatically human-reviewed.

# Decision

A package has one `owner.user` or one `owner.team`, expressed as `@username` or
`@organization/team`. This is a single accountable identity, even if a team has
several members. Real account existence and access rights are verified by GitHub,
not inferred by local syntax checks.

`gnosis codeowners` generates rules for locally authored package paths in the
effective CODEOWNERS file. GitHub looks first in `.github/`, then the repository
root, then `docs/`; the CLI preserves that precedence and existing surrounding
rules. Imported packages do not impose their source owners on consumers.
Later matching rules override earlier rules, so repository administrators must
review rules after the managed block.

CODEOWNERS requests reviews but does not itself enforce approval. Administrators
must configure branch protection or rulesets requiring code-owner reviews,
required status checks, and appropriate bypass restrictions. Owners need write
access; teams need to be visible. Protect CODEOWNERS itself from unauthorized
ownership changes. Gnosis does not automatically change server-side settings.

The three bundled meta skills use the open Agent Skills format: a directory
named for the skill with `SKILL.md` containing `name` and `description`
frontmatter. They are installed under `.agents/skills` without overwriting
customizations:

- `gnosis-navigate`: progressive disclosure, package boundaries, citations,
  freshness, trust, and unknown/broken-link handling.
- `gnosis-update`: evidence-backed authoring, local standards, metadata
  preservation, validation, and source-side review.
- `gnosis-cli`: command selection, commit prerequisites, lock handling,
  publication authorization, and native Git recovery.

These skills contain no business-specific knowledge. Agents must not fabricate
source evidence, human verification, approvals, or claims of successful
publication. A bare `verified` mapping and a list represent the same kind of
events; only `human:` identities signal human review. Stale or deprecated
knowledge must be surfaced as such.

Registry URLs must not contain secrets. Existing Git SSH/credential helpers and
authenticated `gh` handle public, private, and GitHub Enterprise access.
Package scripts, executors, and attesters are never automatically run by gnosis.
Publication requires separate authorization; preparation alone does not push.
