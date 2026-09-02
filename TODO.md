# Probe Game — project state (overseer-facing)

Present and future tense only: operational constraints, pending work,
settled decisions. An in-flight milestone gets one entry, deleted when
committed; the commit message is the record. DESIGN.md and DISPLAY.md stay
target-state only. Agents see this file only through their briefs.

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
- Linear is not set up. Team and project names are the owner's to give.

## Engine friction

Reported to the owner as it is found; the game never works around a gap.

- No line or ribbon primitive. Flight lines now, orbit ellipses and tracers
  later, need a draw with constant screen width along a world path. Found
  2026-09-02.
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
- Slots: orbits of the rock's period at a band's amplitude, phase by index.

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
- Steering controllers of any kind: every motion is a solved transfer
  between orbits.
- Attachment or reference frames of any kind: every body moves under one
  law, and nearness is a consequence of similar orbits.
- Client-side prediction of the sim's response to a want change: a hover
  shows the want change only; the sim's next tick is the feedback.
