---
type: Architecture Decision
title: Knowledge and metadata contracts
description: OKF defines concepts while package-local policies and commit-pinned Gnosis metadata define management.
sources:
  - resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: OKF v0.2 specification
  - resource: https://json-schema.org/draft/2020-12
    title: JSON Schema draft 2020-12
  - resource: ../../README.md
    title: Implemented CLI contracts and examples
---

# Context

OKF is intentionally minimal. It supports bundles distributed as repository
subdirectories, standard Markdown, YAML frontmatter, and progressive disclosure.
It does not specify a package registry, dependency solver, or fixed concept
taxonomy. Those Gnosis management files are extensions, not part of OKF.

# Decision

Every concept is UTF-8 Markdown with a parseable frontmatter mapping and nonempty
string `type`. Unknown types and additional frontmatter fields remain valid.
Optional trust, lifecycle, and provenance fields are not mandatory, and broken
links are not conformance failures. Validation does not claim to verify content
accuracy or execute attested computations.

`index.md` and `log.md` are reserved. Only a bundle-root index may carry
frontmatter, containing `okf_version`. Nested indexes do not have frontmatter.
Log date headings use ISO dates. Bundle-relative `/links.md` resolve inside one
package. Relative cross-package links should be accompanied by declared package
dependencies. Gnosis does not rewrite or validate link targets.

Exactly one `package.yml`, `package.yaml`, or `package.json` declares
`gnosis_version`, package name, descriptive version, description, one owner,
and dependency names. Structural management metadata uses strict decoding;
concepts remain extensible. Package names match directory names.

Every package carries `standard.yml`, a referenced JSON Schema for frontmatter,
and an index template. The policy can also require files, exact heading lines,
and a maximum whitespace-token count in each concept body. These stricter
requirements are package policy, not OKF requirements. The baseline `type`
requirement cannot be disabled. This architecture package demonstrates a
stricter policy requiring titles, descriptions, sources, and decision headings.

Schema references are package-local. No remote schema fetches, executable hooks,
or package symlinks are allowed. Default indexes use deterministic ordering and
escaped Markdown links; generation changes only indexes, not concept content.
Hand-authored indexes require explicit replacement.

The root registry maps globally unique local discovery names to Git repository,
repository-relative path, and default branch. Descriptions and owner labels are
discovery metadata; the fetched source manifest remains authoritative.
Repository discovery reads immediate-child manifests and optional registry
mappings. Changing a name's source requires an explicit registry edit.

`gnosis.lock` records each imported package's source, source commit, split
subtree commit, version, and dependency names. SHA pins, not semantic-version
constraints, determine reproducibility. Existing dependency pins stay unchanged
unless explicitly updated. Dependencies must be acyclic and available.

Registry metadata, package standards, generated indexes, and locks belong in
Git. Runtime locks, pending-merge records, and proposal checkouts live inside
the Git directory, not inside distributable knowledge bundles.
