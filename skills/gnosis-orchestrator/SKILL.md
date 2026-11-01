---
name: gnosis-orchestrator
description: Entry point for all Gnosis knowledge work. Works out what the user needs, routes to the right gnosis skill, and runs multi-step workflows such as capture, correction, review, drift repair, refresh, and contribution, carrying approvals and handoffs between skills. Use for any Gnosis catalog request when the specific skill is not already known.
---

# Gnosis orchestrator

Route each request to the skill that owns it, and run the steps of a multi-step
workflow in order. Do not do a specialist skill's work here: do not answer
catalog questions, shape content, write knowledge, or publish yourself. Do not
grant approvals or add approval gates of your own.

## Route the request

| The user wants to | Skill | Result |
| --- | --- | --- |
| Find or understand existing knowledge, or ask what the catalog says | [gnosis-navigate](../gnosis-navigate/SKILL.md) | Navigation result |
| Know a command, flag, or how to install or set up packages | [gnosis-cli](../gnosis-cli/SKILL.md) | Reference only |
| Capture new or unstructured knowledge, or ingest sources into the catalog | [gnosis-author](../gnosis-author/SKILL.md) | Approved draft |
| Research sources with no catalog change | `gnosis-author`, extraction-only mode | Evidence packet |
| Apply a precise change whose content is already decided | [gnosis-update](../gnosis-update/SKILL.md) | Change receipt |
| Check that knowledge is usable without prior context | [gnosis-review](../gnosis-review/SKILL.md) | Review report |
| Check that knowledge still matches the code or sources | [gnosis-drift](../gnosis-drift/SKILL.md) | Drift report |
| Refresh imported packages, or contribute changes upstream | [gnosis-publish](../gnosis-publish/SKILL.md) | Publication receipt |

When a request fits more than one row, choose by outcome: whether the catalog
changes, who decides the content, and whether anything moves upstream. If it is
still unclear, ask one focused question. If the user asks for several things,
state the order you will run them in and confirm it before writing anything.

Installation, workspace setup, and source configuration are reference-only:
answer with `gnosis-cli` and do not run them as part of a workflow.

## Workflows

Run only the steps the request needs. Each arrow is a handoff to the next
skill. A bracketed step is offered to the user, not run automatically.

- **Capture knowledge:** `gnosis-author` (intake through final draft) →
  [`gnosis-review` of the draft, when the author returns a review request] →
  `gnosis-author` (final approval) → `gnosis-update` → receipt.
- **Correct knowledge:** a precise, decided change from the user goes directly
  to `gnosis-update`. A correction that still needs wording, evidence, or a
  decision goes to `gnosis-author` first, which uses its fast path when eligible.
- **Review knowledge:** `gnosis-review` → the user picks findings → each accepted
  finding follows *Correct knowledge*.
- **Repair drift:** `gnosis-drift` → the user decides, for each finding, whether
  the knowledge or the system is wrong → knowledge fixes follow *Correct
  knowledge*. Code and configuration changes are outside this suite.
- **Refresh imports:** `gnosis-publish` (sync).
- **Contribute upstream:** [`gnosis-review` of the changed concepts, if none has
  been done since they changed] → `gnosis-publish` (propose).

When a skill returns a need it does not own, route it as a new step: for
example, `gnosis-update` returning unstructured knowledge goes to
`gnosis-author`, and `gnosis-author` returning a substantive deviation from
`gnosis-update` goes back to the author to reopen the affected approval.

## Handoffs

Keep a short workflow record in the conversation or host scratch space, not the
catalog: the request, the route, the current step, approvals with their scope,
and the artifacts so far (navigation results, evidence packets, draft
locations, reports, receipts).

Pass each skill only what its contract asks for, as short labeled sections.
Reuse current artifacts instead of making a skill redo work or the user repeat
context. Each receiving skill's contract decides what it accepts. Relay
approvals exactly as the user gave them; the approval-scope rules in
`gnosis-author` decide whether a relayed approval still holds.

Invoke skills by exact name when the host supports it; otherwise read the linked
instructions and perform that role. A missing skill is a broken installation:
report it and stop rather than improvising its procedure. No subagent is needed
except for `gnosis-review`, which stops if it cannot start a fresh reviewer.

## Moving between workflows

Never start publication, a sync, or a code change as a follow-on to another
workflow without the user asking for it. Finishing a local edit does not
authorize a contribution, and a review or drift report does not authorize edits.
When a workflow ends, offer the likely next step in one line and stop.

## Report

At the end of a workflow, report the route taken, each skill's result in one or
two lines, what changed and where, what is still open, and any blocker. Relay
skill receipts accurately: a structural check is not factual verification, and
a prepared proposal is not a submitted contribution.
