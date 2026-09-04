# Probe Game — project state (overseer-facing)

Present and future tense only: operational constraints, pending work,
settled decisions. An in-flight milestone gets one entry, deleted when
committed; the commit message is the record. DESIGN.md and DISPLAY.md stay
target-state only. Agents see this file only through their briefs.

## In flight

One implementation agent at a time (owner, 2026-09-03): for usage and to
avoid seams. A unit ends green through `check.sh`, is verified by the
overseer's own gate run, and commits on the owner's word. Last commit:
b561351, the `Stage` flow with one seating authority, 2026-09-05.

Owner's note on that commit (2026-09-05): the tree grew by about 700
net lines and the owner is worried by how much code lands per unit; the
best system is no system. Every brief from now carries a net-line
budget, the agent reports lines added against deleted, and a unit that
grows the codebase names each new type and what it made unrepresentable.

1. One movement limit for every unit (owner, 2026-09-05): the roster's
   acceleration field goes; the movement limit is one value of the roster
   (hashed with the state, a faction's to skew later); a row's kind
   derives from its manoeuvring limit (zero for a structure); a send is
   one schedule; DESIGN.md rewritten by the overseer (World, Materials,
   Entity, Sends; the slowest-row harness question deleted). Opus,
   critique stop first, in flight.

Discussed and held (owner, 2026-09-05), not scheduled: the solve leaving
the sim, with a send's schedule as a stamped command from the machine
that owns its units, validated in apply by one integration against the
tolerance and the limit, so the solver may warm-start, cache and run
off the step and the docs state only what the sim checks; and, only
after that, a planner memo keyed by place pair and quantised phase.
The overseer's view: the right shape, its own unit, after one speed.

Movement landed and committed da0a169 (2026-09-05): a schedule of two
burns built only from a delta-v and the limit, existence by an
eight-percent burn share of the span plus three aim corrections, coasts
as one propagation in the solver and tick by tick in the step, a flight
on its ship, no arrival snap, an unschedulable send opening frames.
Measured: 2.8 ms per accepted candidate, 5.8 ms per row per send.

## Docs ahead of code

One line per sentence of DESIGN.md, DISPLAY.md or ARCHITECTURE.md the
code does not yet do, naming the unit that lands it. A brief quotes its
lines from here; landing deletes them; the overseer reads this section
against the code at every session start. Verified 2026-09-05.

- DISPLAY Lobby: no Seat column; the lobby still draws one with the
  seat's number in a square of its colour (screens/lobby.rs). Unit 4.
- DISPLAY Lobby: the team choice shows a square of the team's colour in
  the closed control and the open list. Not built. Unit 4.
- DISPLAY Rings and Lobby: colour is the team's everywhere; every
  painter colours by seat through `seat_color32`. Unit 4.
- DISPLAY Title: Settings is not drawn; Quit closes the game on the
  desktop and is not drawn in the browser. Both drawn disabled with a
  reason (screens/title.rs). Unit 4 with plan item 6's `ctx.close`.
- DISPLAY Play: Surrender is not drawn; drawn disabled (screens/pause.rs).
  Unit 4.
- DISPLAY The stockpile: the whole section; the view carries no income
  or spend. Plan item 5.
- DESIGN World, Entity and Sends: one movement limit; the sim solves one
  schedule per row from a per-row acceleration. Unit 1.

## Plan, in order (owner and overseer, 2026-09-05)

Priority: holes against the invariants first, then the belt, since its
rock count and ship count set every other number, then measurement
before any store rewrite, then the screens the owner needs to judge by
play, then the programme.

2. Vectors, Sonnet: the connection's inboxes and both websockets' queues
   gain a stated cap on frame count, past which the connection closes
   (a peer past the cap has broken the protocol); the kept records as a
   deque; the agent memory's rock sets as sorted slices; about 75 fields
   and return types that are never mutated after construction become
   `Box<[T]>` and `Box<str>` at the constructor sites the 2026-09-05
   review named (protocol 6, server 9, sim 19, agents 12, game 15).
   The per-frame wheel rebuild joins item 9. Net lines below zero.
3. The belt and the star, design conversation first, then Opus: map
   generation from seed with regional caps; the star as a distant light
   and a disc; the lobby's seed changes the belt; the preview at
   whole-belt zoom. The conversation settles the rock count, the belt's
   spread against the schedule search bound, and the ship count a match
   is expected to reach, which is the number items 4 and 5 answer to.
4. The harness for scale and truth, Sonnet: a scale check that plays a
   full bot match at the stated ship count on a release build and prints
   milliseconds per tick; a per-tick invariant mode that checks while a
   match plays (no thrust above a limit, every flier on an unended
   schedule, wants against holdings after fulfilment, a re-stepped tick's
   hash against the kept one, stock within capacity, both machines'
   settled hashes) and names the tick and the rule on a violation; the
   gate keeps one short match through it and the long matrices and the
   scale check run before a sim commit. With it the test audit: every sim
   test mapped to the DESIGN.md sentence it pins, read-only report first,
   then the tests with no sentence deleted (the 2026-09-03 audit's ~185
   lines are the floor).
5. The entity columns with one vision per step, Opus, on item 4's
   number: the entity store transposed to one column per field over a
   dense position with a sorted id index, an entity a view over the
   columns, iteration still in id order, `Vision { sweep, sight per
   team }` built once per step, manoeuvring folded into propagation,
   `PerSeat<T>`; one hash re-baseline. Ready moments onto the entity in
   the same unit. Until then every brief carries: no rule holds an
   entity across a tick or indexes the store by anything but an id.
6. Screens on egui's own layout and widgets under one `Style`, driven
   headlessly through `Session::offer_ui`; deletes `control.rs`,
   `field.rs`, every `Places` and width constant. In the same unit: the
   lobby loses its Seat column and the team choice carries the team's
   colour square; colour is the team's everywhere; a control for an
   unbuilt feature is not drawn (Surrender, Settings); Quit through
   `ctx.close` on the desktop and not drawn in the browser;
   `pixels_per_point` on the frame context; `set_tick_interval` for
   pacing instead of dropping steps. Opus. Clears five ledger lines.
7. The stockpile bar, DISPLAY.md "The stockpile", Sonnet: the view
   gains income and spend per material; the HUD's one numeral
   exception; wheel slots dim when unaffordable with the short material
   on hover. Clears the last ledger line.
8. Held for a ruling after item 4's numbers: the solve leaving the sim
   as a stamped schedule command from the owning machine, validated in
   apply by one integration against the tolerance and the limit (the
   host's planner for a bot's seat; a left machine's units hold home);
   then a planner memo by place pair and quantised phase. The
   overseer's view: the right shape if a played match shows the solve
   in the tick's budget, and not otherwise.
9. The structural programme's remainder, in the audit's order, each unit
   rewriting ARCHITECTURE.md: the small collapses (Attractor is Body;
   View::want; Room folded into Socket where anything of it remains; one
   belt-drawing preamble; the sim test fixture module; Materials::
   bottleneck as binding; Material::ALL); a frame on its post; one rock
   type from sim to pixel; one mark state replacing Fill and Reason;
   Layout yielding placed marks and one Dial; mark geometry as primitives
   painted once and rasterised once, then re-judged; one Log type; the
   agent's knowledge as one row per rock; the wheel built once on
   selection; the Maneuver phase and module spelt manoeuvring; the
   unit-newtype question over every measured f64 field. Done already:
   the seating authority, the flow, the comment sweep, the flights on
   their members. Opus for the sim stores and the agents; Sonnet for the
   rest; a line ceiling per brief; `check.sh` gains a duplication check.
10. From play, ruled: a post builds one frame of a row at a time (DESIGN
    Build is flow and Shortfall, fulfilment, the ring's fills); a selected
    rock owns the focus each tick until a pan releases it (DISPLAY Camera,
    main loop).
11. From the harness: seat 0's edge isolated and removed; territory that
    varies with composition; combat before the last third of a match;
    the tick-rate-doubling check; timeouts instead of iteration caps in
    the slow tests.
12. Held for the owner's play or ruling: rocks sub-pixel at region zoom;
    the ring inside the rock at close zoom and far from the ships it
    counts; the fight arc refilling on reinforcement; repair at 15 HP/s
    beating a frigate's 12 DPS; elimination before the clock unreachable
    from a fogged view; crowd packing past half the spacing; a fresh-eyes
    judgement after every display change; the scout's manoeuvring at
    more than half the movement limit against DESIGN's "far below".
13. Later: fog in the display; gamepad; the twelve-slot wheel; hulls as
    meshes with a level-of-detail rule, the stencil icon, and a DESIGN
    line that a faction skews the hull's dialect and never the glyph
    (owner's models); Haiku playtesting agents over the same trait; a
    room list over the server; nicknames on the wire; factions as skews
    over one roster, the movement limit among them.

## Audit findings, 2026-09-03 (the reports themselves are gone; this is
## the record the programme in item 9 is built from)

Duplicated facts (Sonnet): `is_structure` written twice in agents
(plan.rs, survey.rs) and inlined twice in memory.rs, belongs on
`Roster`; `wanted(view, place, row)` identical in screens/play.rs and
display/send.rs, belongs on `View` as `want`; `Play::rock_pos`
re-implements `View::rock_body`; "which slots are seats" derived in
protocol, game and server (fixed by the seating authority); six
one-rock-one-seat test fixtures in sim/src/state/{mod,flight,sight,
radar}.rs and step/{maneuver,propagation}.rs (one `#[cfg(test)]`
fixture module); the WebSocket frame-size builder duplicated in
game/src/net/link/native.rs and server/src/stream.rs.
Tests (Sonnet): ~185 lines; delete glyph.rs's marks-table test, merge
shipped.rs's four per-field tests, delete wheel.rs's two duplicate
slot-glyph tests, fold lobby.rs's spare-row test, merge screen.rs's
absolute-position test into the sub-pixel one, fold ring.rs's lone-run
and compressed-run tests, one extraction case, one hash case; hoist
elements.rs's body triplet; the agents' play tests and the multiplayer
socket test are the slow ones and should wait on a timeout.
Comments (Sonnet): ~120 lines of design rationale in screens/flow.rs,
net/machine.rs, screens/play.rs, server/stream.rs, display/glyph_quad.rs,
main.rs and camera.rs (a duplicated block), agents/plan.rs; worked
arithmetic inside step/mod.rs's tests should become named helpers;
weak verdicts on malformed wire frames in net/socket.rs and
server/stream.rs (a design question: strikes and disconnect?).
Surface (Sonnet): `Income` should be a newtype over its `BTreeMap`;
`State::ready` and `entities_at_rock` are `pub` with no external
caller.
Agents and display (Sonnet): `Survey` copies three `Memory` facts
(`enemy`, `enemy_rocks`, `threats`); the dps-per-cost formula in
roles.rs and personality.rs with one guard missing; harness.rs binds an
unused tuple half; plan.rs (612 lines) has no direct unit tests; two
material palettes (display/hue.rs and tint.rs); polar "clockwise from
twelve" conversion written five times (wheel.rs, hud.rs twice,
wheel.rs brighten, stencil.rs arc); two Color-to-Color32 roundings
(tint.rs, glyph_quad.rs); `BeltCamera::local` duplicates
`Screen::local`; `Layout::of` computed up to three times per ring per
frame.
Structural (Opus), the programme's twelve items with their shapes:
frames on their post as `Composition { rows: BTreeMap<RowId, Wanted
{ count, frames: Vec<Frame> }> }` deleting index-as-identity and eleven
helpers (~150); one `Mark { row, state: MarkState, dim }` replacing
`Fill` plus `Reason` and the cloned `Glyph` per mark (~100); mark
geometry as `Primitive`s painted once and rasterised once (~110);
flights on their members as `Flying { anchor, depart, arrive, source,
destination, burns }` deleting `FlightId` and the flights store (~100;
to be reconciled with the movement redo, which reshapes flights first);
ready moments as `Entity.ready: Vec<Moment>` deleting the store and the
per-shot scan (~70); one `Vision { sweep, sight per team }` per step,
`Maneuver` folded into `Propagation` as one thrust function, `PerSeat<T>`
(~120); `Rock` carried from sim to pixel as one type, deleting
`Terrain`, `RockView`, `Caps`, `RingView.caps` (~90); one seating
authority (done); `Log` as the one stamped-log type, deleting the
server's `Ledger` and the record's fold (~90); the agent's knowledge as
one row per rock replacing seven collections and `Plan.promised` (~150,
behaviour moves); `Layout` yielding `Placed` with `span`, `Spacing`
replacing `Fit`, one `Dial` (~80); the small collapses (`Attractor` is
`Body`; `View::want`; a two-method `Transport`; `Room` folded into
`Socket`; one belt-drawing preamble; the fixture module; `Materials::
bottleneck` as `binding`; `Material::ALL`; ~340). Order: small
collapses, fixture module, ready, flights, frames, vision, rock, mark,
layout, primitives, log, agents; the flow (done) last of all. The
roster out of the hashed state only if the two ignored cost reports in
step/mod.rs and history/session.rs move.

## Operational

- The engine is a path dependency at `../../mirage-renderer`; its wgpu 29
  and egui 0.35 pin is its own concern. Its verification recipes are in
  `docs/verifying.md` there.
- The prototype from the web sessions sits at `/tmp/probe-game-previous-work`
  until the next reboot. The owner does not need it kept. Agents never read
  it.
- Linear is not set up and is ignored for now (owner, 2026-09-03).
- Sonnet briefs carry a hard rule (owner, 2026-09-03): every file edit
  through the Edit and Write tools, never a shell script, sed or heredoc;
  scripted edits produce lazy, poorly shaped code. Opus is told the same.
- Owner ruling 2026-09-03, every brief from now: comments and test
  names across the codebase are poor. A comment is replaced by a type or
  an invariant upheld at compile time wherever one can carry the fact;
  rustdoc is one plain sentence of contract. One test per guarantee a
  module makes to its callers, named as the sentence of that guarantee;
  no test per branch, field, helper or identity; a test no plausible
  wrong implementation fails is deleted. A sweep unit over the existing
  code is queued after the state system lands.
- Naming rule for every brief (owner, 2026-09-03): a function is named
  by what it returns or does in the game's words, never `of`, `get`,
  `handle`, `process`, `run`, `update`, `helper`, `util` or a suffix;
  a type is a noun the design documents use; a test is named as the
  guarantee sentence, never mechanically and never with `test`, `works`,
  `should`, `check`; comments that restate, narrate, or section are
  deleted; rustdoc never begins "This function" or "Returns".
- Placement rule for every brief (owner, 2026-09-03): a computation
  lives on the type that is its subject and there is one computation of
  each fact; before writing a derivation an agent searches for the type
  that owns the fact and extends it; a second derivation anywhere is a
  defect. The first instance: sim/src/state/view.rs re-deriving counts,
  frame fractions, exchanges and radar; being relocated in the controls
  unit. The codebase sweep (plan) audits for duplicates crate by crate.
- Match setup, bots included, is the start menu's job (owner,
  2026-09-03); the playable takes no command-line arguments.
- Store rule for every sim brief (overseer, 2026-09-05, for plan item 5):
  no rule holds an entity across a tick or indexes the entity store by
  anything but an id, so the columns land without touching a rule.
- Brief rules in force (owner, 2026-09-05): no comments of any kind; a
  confused agent is a naming defect; a net-line budget per unit with
  lines added against deleted reported; Edit and Write only.

## Engine friction

Reported to the owner as it is found; the game never works around a gap.

- Landed in the engine at 0688af3 on 2026-09-03: `Camera::pixel_of(point,
  size) -> Option<Vec2>`, the inverse of `ray_through`, physical pixels
  from the top left, `None` at or behind the eye; and
  `Camera::pixels_per_meter(point, size) -> Option<f32>`, the screen scale
  at the point's depth. game/src/camera.rs deletes its own projection and
  reads these; landed and verified green 2026-09-03, awaiting the
  owner's review of game/src/camera.rs.
- No line or ribbon primitive, and the engine will not get one (owner,
  2026-09-03). Ruled the same day: the HUD is painted in screen space
  through egui's painter over `pixel_of` projections: rings, runs, arcs,
  wheel, flight lines, radar contacts. The 3D scene draws rocks and ship
  glyphs as textured billboards from a sheet, kept as textures because
  custom icons are likely later. Display unit on a Sonnet agent, docs
  first; the owner reviews its code. Landed and verified green 2026-09-03
  with three screenshots; a fresh-eyes judge read ownership, the fight arc
  and the flight correctly. Ship glyphs were re-sized to fixed points and
  the scene now carries glyphs instead of row ids.
- Owner ruling 2026-09-03: `look` folds into `game` as a binary behind a
  `look` feature; `harness` becomes a binary of `sim` the same way. The
  owner's view: the skeleton `main.rs` drawing a cube was tolerated only
  because the display lived in a library nothing ran.
- Owner ruling 2026-09-03: the playable is an Opus unit. The owner cannot
  judge visuals until they can play; layout numbers, the manoeuvring rule,
  and the ring-inside-the-rock-at-close-zoom gap all wait for that.
- Register review of the three docs done 2026-09-03; the overseer applied
  the fixes (definitions before use, the manoeuvring rule split into one
  fact per sentence, the anchor defined the same way in DESIGN.md and
  ARCHITECTURE.md, ARCHITECTURE.md's stale Milestones section deleted,
  DESIGN.md's Build order brought to the truth).
- The sim step unit landed 2026-09-03, gate green end to end on the
  overseer's run: 118 sim tests, a measured 0.216 ms tick with 100
  entities on the 21-rock belt; the budget breaks near 1500 entities in
  the pair loop and the sight build. Key files: sim/src/step/mod.rs, the
  four phases under sim/src/step/, sim/src/state/view.rs,
  sim/src/session.rs, sim/src/belt.rs. The look fold landed the same day.
  DESIGN.md's Surplus bullet was reworded by the overseer to the code's
  reading: a shortfall, places in key order, pulls the nearest surplus.
- Held findings from the step unit, for the owner after playing: the
  impulsive-anchor lag is about 1000 m on a 2000 m send at belt scale and
  takes minutes to close (same fix candidates as above); a shipyard's
  repair at 15 HP/s outpaces a frigate's 12 DPS, so a lone attacker cannot
  kill at a builder's rock, a harness question; a send that cannot be
  planned is retried silently every tick and DISPLAY.md has no language
  for it, the one tolerated failure with no visible consequence; the belt
  spacing of 2000 m is set so a send crosses in about a minute, and a belt
  spread over the whole ring would put every rock outside the planner's
  ten-minute bound.
- The playable landed 2026-09-03, gate green end to end on the overseer's
  run: 123 sim tests, 53 game tests, wasm, the look feature build; a
  headless drive behind the `look` feature places a shipyard through the
  ring and the wheel and pans. Key files: game/src/main.rs, scene.rs,
  screen.rs, stencil.rs, fights.rs, send.rs, hud.rs, wheel.rs. Screenshots
  game/look/play_start.png and play_placed.png. Readings taken where the
  docs were silent: every rock draws its inner ring always and "an empty
  ring draws nothing" is about runs; the belt's centre is the mean of the
  rocks; a drag moves whole units off the end of the source run, cheapest
  rows first, one edit pair per row; a radar streak is velocity relative
  to the nearest rock over twenty seconds. Overseer ruling: `Mark.dim` is
  orthogonal to `Fill`, as built.
- Held for the owner's play: rocks are sub-pixel at any zoom that shows a
  region, so the belt reads entirely as HUD until within a few hundred
  meters; seat 0's palette colour is red, the fight arc's trail colour, so
  its own trail is invisible; the whole wave is uncommitted pending the
  owner's word.
- The offscreen `Session` has no way to drive the scroll wheel:
  `Motion::Wheel` is filled only by a winit event. Wheel zoom and the send
  drag's wheel count cannot be verified headlessly; the playable verifies
  zoom through a key binding instead. Found by the playable agent
  2026-09-03; for the owner's engine session.
- Screens outside the match move to egui's own layout and widgets
  under one game style (owner, 2026-09-03), driven headlessly through
  `Session::offer_ui`, which the multiplayer agent overlooked when it
  reported offscreen input never reaching the UI layer; the hand-painted
  control vocabulary, the text field and every per-screen `Places` go.
  Queued on the controls agent after the `Stage` unit.
- The painter's points-per-pixel is reachable only inside a `ctx.ui`
  closure, so every screen opens a throwaway `ctx.ui` to read it before
  drawing; a `FrameCtx::points_per_pixel()` would delete that. Found
  2026-09-03.
- Offscreen input never reaches the UI layer: `press` and `set_pointer`
  are read by the game and `offer_ui` by egui, never both, so a screen
  built from egui widgets could not be verified headlessly; the game's
  screens hit-test themselves through `ctx.pointer()` for that reason.
  Found 2026-09-03.
- Crate downloads are blocked in the agents' environment (crates.io's
  static host answers 403 while the index answers 200), so a new
  dependency must already be in the cargo cache; `serde` and `ciborium`
  were, `postcard` was not. Owner's environment, 2026-09-03.
- No runtime tick interval: `Config::with_tick_interval` is fixed at
  construction, so the game paces its own sim by treating every engine
  tick as an opportunity to step. Found 2026-09-03.
- No character input in the action vocabulary: the address field reads
  `egui::Event::Text`, Backspace and Enter off the UI layer's context, and
  `Session::offer_ui` is behind the `ui` feature, so typing is driven
  headlessly only behind it. Found 2026-09-03.
- The owner launched the engine work for the close request, the
  points-per-pixel accessor and the runtime tick interval on 2026-09-03.
- A game cannot quit: the engine exits only on the window's close
  request and there is no `ctx.quit()`; the pause menu has Resume only.
  Found 2026-09-03.
- `Key` has no punctuation keys, so zoom is bound to Q and E rather than
  minus and equals. Found 2026-09-03.
- A drag pan under perspective is inexact: `pan_by_pixels` scales at the
  focus's depth and lands a few percent off on a long drag. A camera-side
  "pan so this world point lands on that pixel" would be exact. Found
  2026-09-03.
- Verified present, so nobody re-checks: `TextureData::rgba8` for a glyph
  sheet generated at startup; `.frame(cell)` per draw for glyph variants and
  arc fill steps; `.faded(alpha)` per draw; `Session::pixels` for
  screenshots.

## Architecture notes, pending ARCHITECTURE.md

- Propagation: the universal-variable two-body solution, every body, every
  tick; a rock never thrusts, so its conic is fixed for the match.
- Transfers: Lambert's problem in universal variables gives the impulsive
  solution; a shooting correction makes it flyable under the row's
  acceleration limit. One solver serves rock-to-rock flights, band
  changes, and chases; re-solve only when the target changes, on a fixed
  cadence otherwise.
- Anchors: the rock's orbit shifted ahead by the band's amplitude; one per
  place, shared by all seats.

## Settled, do not re-raise

- No command budget as a rule of the game; caps against hostile input are an
  engineering matter inside the relay and `apply`.
- Ships are never drawn in a layout the sim does not have.
- No per-ship health bars.
- No points, no anchors, no objectives, no intrinsic compositions.
- Fixed-point numerics; per-row speed limits.
- A commander or any vital row; fixed seat quadrants on the ring; a staging
  place per seat; rock gravity or patched conics, since a slot is a free
  orbit with the rock's period.
- Attachment or reference frames of any kind: every body moves under one
  law, and nearness is a consequence of similar orbits. Re-affirmed
  2026-09-03: an anchor is an orbit, not a frame.
- Slots or any fixed formation lattice: formation is emergent from the
  manoeuvring rule (owner, 2026-09-03). The rule itself is replaceable.
- Client-side prediction of the sim's response to a want change: a hover
  shows the want change only; the sim's next tick is the feedback.
