# Probe Game — project state (overseer-facing)

Present and future tense only: operational constraints, pending work,
settled decisions. An in-flight milestone gets one entry, deleted when
committed; the commit message is the record. DESIGN.md and DISPLAY.md stay
target-state only. Agents see this file only through their briefs.

## In flight

- A parallel wave over milestones 1 and 2 was stopped at 01:00 on
  2026-09-03 for usage reasons; the tree is uncommitted and RED. Verified
  by the overseer on 2026-09-03 on a scratch copy with `Slot` and
  `Transfer` stubbed: 66 sim tests and 20 game tests pass, clippy and fmt
  clean. Landed and read: roster/, orbit/{body,stumpff,universal},
  state/{mod,seat,rock,entity,wants,frame,ready,command,sweep,hash},
  step/{extraction,construction} as pure kernels without their phase
  types, game/{scene,glyph,ring,camera}. Still one-line stubs:
  orbit/{elements,lambert,slot,transfer}, state/{view,standings},
  step/{fire,fulfilment,propagation}, game/{sheet,draw,wheel}; look/ and
  harness/ were never created; main.rs is the skeleton.
- Two reds: state/entity.rs imports `orbit::slot::Slot` and
  `orbit::transfer::Transfer`, which are stubs; and camera.rs's tests use
  an untyped float literal with `powi`, hidden behind the first.
- Owner ruling 2026-09-03: the orbit seam (no `Orbit` type, `Rock` holding
  an epoch body, `Motion::Free` storing a body while holding, `Slot`
  undefined) is refactored from the ground up as a red-state refactor,
  with an `Orbit` type. The shape is proposed to the owner before the
  agent is dispatched.
- Owner ruling 2026-09-03: the whole wave commits as one commit, docs and
  code together, on the owner's word once green.
- Owner ruling 2026-09-03: Opus agents take the hard sim units (orbit,
  step, session); Sonnet agents take the rest, and the owner reviews every
  Sonnet unit personally, so a Sonnet report leads with its key file
  paths.
- Owner ruling 2026-09-03: display work waits for `Camera::project` in
  the engine (below); sim work proceeds now. The owner launched that
  engine work in the engine's own session on 2026-09-03.
- Stable subset landed and verified green by the overseer on 2026-09-03:
  `Orbit` as equinoctial elements with the Kepler-versus-universal
  cross-check, `Rock` over `Orbit`, entities as an id-keyed map, `Body`
  and `Gravity` fixes, the empty modules undeclared. `Motion` has only
  `Fixed` until the movement unit lands.
- Owner ruling 2026-09-03, movement: ships are free bodies under the one
  law at all times. Two thrusts per row: `accel` for solved transfers,
  `maneuver` for flocking. Every place has an anchor, the rock's orbit
  shifted ahead by the band's amplitude (inner 4 m, outer 30 m,
  hypotheses), shared by all seats. A send is one Lambert transfer flown
  by a virtual anchor with per-row burns; ships flock around it. The
  manoeuvring rule is a damped spring to the attractor plus a bounded
  pair potential split by mass, in `step/maneuver.rs`, the one module
  the owner expects to replace after visual results, so it stores no
  state. Right of way is emergent from the mass split and the clamp.
  Chase is the same rule with a different attractor. Slots are gone.
  The movement unit landed 2026-09-03, verified green on `sim` by the
  overseer (103 tests, clippy native and wasm32). Key files:
  sim/src/step/maneuver.rs, sim/src/state/flight.rs,
  sim/src/orbit/lambert.rs, sim/src/state/attractor.rs, DESIGN.md
  Movement and combat.
- Owner ruling 2026-09-03: no further tweak to the manoeuvring rule or the
  flight planner until the owner has judged the motion visually; too many
  fixed edge cases would be hard to judge. Observed and held, not fixed:
  (1) a heavy row lags its flight's anchor by hundreds of meters on a
  kilometre send and oscillates for minutes under the proportional pull; a
  braking pull, aiming at the closing speed the manoeuvring limit can
  arrest, is the candidate fix. (2) The planner's earliest-fitting arrival
  maximises that lag; requiring the flight to be a few burn spans long is
  the candidate fix. (3) Twenty ships settle at about half the spacing and
  larger crowds pack tighter, since attraction sums over neighbours;
  dividing the attractive term by the neighbour count is the candidate
  fix. (4) Unarmed rows hold at the anchor rather than chase. (5) Tidal
  acceleration at a five-minute rock period is half a frigate's
  manoeuvring limit; negligible at nine hours.

## Plan, in order

1. The roster in `sim`, seeded from the table below, and in `game` the two
   pure functions of the display language with tests: row to glyph, and a
   ring's forces per seat to glyph placements.
2. Rendering over a `Scene` interface, and a native-only `look` tool that
   drives the offscreen Session over the three fixed scenes in DISPLAY.md
   and writes screenshots.
3. A fresh-eyes judgement of the screenshots by an agent that never saw the
   code, then the owner's review. Changes go into DISPLAY.md before any sim
   work.
4. The headless sim per DESIGN.md's build order, `harness` crate with it.
5. The Linear issue list, once step 3 has made the backlog concrete.

## Roster seed

The prototype's tuned numbers, a hypothesis for the harness. Moves into
`sim` in plan step 1 and is deleted here then. Cost is metals/volatiles/
energy; radar is twice sight unless stated; rate is per second.

| row | cost | HP | accel | sight | weapons | other |
|---|---|---|---|---|---|---|
| constructor | 30/10/10 | 50 | 4 | 6 | build 3 | |
| extractor | 40/0/10 | 120 | 0 | 3 | extract 2 | rate is the overseer's guess |
| storage | 30/0/10 | 150 | 0 | 2 | | capacity 500 |
| shipyard | 100/0/40 | 300 | 0 | 6 | build 15 | capacity 500, the overseer's guess |
| scout | 5/10/0 | 15 | 10 | 20 | | radar 50 |
| raider | 20/20/5 | 40 | 8 | 12 | damage 3 range 3 rate 4 falloff 0.5 | |
| frigate | 80/10/30 | 150 | 2 | 8 | damage 6 range 6 rate 2 | plating 1 |
| lancer | 40/5/40 | 60 | 3 | 5 | damage 20 range 14 rate 1 | radar 10 |

Start: a reserve of one shipyard and one constructor, stockpile
300/100/100. Band amplitudes and the slot phase step are unset.
Rock caps: 0 to 10 per second per material, drawn by region.

## Operational

- The engine is a path dependency at `../../mirage-renderer`; its wgpu 29
  and egui 0.35 pin is its own concern. Its verification recipes are in
  `docs/verifying.md` there.
- The prototype from the web sessions sits at `/tmp/probe-game-previous-work`
  until the next reboot. The owner does not need it kept. Agents never read
  it.
- Linear is not set up and is ignored for now (owner, 2026-09-03).

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
