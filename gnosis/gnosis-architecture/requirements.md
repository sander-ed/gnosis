---
type: Architecture Decision
title: Goals and confirmed requirements
description: Separate agent-authored knowledge from CLI-managed structure, with one authoritative source and owner per package.
sources:
  - resource: ../../PLAN.md
    title: Original implementation plan
  - resource: implementation conversation dated 2026-09-20
    title: User confirmation of format, Git model, and architecture ownership
---

# Context

The repository began as a Go echo-command proof of concept. The implementation
plan asks for a knowledge management CLI, modular knowledge, dependency locks,
registry discovery, customizable package standards, agent meta skills, and a
source-owner approval workflow.

The central principle is: the registry owns discovery, the source repository
owns content and approval, and consumers use package names rather than source
locations. Gnosis must not become a second version-control system or an
autonomous content author.

# Decision

The implementation conversation explicitly confirmed these answers:

| Question | User-confirmed answer |
| --- | --- |
| Which Open Knowledge Format specification? | GoogleCloudPlatform/knowledge-catalog OKF v0.2 |
| How should packages integrate with Git? | Vendored Git subtrees, with repository-wide merge/rebase |
| Who owns the architecture package? | `@sander-ed` |
| How should the Go code be organized? | Use a clearer folder structure instead of a flat Go layout |

Each direct child directory of `gnosis/` is one package and one OKF bundle.
Each package has exactly one accountable GitHub user or team. Multiple
independent accountable owners should normally mean multiple packages.

Agents maintain concept prose, evidence, provenance, and appropriate lifecycle
metadata. The CLI manages package scaffolding, metadata parsing, dependency
resolution, lock state, indexes, structural validation, and Git workflows.
It never invents facts or human verification.

The following are implementation defaults, not additional user-confirmed
requirements: main as the default source branch; YAML for generated metadata
with YAML/JSON manifest support; descriptive version strings with commit pins
rather than version-range resolution; the specific internal package split;
package-local JSON
Schema and text templates; and proposal preparation without publication unless
`--publish` is explicitly supplied.

The canonical architecture package is named `gnosis-architecture`, correcting
the spelling in the original plan. This package records decisions for future
agents; its presence does not imply the owner has reviewed every concept.

See [package contracts](formats.md), [Git synchronization](git-workflows.md),
and [approval boundaries](ownership.md) before changing these behaviors.
