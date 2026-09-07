# Neumannarch — project state (overseer-facing)

Present and future tense only: what is in flight, what is queued, the
rules every brief carries, the engine gaps verified open, and what is
settled. Nothing dated stays: a landed or resolved item is deleted in
the session it lands, never annotated, and a ruling that changes the
game goes into DESIGN.md or DISPLAY.md, never here. The commit message
is the record. Agents see this file only through their briefs.

## In flight

Nothing is in flight.

Next: the belt and the star, plan item 3, pulled ahead of every
cleanup pass (owner, 2026-09-07: visual and gameplay changes first,
the plan's order bent for them). Its design is ruled with the owner
as mechanism before a brief carries it. The starfield is an engine
feature the owner adds in parallel (owner, 2026-09-07); the game unit
draws the star and the belt and takes the sky when it lands. Ships
should move faster: the movement limit is a belt number and is set in
the belt unit with the spacing and the schedule bound. After it, the
next visual and gameplay units in the owner's order: ship speed and
combat feel through the harness; the asteroid bars and flight line
unit (Docs ahead of code, below); camera and hotkeys.

Open from the time split: extraction, construction and fulfilment act
per tick with a fixed second's worth of work rather than integrating
the match time that passed, so the step still returns early during the
draft. The complete shape is every phase taking the step's match-time
span, zero in the draft, and no early return. Contraction audit.

Open from the draft unit:
- `harness draft` on the fixed belt: the first picker lost 8 of 8
  mirrors. Caps are even (both seats' two asteroids sum to 17); position
  is not: the first picker's asteroids lie at the belt's ends, the
  second's sit adjacent in the middle. The fixed table's geometry; for
  the belt unit and the harness.
- The owner's arrival scene, the probes flying into the system during
  the draft: display only, not yet designed.

## Open, each for a ruling or a unit

- The naming pass over the sim crate (owner, 2026-09-06), the owner
  judging it from the diff: read-only Haiku surveyors in parallel, one
  per module group (state, step, orbit and roster, history and the
  rest), each returning a table of symbol, file, one sentence of what
  it holds, and the proposed name under the naming rule; the overseer
  filters, the owner rules on the list, one Sonnet applier renames
  workspace-wide under the gate, one commit; the other crates follow
  if the sim's proposals read well, else the survey moves to Sonnet.
  Deferred behind the visual and gameplay units (owner, 2026-09-07).
- No look scene hovers a plus or minus button over the stockpile bar,
  so the cost and refund segments are pinned by test and unseen; a
  look scene for each is a small display item, after the owner's play
  has judged them.
- Tuple keys outside the preview unit's files, about thirty maps: an
  asteroid with a seat as a bare pair in `step/fire.rs`,
  `step/extraction.rs` (`Income`), `display/fights.rs`; a row at a
  post as a pair or triple in `agents/plan.rs` (`targets`,
  `standing`); and tuple returns in `display/glyph.rs`, `icon.rs`,
  `glyph_quad.rs`, `personality.rs`, `harness.rs`. One Sonnet sweep
  under the tuples rule, deferred behind the visual and gameplay
  units.
- Second derivations in the agents, found 2026-09-06: `survey.rs`
  tallies the seat's holdings from `view.present` where the view
  already carries `compositions` from `State::holdings`; with the four
  rank sorts under 1e. A Sonnet unit after the preview.
- Ship glyphs stairstep: the quads use an alpha-test cutout MSAA cannot
  soften. Ruled: a coverage-sampled glyph rasteriser, edge coverage in
  the sheet's alpha, no material change. A small display unit.
- A thousand units at one asteroid cost 8.3 to 8.5 ms a tick against the
  8.333 ms budget (release, 21-asteroid belt); rank the roll once per asteroid
  and plating. The state is cloned whole every tick and 240 clones are
  kept (`Retention::shipped`, every tick over a two-second window), so
  every field added to the state is copied 120 times a second; the
  cadence constant already exists and a rewind of the whole window
  costs 26 ms at 100 entities.
- Time (owner's worry, 2026-09-06): `TICKS_PER_SECOND` is 120 and the
  game paces the sim at that rate of wall time with no speed control;
  verified 2026-09-06, no time scale exists in the game. Every "per
  second" in the sim is a sim second; if a speed control is added,
  rates stay sim seconds and only the pacing changes. Into the
  contraction audit: the 49 non-test uses of `TICKS_PER_SECOND` in the
  sim against one `Tick` vocabulary.
- Panics by raw index (owner, 2026-09-06): `Index` impls on `State`
  and a handful of `as usize` lookups panic on an id the store did not
  mint. Structural fix for the contraction audit or the columns unit
  (plan 5): stores whose ids are minted only by the store, so an id in
  hand is valid by construction and no lookup can fail.
- Dead `pub` items rustc cannot see: a `pub` item in a library crate is
  never flagged unused, so a crate's surface hides dead code from the
  gate. The sim is done: every item outside `lib.rs`'s re-exports is
  `pub(crate)` and rustc's lint now finds the rest. Remaining, by
  count of `pub` items: protocol 64, agents 38, game 375. Into the
  contraction audit (1e); `check.sh` gains the scan until then.
- No panics anywhere in the game or the sim (owner, 2026-09-06,
  distant): every panic path becomes error propagation and handling;
  with the id-minting stores above and the raw-index item. A crate-wide
  audit, after the belt.
- Camera controls are poor (owner, 2026-09-06); future work, no unit
  yet. Kept beside the hotkeys item since both are input.
- Hotkeys: the wheel's bands take the pointer only, and strategy players
  require hotkey play; the wheel's shape makes a binding per row and per
  step hard to add. Kept in mind for every wheel change; no unit yet.
- Whether a flying unit can be re-sent, its schedule solved from its own
  body mid-flight, when its destination's want falls, or whether a send
  is a commitment. A design ruling.
- Standings in a match: a page over the match toggled by a key with
  per-team standings, shaped as Beyond All Reason's stats page. A design
  conversation on its content before DISPLAY.md gains it.
- Fable's wheel proposals, not built, for the owner: asteroid names in
  phrases; total HP on the fight bar's hover; a live send line from
  wheel to pointer; the hint phrase advancing.
- Held for the owner's play: asteroids sub-pixel at region zoom; the fight
  arc refilling on reinforcement; repair at 15 HP/s beating a frigate's
  12 DPS; elimination before the clock; a fresh-eyes judgement after
  every display change; ships too slow and combat slow to start and
  random-feeling in outcome, both weighed by the harness.

## Docs ahead of code

One line per sentence of DESIGN.md, DISPLAY.md or ARCHITECTURE.md the
code does not yet do, naming the unit that lands it. A brief quotes its
lines from here; landing deletes them; the overseer reads this section
against the code at every session start.

- DISPLAY Resources: an extractor wanted or building at an asteroid shows
  its coming yield on the asteroid's bar; not drawn. The extractor-coming
  unit.
- DISPLAY Flights: the line ahead of a ship follows its schedule's
  path; the code draws a straight line. The extractor-coming unit.
- DISPLAY Resources: the small state is bars alone, tight to the
  asteroid; the code draws icons at both states. The extractor-coming
  unit.
- DISPLAY Lobby: no Seat column; the lobby still draws one with the
  seat's number in a square of its colour (screens/lobby.rs). Plan 6.
- DISPLAY Lobby: the team choice shows a square of the team's colour in
  the closed control and the open list. Not built. Plan 6.
- DISPLAY Rings and Lobby: colour is the team's everywhere; every
  painter colours by seat through `seat_color32`. Plan 6.
- DISPLAY Title: Settings is not drawn; Quit closes the game on the
  desktop through `ctx.close` and is not drawn in the browser. Both
  drawn disabled with a reason (screens/title.rs). Plan 6.
- DISPLAY Play: Surrender is not drawn; drawn disabled (screens/pause.rs).
  Plan 6.
- DESIGN World, Entity and Sends: one movement limit; the sim solves one
  schedule per row from a per-row acceleration. Plan 1e or the belt.

## Plan, in order

Priority (owner, 2026-09-07): visual and gameplay changes first, the
belt since its asteroid and ship counts set every other number, then
the readings the owner needs to judge by play, then the contraction,
then measurement before any store rewrite, then the screens, then the
programme.

1. The belt and the star (item 3), then ship speed and combat feel
   through the harness, then the asteroid bars and flight line unit
   (Docs ahead of code), then camera and hotkeys, before the
   contraction audit.
1e. Contraction audit, read-only then units: the codebase is much
   larger than it has any right to be for this amount of game. Survey
   for types whose fields copy another type's, parallel indexes over
   what the state answers, per-tick bags of borrowed values. Each
   finding becomes a deletion in the unit that next touches its file,
   or its own Sonnet unit with a ceiling. Known: `display::scene::
   Client` passes two of Scene's own fields through a constructor from
   four call sites; four identical rank-by-score-then-id sorts in
   agents/plan.rs and roles.rs.
2. Vectors, Sonnet: the connection's inboxes and both websockets'
   queues gain a stated cap on frame count, past which the connection
   closes; the kept records as a deque; the agent memory's asteroid sets as
   sorted slices; about 75 never-mutated fields and returns become
   `Box<[T]>` and `Box<str>` at their constructor sites (protocol 6,
   server 9, sim 19, agents 12, game 15). Net lines below zero.
3. The belt and the star, Opus: map generation from seed with regional
   caps; one to two hundred asteroids in an annulus from the seed with
   regional cap triples; neighbour spacing near two kilometres, the
   extent growing with the count; the star in the middle of the system
   as the central mass drawn, and a starfield skybox, both engine
   questions first since the engine deleted its sky; the lobby's seed
   changes the belt and its preview is the seed's belt at whole-belt
   zoom. The zone radius tens of metres, from the largest force an
   asteroid holds at the holding rule's spacing. Settles the asteroid
   count, the belt's spread against the schedule search bound, and the
   ship count a match reaches, which items 4 and 5 answer to.
4. The harness for scale and truth, Sonnet: a scale check playing a full
   bot match at two and five thousand ships on a release build printing
   milliseconds per tick; a per-tick invariant mode checking while a
   match plays (no thrust above a limit, every flier on an unended
   schedule, wants against holdings after fulfilment, a re-stepped
   tick's hash against the kept one, stock within capacity, both
   machines' settled hashes) naming the tick and the rule on a
   violation, extended with bot-behaviour guarantees (a bot builds an
   army and attacks when it beats the defence); the win-loss matrix
   against an expected table as an ignored test or a harness command
   run before a sim commit and after a balance change, the gate kept
   fast. With it the test audit: every sim test mapped to the DESIGN.md
   sentence it pins, read-only report first, then the tests with no
   sentence deleted.
5. The entity columns, Opus, on item 4's number: the store transposed to
   one column per field over a dense position with a sorted id index,
   an entity a view over the columns, iteration in id order, `Vision`
   built once per step, manoeuvring folded into propagation,
   `PerSeat<T>`; ready moments onto the entity; one hash re-baseline.
6. Screens on egui's own layout and widgets under one `Style`, driven
   headlessly through `Session::offer_ui`; deletes `control.rs`,
   `field.rs`, every `Places` and width constant; the lobby loses its
   Seat column and the team choice carries the team's colour square;
   colour is the team's everywhere; a control for an unbuilt feature is
   not drawn; Quit through `ctx.close` on the desktop and not drawn in
   the browser; `set_tick_interval` for pacing instead of dropping
   steps; button text centred. Opus. Clears five ledger lines.
7. Held for a ruling after item 4's numbers: the solve leaving the sim
   as a stamped schedule command from the owning machine, validated in
   apply by one integration against the tolerance and the limit; then a
   planner memo by place pair and quantised phase. The overseer's view:
   right if a played match shows the solve in the tick's budget.
8. The structural programme, each unit rewriting ARCHITECTURE.md: the
   small collapses (View::want; Room folded into Socket; one
   belt-drawing preamble); a frame on its
   post; one asteroid type from sim to pixel; one mark state replacing Fill
   and Reason; Layout yielding placed marks and one Dial; mark geometry
   as primitives painted once and rasterised once; one Log type; the
   agent's knowledge as one row per asteroid; the wheel built once on
   selection; the Maneuver phase spelt manoeuvring; the units question:
   evaluate one existing dimensional-analysis crate with const-generic
   dimensions, adopt only if already in the cargo cache and its bounds
   stay out of the rules. Opus for the sim stores and the agents; Sonnet
   for the rest; a line ceiling per brief; `check.sh` gains a
   duplication check.
9. From play: a selected asteroid owns the focus each tick until a pan
   releases it (DISPLAY Camera, main loop).
10. From the harness: seat 0's edge isolated and removed; territory that
    varies with composition; combat before the last third of a match;
    the tick-rate-doubling check; timeouts instead of iteration caps in
    the slow tests.
11. Later: fog in the display; gamepad; the twelve-slot wheel; hulls as
    meshes with a level-of-detail rule, the stencil icon, and a DESIGN
    line that a faction skews the hull's dialect and never the glyph;
    Haiku playtesting agents; a room list over the server; nicknames on
    the wire; factions as skews over one roster.

## Operational

- The engine is a path dependency at `../../mirage-engine`; its wgpu 29
  and egui 0.35 pin is its own concern. Its verification recipes are in
  `docs/verifying.md` there.
- The browser: `game/tools/serve-web.sh` builds the game for wasm32,
  binds it with the wasm-bindgen CLI pinned by the lockfile, serves
  `dist/` with the engine's page; a WebGPU browser is required. No
  browser run has been made yet; the first is the owner's.
- Linear is not set up and is ignored.
- Crate downloads are blocked in the agents' environment, so a new
  dependency must already be in the cargo cache. In it: resvg and usvg
  0.45.1, tiny-skia (in the lockfile), kurbo.
- One implementation agent at a time. Every brief carries the rules
  below.
- Every file edit through the Edit and Write tools, never a shell
  script, sed or heredoc; scripted edits produce mistakes the owner has
  to correct. No relaxation for sweeps.
- No comments of any kind. A comment is replaced by a type or an
  invariant upheld at compile time wherever one can carry the fact. One
  test per guarantee a module makes to its callers, named as the
  sentence of that guarantee; no test per branch, field, helper or
  identity; a test no plausible wrong implementation fails is deleted.
  Tests share one fixture module per crate.
- Naming: a function is named by what it returns or does in the game's
  words, never `of`, `get`, `handle`, `process`, `run`, `update`,
  `helper`, `util` or a suffix; a type is a noun the design documents
  use; a test is named as the guarantee sentence, never with `test`,
  `works`, `should`, `check`. A confused agent is a naming defect. A
  name has as many words as a design-document reader needs to know
  what it holds without opening it, usually two, never capped; a
  design noun stands alone; a coined single word is a defect, for
  fields and methods too (owner, 2026-09-06).
- Tuples (owner, 2026-09-06): a tuple of three or more members is
  always a defect, and any tuple that keys a map, is a field, or
  crosses a function boundary becomes a struct with methods and its
  fields private where it can; a tuple inside one expression, such as
  a sort key, is fine. An asteroid with a seat is `Post`; a row at a
  post is `Posting`.
- Placement: a computation lives on the type that is its subject and
  there is one computation of each fact; before writing a derivation an
  agent searches for the type that owns the fact and extends it; a
  second derivation anywhere is a defect.
- Store rule for every sim brief: no rule holds an entity across a tick
  or indexes the entity store by anything but an id, so plan item 5's
  columns land without touching a rule.
- Three line budgets reported separately, code, tests and docs; the
  code budget below zero and the test budget below zero unless a new
  guarantee has no old test to replace. ARCHITECTURE.md sections are
  the type block plus the facts the block cannot say.
- Words on screen: every user-facing string is a short phrase, no full
  stop, semicolon or dash; a comma is escalated to the owner before it
  is drawn (DISPLAY.md "Words on screen").
- Match setup, bots included, is the start menu's job; the playable
  takes no command-line arguments.
- Visuals: a Fable agent under the owner's direct steering through the
  overseer chat; a fresh-eyes judge on screenshots after.
- Easing: two spans and no other, `ease::Span { Fast, Slow }`, 20 ms
  for what the pointer causes and 100 ms for the camera and the
  screens (DISPLAY The wheel); no other easing constant is added.
- The engine moves under the owner's parallel work; a game unit
  expects its signatures to change mid-unit and reports each break as
  engine friction rather than working around it.

## Engine gaps, verified open 2026-09-06

Reported to the owner as found; the game never works around a gap.

- No text measure without a painter: cell widths are guessed at 0.6 em
  per digit; a measure on `FrameCtx` would delete the guess.
- The offscreen `Session` cannot drive the scroll wheel: `Motion::Wheel`
  is filled only by a winit event, so wheel zoom and the send drag's
  wheel count are verified through a key binding instead.
- The offscreen `Session::step` reports a frame `dt` of zero, so easing
  by frame delta freezes headlessly; the game eases by elapsed game
  time. A headless frame carrying a duration would let the drive test
  time-based display behaviour.
- `FrameCtx` exposes no cursor icon.
- `Key` has no punctuation keys, so zoom is bound to Q and E.
- `FrameCtx::dt` on the tick and the frame contexts share a name for a
  fixed and a variable step.
- egui strokes have no round caps without a second overlapping shape,
  which forces a blend-toward-backdrop fade instead of alpha.
- The overlay's MSAA sample count and egui's tessellation options are
  not exposed; whether the headless target resolves MSAA is
  undocumented.
- Offscreen input never reaches the UI layer: `press` and `set_pointer`
  are read by the game and `offer_ui` by egui, never both, so a screen
  built from egui widgets cannot be verified headlessly; plan item 6
  depends on this.
- A drag pan under perspective is inexact: `pan_by_pixels` scales at
  the focus's depth and lands a few percent off on a long drag; a
  camera-side "pan so this world point lands on that pixel" would be
  exact.

## Settled, do not re-raise

- No command budget as a rule of the game; caps against hostile input
  are an engineering matter inside the relay and `apply`.
- Ships are never drawn in a layout the sim does not have.
- No per-ship health bars.
- No points, no anchors, no objectives, no intrinsic compositions.
- Fixed-point numerics; per-row speed limits.
- A commander or any vital row; fixed seat quadrants; a staging place
  per seat; asteroid gravity or patched conics.
- Attachment or reference frames of any kind: every body moves under
  one law; an anchor is an orbit, not a frame.
- Slots or any fixed formation lattice: formation is emergent from the
  holding rule, which is one replaceable module.
- Client-side prediction of the sim's response to a want change: the
  preview is the sim's answer or nothing.
- No line or ribbon primitive in the engine: the HUD is painted in
  screen space through egui's painter over `Camera::pixel_of`.
- Hand-rolled dimensional newtypes (owner: madness).
- The small wheel and the asteroid bars at fixed pixels at every zoom,
  overlapping when crowded; nothing hidden by zoom.
- A deselected bare asteroid's wheel vanishes rather than shrinking.
