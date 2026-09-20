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
  - resource: ../../internal/workspace/process_unix.go
    title: Internal Unix process-group cancellation
  - resource: ../../internal/workspace/process_windows.go
    title: Internal Windows process-tree cancellation
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
Use `cmd/gnosis` for the executable and focused internal packages for CLI routing,
knowledge formats, and Git-backed workspace workflows. This replaces the initial
flat layout following the user's request for a clearer folder structure.
Dependencies flow from CLI to workspace to knowledge; knowledge has no Cobra or
Git dependency. No service, database, plugin, or dependency-injection layers are
introduced. Package metadata is explicit
versioned data, not environment-driven application settings, so Viper adds
no needed configuration layer and is not included.
Command groups have separate files under `internal/cli`. Workspace state, skill
installation, and CODEOWNERS generation have focused files under
`internal/workspace`. Platform-specific process cancellation uses Go build tags.
Package scaffold assets live with `internal/knowledge`; distributable agent
skills remain under `skills/` and are embedded from that canonical location.
Unit tests are colocated; cross-command workflows live in `tests/integration`.

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
timeouts. Internal subprocess cancellation terminates owned process groups on
Unix and uses `taskkill /T` on Windows; pipe draining is bounded. Archive
cancellation preserves the caller's cancellation error and exit status.
Partial Git changes and pending recovery metadata are retained, never reset.
Explicit native Git passthrough is interactive and follows the caller's
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
