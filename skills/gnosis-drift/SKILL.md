---
name: gnosis-drift
description: Reports where Gnosis knowledge no longer matches the code, configuration, or sources it describes. Use for read-only drift or staleness audits of concepts or packages, not for answering questions, editing knowledge, or comparing local edits with upstream packages.
---

# Drift report

Knowledge decays as the systems it describes change. This skill compares
concepts with the current state of what they describe and reports only the
differences. A person decides whether the knowledge or the system is wrong.

## Contract

Receive a workspace; a scope of packages or concepts; and the approved source
scope, such as the repository the knowledge describes and any explicitly
supplied external sources. Ask for a scope rather than auditing the whole
catalog by default. Return a **drift report**.

This skill is read-only. Follow the read-only limits in
[source extraction](../gnosis-author/references/source-extraction.md#read-only-limits):
do not run code, tests, builds, or scripts to establish current behavior, and do
not expand source scope without asking.

Differences between local edits and an imported package's upstream version are
not drift; they belong to [gnosis-publish](../gnosis-publish/SKILL.md).

## Procedure

1. Use [gnosis-navigate](../gnosis-navigate/SKILL.md) to list the concepts in
   scope. For more than 15 concepts, confirm the scope with the user first.
2. Record the revision of each source you inspect, such as the workspace's
   `git rev-parse HEAD` and whether the worktree has uncommitted changes.
3. For each concept, collect its checkable anchors: `resource` and `sources`
   metadata, file paths, symbols, commands, configuration keys, values, URLs, and
   linked concepts. Note `verified` and `stale_after` metadata.
4. Locate each anchor within the approved scope. When a reference is vague,
   search for the named symbol or value with a targeted search, then read the
   current source. Do not rely on memory or on earlier reads in this session.
5. Compare each material claim with what you read and classify any difference:
   - **Contradicted:** a value, behavior, or rule differs from the source.
   - **Missing:** the described file, symbol, command, or option no longer exists
     or was renamed.
   - **Incomplete:** a cited source now contains material behavior the concept
     omits, such as a new required step or option. Check only sources the
     concept cites.
   - **Broken reference:** a link or source cannot be resolved.
   - **Expired:** `stale_after` is earlier than the current date.
   - **Unanchored:** a material claim has no checkable source. Suggest adding
     one so future checks are cheaper.
6. Report only differences. Do not describe what matches. Where you cannot
   inspect a source, report that as a limit, not as drift or as a match.

## Drift report

Return short labeled sections:

- **Scope:** concepts checked, sources and revisions inspected, current date.
- **Findings:** grouped by concept path. For each finding, give the kind, the
  quoted claim, and the evidence as path and line range, symbol, or URL. Write
  "No drift detected" for concepts without findings.
- **Limits:** sources not inspected, inferences, and claims that could not be
  checked.

Keep the report in the conversation. If the user asks for a file, write it
outside the `gnosis/` directory and do not commit it.

## Hand back

Return the report to the caller, normally
[gnosis-orchestrator](../gnosis-orchestrator/SKILL.md). Do not start fixes. For
each finding, the user decides which side to change. Knowledge changes go to
[gnosis-author](../gnosis-author/SKILL.md) or, when the correction is precise
and decided, to [gnosis-update](../gnosis-update/SKILL.md). Code or configuration
changes are outside this suite. For an imported package, note that the
correction may belong upstream through `gnosis-publish`.

Never update `verified`, `stale_after`, or other metadata here. A concept with
no drift has been compared with its sources at one revision; it has not been
verified by a person.
