# Neumannarch — rules for working in this repo

The overseer reads OVERSEER.md before anything else. Implementation
agents do not.

Neumannarch is a two-to-four-player space RTS: a deterministic lockstep
simulation of ships, structures and asteroids, played through one verb, on
the Mirage engine (a path dependency at `../../mirage-engine`) for desktop
and the browser. **Read DESIGN.md before writing sim code and DISPLAY.md
before writing display code.** They describe the target only and are the
authority on the rules of the game and on what the player sees. If
implementation reveals a problem with a design, stop and propose a change to
the document; do not silently deviate.

## Layout

- `sim/` — the simulation: state, commands, the step. No engine, no
  platform code, no floats but `f64`. Builds on both targets.
- `game/` — the playable: the Mirage `Game`, rendering, input, UI, the
  relay client. The only crate that draws.
- `protocol/` — every value two machines exchange. No io.
- `agents/` — the scripted opponents, and the `harness` binary: balance
  matrices and determinism checks over `sim`. Native only.
- `server/` — the match server: rooms, lobby authority, forwarding.

Dependencies point one way: `protocol` depends on `sim`; `agents` on
`sim` and `protocol`; `game` and `server` on all three; `sim` depends on
nothing in this repo and never on the engine.

## Invariants (every change, no exceptions)

- `./check.sh` passes before every commit: fmt, clippy `-D warnings`,
  native build, tests, wasm32 build of `game`.
- Determinism: the same initial state and command log produce the same
  state hash on every target. In `sim`: no `f32`; transcendentals only
  through `libm`, never `std`; no `HashMap` or `HashSet` (a `BTreeMap` or a
  sorted `Vec` instead); no clocks, randomness, threads or platform calls.
- Untrusted input never panics and never grows a store without bound: a
  command from the relay is applied or rejected by name, and every count a
  command carries has a stated cap.
- No blocking calls (`block_on`, `std::thread::sleep`, sync file IO) in
  `game` or `sim`; the rule exists for wasm safety.
- Prefer compile-time enforcement; a runtime check the type system
  could not express is listed in ARCHITECTURE.md's Invariants with the
  shape change that would delete it.

## Code quality bar

The standard is the highest: code an experienced Rust reviewer would sign
off on without comments.

- Idiomatic Rust per the API Guidelines: precise names, newtypes over bare
  primitives where meaning exists, iterators over index loops, no `clone()`
  to dodge a borrow you could restructure.
- No new type whose fields are copies of another type's fields (owner,
  2026-09-05). A type is added only where it owns a fact no existing
  type owns; a bag of values borrowed from an entity, a row or an asteroid is
  not a type, it is a function over them. Before adding a type an agent
  names the existing type that owns the nearest fact and extends it. A
  replacement system is built from the existing types first: a brief
  names the types the build must reuse, and a build that grows a
  parallel set of them is a defect.
- A function belongs to the type that is its primary subject; a subject
  with no type is a missing type — create it, then the function is its
  method. A free function stays only where no argument is the subject
  (min/swap-shaped peers). The counterweight: a type that gains methods
  from every module is also a missing type.
- No comments of any kind: no `//`, no rustdoc, no module docs (owner,
  2026-09-05). A contract is carried by the type, the name and the test
  that pins it; a unit, a default or a `None` case lives in a newtype, a
  constant's name or the return type. Anything a comment would have said
  belongs in the design docs or in a name, or is not worth saying.
- A confused agent is a naming defect (owner, 2026-09-05): when an agent
  misreads a type, a function or a field, the fix is a more descriptive
  name, or a new type so the thing can be named at all; never a comment,
  never a note in a brief.
- Literal register: names and the design docs state what the code does in
  literal verbs, no figurative phrasing. The game's own vocabulary (chase,
  leash, fire, spot, home, want) is literal here.
- The fix is the structural fix: when a defect admits a type-level answer,
  that is the one to implement; a workaround is never the recommendation,
  and churn is no counterargument. Every place a failure is tolerated at
  runtime — a fallback, a silently ignored input, a cap — gets one of
  three verdicts: made unrepresentable by an API shape, moved to a
  boot-time failure, or listed in ARCHITECTURE.md's Invariants with the
  shape change that would delete it. A documented hole is still a hole.
- No dead code, no placeholder stubs (an architecture-required item may
  land before its driver, but with a real body and a test of its contract),
  no `#[allow]`, no commented-out code.
- Small single-purpose modules; `pub(crate)` by default, `pub` only for the
  documented surface.
- A test is written from the guarantee, never from the fix's own geometry,
  and a test pinning a defect is shown to fail on the unfixed code. Every
  sim system lands with its unit tests.

## Workflow

- The owner controls git. Implementation agents never run a mutating git
  command: working-tree edits only, scoped to the assigned milestone. The
  overseer commits only on the owner's explicit word, once per commit, one
  commit at a time, a one-sentence message announced when made. The
  message states the change's behavior in the game's vocabulary, never
  process nouns (milestones, units, reviews, verification). Design docs
  commit alongside code, never alone. Read-only git is always fine.
- An ambiguity stops the work (owner, 2026-09-05): when a brief, a
  design document or the code admits two readings that lead to different
  work, the agent stops, reports the question with the readings it sees,
  and waits; it never picks one and proceeds. Stopping to ask is never
  a defect; guessing is.
- Critique before building on unfinished work: an agent dispatched onto an
  in-flight tree first reports the defects, doubts and shapes it would not
  have chosen in what it inherits, and implements only after the overseer
  answers that critique (fix, steer, or proceed).
- Red-state refactors: a replacement starts by deleting the old system,
  core and call sites, so surviving leaves cannot steer the new code into
  the old shape. The agent stops and reports at full red, and builds only
  after the overseer reviews the demolition. A unit still ends green
  before commit.
- Visuals are judged only by an agent that never saw the builder's code or
  reasoning, against screenshots, by the questions in DISPLAY.md. Changed
  docs get a fresh-eyes register review before owner review, flagging:
  needs-a-second-read sentences, undefined coined nouns, missing units or
  defaults, signature restatement.
- If the index or files change while you work, that is the owner
  steering: leave their changes alone.
- Create a new crate's files before naming it in any Cargo.toml; the
  owner's IDE caches a broken workspace otherwise.
- The engine is under active development and this game exists partly to
  polish it. A missing engine feature or friction with its API is reported
  in the milestone report as its own item, never worked around in the game.
- Never open a window on the owner's desktop. Verify headlessly: xvfb-run,
  or the engine's offscreen Session; the recipes are in
  `../../mirage-engine/docs/verifying.md`.
- Complete the milestone, verify on both targets, then stop for owner
  review; never start the next unprompted.
- Owner questions go through the question tool the moment they exist; a
  note records a ruling, never a pending ask.
- A torn-down system's history is owner and overseer reference only;
  agents never read superseded implementations, and never the prototype
  that preceded this repo. Briefs carry requirements, never old shapes.

Project state — pending work, queues, defects, uncommitted milestones —
lives in TODO.md. It is overseer-facing: the overseer reads it at session
start, briefs agents with only what their task needs, and maintains it
under its own discipline. Implementation agents work from their brief, not
from TODO.md.
