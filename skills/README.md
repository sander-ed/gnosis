# Gnosis skills

Eight composable skills; `skills/` is the canonical distribution.
[gnosis-orchestrator](gnosis-orchestrator/SKILL.md) is the single entry point:
it routes each request and sequences multi-step workflows. The other seven are
specialists that return results to their caller. There is no `gnosis-add` skill
or standalone ingestion skill.
[EVALS.md](EVALS.md) contains acceptance scenarios and recorded results.

## Choose a skill

Start with `gnosis-orchestrator` unless you already know the specialist you need.

| Skill | Use for | Result |
| --- | --- | --- |
| [gnosis-orchestrator](gnosis-orchestrator/SKILL.md) | Any Gnosis request; routing and multi-step workflows | Workflow report with each skill's result |
| [gnosis-navigate](gnosis-navigate/SKILL.md) | Existing catalog questions or placement context | Read-only navigation result with evidence and gaps |
| [gnosis-cli](gnosis-cli/SKILL.md) | Command syntax, effects, and limitations | Factual reference, not an executed workflow |
| [gnosis-update](gnosis-update/SKILL.md) | A precise change brief or approved draft | Validated local change receipt or explicit blocker |
| [gnosis-author](gnosis-author/SKILL.md) | New/unstructured knowledge, source ingestion, or bounded evidence extraction | Approved draft packet for the writer, or evidence-only packet |
| [gnosis-publish](gnosis-publish/SKILL.md) | Explicit upstream refresh or contribution preparation | Refresh/proposal receipt, not an implicit submission |
| [gnosis-review](gnosis-review/SKILL.md) | Checking that concepts or drafts work for a reader with no context | Goldfish review report; read-only |
| [gnosis-drift](gnosis-drift/SKILL.md) | Checking that knowledge still matches the code or sources it describes | Drift report; read-only |

Examples, all of which `gnosis-orchestrator` routes as shown:
"What does our catalog say about retries?" uses `gnosis-navigate`.
"Capture our incident playbook from these notes" runs `gnosis-author`, then
`gnosis-update` after approval.
"Replace the timeout in this concept with the specified value" can use
`gnosis-update` directly. "Refresh imported knowledge" uses `gnosis-publish`.
"Would a new engineer understand this runbook?" uses `gnosis-review`.
"Is our deployment knowledge still accurate?" uses `gnosis-drift`.
"How do I install a package?" uses `gnosis-cli`'s `gnosis add` reference.

No skill owns workspace setup or dependency installation. Those commands remain
reference-only in `gnosis-cli`. Authoring and upstream movement are separate
requests; an imported package can be edited locally without refreshing it.

## Composition and handoffs

`gnosis-orchestrator` is the only router. It classifies the request, runs the
owning skill, and decides the next step from what that skill returns: for
example, it hands the author's approved draft packet to `gnosis-update`, a
review request to `gnosis-review`, and the review report back to the author.
Specialists do not call each other to continue a workflow; they return results
or out-of-scope needs to their caller. Specialists may still consult read-only
helpers such as `gnosis-navigate` and `gnosis-cli`, and use shared references.
The orchestrator never grants approvals or starts publication without a request.

Callers use exact skill names and linked instructions. If the host supports skill
invocation, invoke the relevant skill; otherwise read its `SKILL.md` and perform
that role in the current agent. No host-specific tool, subagent, or orchestration
framework is required, except that `gnosis-review` needs a fresh reviewer context
and stops without one. Host permissions and safety rules still apply.

Each receiver owns its input/output contract. Pass short labeled Markdown
sections, reusing conversation context rather than making the user fill in
forms. Preserve the active brief and approval scope across role changes. Load
dependencies on demand and reuse current navigation and evidence. A missing
skill is an installation failure, not an invitation to improvise its procedure.

`gnosis-author` owns intake, readiness, and the full/fast approval paths, and
returns an approved draft packet instead of writing.
Its [source-extraction reference](gnosis-author/references/source-extraction.md)
also serves evidence-only requests without starting intake.
`gnosis-update` owns content application and the
[contributor-policy reference](gnosis-update/references/contributor-policy.md),
which publication reuses. Domain agents can supply precise approved changes to
the writer without delegating their subject-matter decisions.

