---
type: Architecture Decision
title: Tooling choices and research record
description: Prefer established format, Git, schema, CLI, and agent tooling over custom infrastructure.
sources:
  - resource: https://github.com/spf13/cobra/blob/main/site/content/user_guide.md
    title: Cobra user guide
  - resource: https://github.com/santhosh-tekuri/jsonschema/tree/v5.3.1
    title: JSON Schema validator implementation and supported drafts
  - resource: https://github.com/yaml/go-yaml
    title: Maintained Go YAML implementation
  - resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: Open Knowledge Format v0.2
---

# Context

The plan requests research and preference for industry-standard tooling.
Primary documentation was consulted during implementation on 2026-09-20:
OKF v0.2; Git subtree, clone, and sparse checkout; GitHub CODEOWNERS and
`gh pr create`; the Agent Skills specification; Cobra; and JSON Schema tooling.
These specifications describe separate concerns rather than one universal
knowledge package manager.

# Decision

Use Cobra for command routing, flag validation, help, and shell completions.
Retain a flat Go package with focused files rather than introduce service,
database, plugin, or dependency-injection layers. Package metadata is explicit
versioned data, not environment-driven application settings, so Viper adds
no needed configuration layer and is not included.

Use the stable, security-maintained `go.yaml.in/yaml/v3` parser for YAML and
JSON-shaped management data. Upstream feature development is on v4; this
implementation pins v3's stable API. Strict decoding avoids misspelled structural keys and ambiguous
manifests. Use `santhosh-tekuri/jsonschema/v5` for JSON Schema compilation and
validation instead of implementing a partial schema language. Go `text/template`
provides a package-local, non-executable index rendering format.

Git handles storage, commits, package history splitting, merges, rebases,
credentials, and conflict state. GitHub CLI handles GitHub/GitHub Enterprise PR
creation. GitHub rulesets and CODEOWNERS handle source-side approval. Gnosis
adds only the package registry/lock contracts, resolution orchestration,
validation/index glue, and recoverable workflow metadata.

The CLI uses signal-aware contexts and bounded internal Git/gh subprocess
timeouts. Explicit native Git passthrough is interactive and follows the caller's
context. Management writes use temporary files and atomic replacement.
Operational errors are surfaced with nonzero status, not hidden behind empty
registries or success-shaped defaults.

Local integration tests use temporary source and consumer repositories to check
dependency ordering, import scope, repeated pulls, pinned restore, clean-state
requirements, native conflicts and recovery, and proposal isolation. Publication
uses a stub `gh` in tests: no real GitHub PR is created. Structural tests cover
YAML/JSON ambiguity, editable policies, exact word thresholds, deterministic
indexes, path containment, schema loading restrictions, and unsafe Git entries.

Current boundaries are intentional: no hosted registry service, semantic-version
range solver, automatic fork orchestration, package removal/pruning, business
content generation, automatic attestation execution, or server-side permission
management. Future extensions must preserve the separation of registry discovery,
source authority, and agent-authored knowledge.
