# Probe Game — rules for working in this repo

The overseer reads OVERSEER.md before anything else. Implementation agents do not.

## Invariants (every change, no exceptions)

- to be filled in

## Code quality bar

The standard is the highest: code an experienced Rust reviewer would sign
off on without comments.

- Idiomatic Rust per the API Guidelines: precise names, newtypes over bare
  primitives where meaning exists, iterators over index loops, no `clone()`
  to dodge a borrow you could restructure.
- Loose functions are a missing-type indicator (owner, 2026-08-29): a
  function whose signature has a clear primary subject belongs in that
  type's impl; one whose subject has no type yet is evidence the type is
  missing — create it, then the function is its method. A free function
  stays only where no argument is the subject (min/swap-shaped peers).
- Concise rustdoc on every public item, stating contracts (defaults, units,
  when things run) — never restating signatures. One line unless a contract
  genuinely needs more: what it is, then when you need it, plain
  subject-verb-object, one fact per sentence. Design rationale lives in
  ARCHITECTURE.md, never in rustdoc; no worked examples on ordinary items.
  Calibration (owner-supplied): "Definition for repaintable sections of a
  mesh. Required if you want to change a material when drawing something."
- Comments are a last resort: if you reach for one, factor instead — extract
  a named function or type until it is unnecessary. Survivors state only
  what code cannot (safety contracts, platform quirks, why not the obvious
  way), one line preferred, two at most. Module docs are a couple of lines.
- Literal register only: docs and comments state what the code does in
  literal verbs — no figurative or anthropomorphic phrasing.
- The fix is the structural fix: when a defect admits a type-level or
  engine-level answer, that is the one to implement; mitigations and
  game-side workarounds are stopgaps, never the recommendation, and churn
  is no counterargument. "Parked" applies to speculative features only.
- A soft-failure seam is presumed eliminable — unspellable, boot-fatal,
  or an uncapped store; "justified-dynamic" must survive the challenge:
  what shape change would delete you? (owner, 2026-09-01)
- Never the cheap seam (owner, 2026-08-27): when a design admits a
  fully-enforced shape, pay the churn for it — there is no rush. A
  documented hole is still a hole.
- No dead code, no placeholder stubs (an architecture-required item may land
  before its driver, but with a real body and documented contract), no
  `#[allow]` without a justifying comment, no commented-out code.

## Workflow

- The owner controls git. Implementation agents never run a mutating git
  command — working-tree edits only, scoped to the assigned milestone. The
  overseer commits ONLY on the owner's explicit word — a per-commit
  "commit", or a standing "commit when ready" (given 2026-08-27), which
  authorizes committing each unit once it is verified AND its fresh-eyes
  reviews are clean: one commit at a time, a one-sentence message,
  announced when made. The message states the change's behavior in the
  engine's vocabulary, the repo register — never process nouns
  (milestones, units, reviews, verification), which live in TODO and
  reports (owner, 2026-08-29). Design docs commit alongside code,
  never alone.
  Read-only git is always fine.
- Model policy: engine internals go to Opus agents;
  Reuse an agent while its context is low; relaunch on an egregious
  or repeated mistake, or high context.
- Critique before building on unfinished work (owner, 2026-08-31): an
  agent dispatched onto an in-flight tree FIRST reviews what it
  inherits — reading the working-tree state its phase builds on and
  reporting defects, doubts, and shapes it would not have chosen —
  and implements only after the overseer answers that critique (fix,
  steer, or proceed). The reading is paid for anyway, and the critique
  sets the adversarial footing the repo expects before any code is
  written on top.
- Visuals are judged only by an agent that never saw the builder's code
  or reasoning, against screenshots. It may be reused. A milestone's changed
  docs likewise get a fresh-eyes register review before owner review,
  flagging: needs-a-second-read sentences, undefined coined nouns, missing
  units/defaults, signature restatement.
- If the index or files change while you work, that is the owner steering —
  leave their changes alone.
- Never open a window on the owner's desktop; verify headlessly (xvfb-run,
  or the offscreen Session — recipes in docs/verifying.md).
- Complete the milestone, verify on both targets, then stop for owner
  review — never start the next unprompted. Report decisions and any
  friction with ARCHITECTURE.md.
- Owner questions go through the question tool the moment they exist; a
  note records a ruling, never a pending ask.