`gnosis-review` and `gnosis-drift` never write knowledge. Review checks that
knowledge is usable without context; drift checks that it still matches its
sources. Neither is factual verification by a person. Both return findings, and
the user decides which ones the orchestrator routes to `gnosis-author` or
`gnosis-update`.

Keep temporary drafts, evidence notes, and approval state in the conversation or
host scratch space, not the knowledge catalog. Instructions are in English;
knowledge remains in its target package's language and conventions.

## Local installation (macOS/Linux)

Install all eight skills together so relative dependencies resolve. Run this
from the repository root. It preflights every destination, preserves unrelated
skills, leaves matching links alone, and refuses to overwrite anything else.
It does not remove old installations or change global agent settings.

```sh
(
  set -eu
  for parent in .agents .agents/skills; do
    if [ -L "$parent" ] || { [ -e "$parent" ] && [ ! -d "$parent" ]; }; then
      printf 'Inspect unexpected installation parent: %s\n' "$parent" >&2
      exit 1
    fi
  done
  for legacy in gnosis gnosis-add; do
    path=".agents/skills/$legacy"
    if [ -e "$path" ] || [ -L "$path" ]; then
      printf 'Legacy skill needs explicit removal approval: %s\n' "$path" >&2
      exit 1
    fi
  done
  for name in gnosis-orchestrator gnosis-navigate gnosis-cli gnosis-update \
      gnosis-author gnosis-publish gnosis-review gnosis-drift; do
    path=".agents/skills/$name"
    target="../../skills/$name"
    test -f "skills/$name/SKILL.md"
    if [ -L "$path" ] && [ "$(readlink "$path")" = "$target" ]; then
      continue
    fi
    if [ -e "$path" ] || [ -L "$path" ]; then
      printf 'Preserving unexpected destination: %s\n' "$path" >&2
      exit 1
    fi
  done
  mkdir -p .agents/skills
  for name in gnosis-orchestrator gnosis-navigate gnosis-cli gnosis-update \
      gnosis-author gnosis-publish gnosis-review gnosis-drift; do
    path=".agents/skills/$name"
    if [ ! -L "$path" ]; then
      ln -s "../../skills/$name" "$path"
    fi
    test -f "$path/SKILL.md"
  done
)
```

Expected links are `.agents/skills/NAME -> ../../skills/NAME`. `.agents/` stays
ignored: only canonical files and these instructions are distributed. If an old
combined `gnosis` skill or a `gnosis-add` installation exists, inspect it and
obtain permission before removal; replace it with this suite, whose router is
`gnosis-orchestrator`.

Verify all relative links from both `skills/` and `.agents/skills/`, particularly
publication's cross-skill contributor reference. In Copilot CLI, run
`copilot skill list` from this repository and confirm all eight names appear under
project skills. For other hosts, use their skill-discovery UI/command.
Filesystem links alone do not prove discovery. An already running agent may have
cached its skill list; start a new session if the names are absent. Do not change
global settings to hide a discovery problem.

If directory symlinks are unsupported, copy each of the eight canonical folders
into a new `.agents/skills/NAME` destination with `cp -R skills/NAME
.agents/skills/NAME` after the same collision/legacy checks. Do not mix copies and
links unknowingly. **Refresh copies after canonical changes:** compare each with
`diff -ru skills/NAME .agents/skills/NAME`; preserve any unexpected local edits,
move the old copy to an explicitly chosen backup location outside scanned skill
directories, and copy the current canonical folder into the now-absent
destination. Recheck links and discovery. Never edit installed copies as a
second canonical source or overwrite a stale copy blindly.

## Validation

Use the structural checklist and behavioral procedure in [EVALS.md](EVALS.md).
There is deliberately no Rust skills test, new evaluation framework, or product
CLI change. All mutating fixtures belong in disposable Git workspaces, never
this repository's real catalog. Structural checks do not establish agent
behavior, factual correctness, or human verification.
