# Probe Game — project state (overseer-facing)

Present and future tense only: operational constraints, pending work,
settled decisions. An in-flight milestone gets one entry, deleted when
committed; the commit message is the record. DESIGN.md and DISPLAY.md stay
target-state only. Agents see this file only through their briefs.

## In flight

- Committed 09ef8f4 on 2026-09-03: the whole first wave, green on both
  targets. Key files for the owner's review of the Sonnet units:
  game/src/camera.rs, glyph_quad.rs, belt.rs, hud.rs, wheel.rs,
  game/src/bin/look.rs.
- In flight 2026-09-03 on the playable's Opus agent: five HUD defects
  against DISPLAY.md as written (hover wedge, preview stacked on the placed
  glyph, wheel overlapping the outer ring, uneven wheel glyphs, an opening
  view that frames the region). Rock size, ring radii, glyph sizes and
  colours untouched by the owner's word.
- Held for the owner's play, in one list: heavy-row lag after a send and
  its braking-pull candidate; the planner's earliest-fit arrival; crowd
  packing past half the spacing; unarmed rows never chase; tidal drift at
  short rock periods; repair at 15 HP/s beats a frigate's 12 DPS; a send
  that cannot be planned is retried silently; rocks are sub-pixel at region
  zoom; seat 0's red is the arc trail colour; the ring at a fixed screen
  radius sits inside a rock at close zoom; readings the playable took
  where the docs were silent (every rock draws its inner ring always; the
  belt's centre is the mean of the rocks; a drag moves whole units off the
  source run cheapest rows first, one edit pair per row; a radar streak is
  velocity relative to the nearest rock over twenty seconds; `Mark.dim` is
  orthogonal to `Fill`).

## Plan, in order

1. The owner plays; their judgement of feel and legibility unlocks the held
   list above, each item a ruling then a unit.
2. The scripted agent and `harness` as a binary of `sim`: replay-hash and
   doubled-tick-rate checks, composition matrices; the repair-versus-fire
   question goes there first.
3. Display language for a send that cannot be planned.
4. Map generation from seed with regional caps; fog in the display;
   gamepad; the twelve-slot wheel level.
5. The relay and multiplayer lockstep.

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
