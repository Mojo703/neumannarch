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
- Owner ruling 2026-09-03, from play: a post builds one frame of a row at
  a time. A shortfall of N opens one frame; the next opens when it
  completes. Rows still build in parallel with each other. DESIGN.md's
  Build is flow and Shortfall bullets, fulfilment, and the ring's fill
  states (one filling glyph, the rest hollow) follow. Sim unit, after the
  harness lands.
- Owner ruling 2026-09-03, from play: a selected rock owns the focus. The
  camera's focus is the rock's body each tick while a rock is selected, so
  the camera moves as if connected to it; a pan releases it to the free
  focus at the local orbital velocity. DISPLAY.md's Camera section and
  main.rs follow. Display unit, after the harness lands.
- Landed 2026-09-03, uncommitted: rendering relative to the focus through
  `Screen::local`, killing the f32 jitter at belt distance; tests state
  the guarantee and were shown failing on the old conversion.
- Deferred from the skirmish unit 2026-09-03: a match ending by
  elimination before the clock is unreachable from a fogged view, since
  standings are revealed only at the clock; Results is reached at the
  clock only, and an eliminated seat's end needs a DESIGN.md ruling. The
  fight arc refills when a seat reinforces, which DISPLAY.md's "full at
  the fight's start and drains" does not say; held for play.
- From the icon judgements 2026-09-03: at close zoom the ring sits far
  from the ships it counts, and a judge read the belt's ship billboards
  near the rock as unexplained units outside the ring. Held with the
  ring-inside-the-rock gap for the owner's ruling on rings at close zoom.
- Owner's points relayed by the icon agent 2026-09-03, pending rulings:
  a seat's colour and number against its team's representation in the
  lobby and on rings (DISPLAY.md numbers seats and teams independently
  of colour); whether a control for an unbuilt feature (Surrender) should
  exist at all rather than stand disabled with a reason, which would
  amend the Controls section; camera locking to a selected rock, already
  ruled and queued as the focus unit; Regenerate renamed Random Seed by
  the owner's own edit across the screens and DISPLAY.md.
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

Settled 2026-09-03 with the owner: the foundation is rollback, not
lockstep. Every machine runs the whole match; a command applies at the
tick it was issued; a late command rewinds and replays; hashes compare at
settled ticks; a bot is a seat run by the machine that added it; the
lobby is one document the host shapes and guests edit their own seat in;
the match server holds rooms, mirrors the lobby, forwards, and keeps
records. ARCHITECTURE.md's Crates, Sim: history, Agents, Protocol, Game:
net and screens, and Server sections state it; DESIGN.md's Session
bullets and DISPLAY.md's Screens section state the rules and the look.

1. Reorganise the workspace: done 2026-09-03, gate green on the
   overseer's run, uncommitted by the owner's word. `agents/` split from
   `sim`, `sim/src/history/`, `game/src/display/`. The agents' play
   tests make the gate take about a minute.
2. Register review of the three rewritten docs: done and applied
   2026-09-03; the owner's read is pending.
3. The state system: landed 2026-09-03, gate green and the harness
   `rollback` check passing on the overseer's run. A full-window rewind
   (two seconds, 240 ticks, 100 entities) costs 73.5 ms in release;
   `Retention`'s `every` is the dial. Key files: sim/src/history/
   {session,snapshots,log,record}.rs, sim/src/setup.rs,
   sim/src/state/command.rs. Was: `Issued.seq`, `Stamped`, `Setup` and
   `State::start(setup)`, `History` with `Retention::Window`, `Session`
   as ARCHITECTURE.md states it, `Record`; the harness gains `rollback`.
   Red-state refactor over today's `Session`; the owner asked for great
   care here.
4. and 5. Landed 2026-09-03: `protocol` (serde plus ciborium; `Lobby`,
   `LobbyEdit`, `Message`, `Record` with `Record::of(&Session)`,
   `Bot`), `game` controllers and `Local`, `Flow` and the six screens in
   the styled register, the lobby grouped by team over the full-screen
   belt with ring strokes tinted by caps, Results wording, the headless
   drive in `check.sh`. Drawn disabled with rustdoc naming their unit:
   Host, Join, Settings, Quit (engine gap), Surrender (no DESIGN rule).
   Key files for the owner: protocol/src/, game/src/net/,
   game/src/screens/, game/src/display/tint.rs.
6. Landed and committed 2026-09-03, gate green on the overseer's run
   with the icon unit (placed marks, three new marks, glyph half-width
   11 pt, size steps 0.85/1.0/1.2, rolling faded flight lines): `server` (rooms
   pure and network-free, `stream.rs` the only socket), `Socket` over
   `tokio-tungstenite` on native and `web-sys` in the browser, `Machine`
   as the engine-free lockstep loop, pacing (hold on the window, slow one
   step in four past a 24-tick lead), the address field, Host and Join,
   held states, a two-machine test over real sockets in one process.
   Two gaps for a small follow-up: Rematch after a room match needs a
   room-side rule and a message; a refused lobby edit is silent in the
   lobby (the room's broadcast is the truth). Was: rooms, forwarding, hash reports, desync,
   records; host embeds it; join by address; pacing and the waiting
   screen. Multiplayer functional.
7. Controls: landed 2026-09-03, gate green on the overseer's run (259
   tests; `check.sh`'s fmt step now scoped to this workspace's crates,
   since `cargo fmt --all` reached into the engine's tree). Key files
   for the owner: game/src/screens/control.rs, field.rs, lobby.rs,
   protocol/src/lobby.rs (Kick, AlreadySeated), server/src/rooms.rs,
   sim/src/state/radar.rs, step/fire.rs (Shots::exchanges). Overseer
   ruling: `Message::Removed` for a kick, distinct from Leave, stands.
   The engine work landed the same day: wire Quit, the points-per-pixel
   accessor and the tick interval as a small follow-up. Was:
   three kinds of control, enabled means it will work with the reason on
   hover, no notices or errors except inside the join field; the lobby
   row as holder choice with Kick, team choice, seed field with
   Random Seed inside, clock choice; stalled-for-material belt mark and
   hover reasons on run glyphs; wheel bands disabled at the cap and at
   zero; Quit disabled with its reason until the engine can close;
   Rematch in a room. Next unit, Opus, critique stop first.
8. The two play rulings: one frame per row at a time; a selected rock
   owns the focus.
9. A proper asteroid belt and the star: map generation from seed with
   regional caps; the star as a distant light and disc.
10. From the harness: seat 0's edge isolated and removed; territory that
   varies with composition; combat before the last third of a match.
11. Icons by placement (owner's concept sheet, art/concepts/, kept out of
    git, 2026-09-03): marks placed by role plus an arc for radar above
    default, a belt for plating, a ring for capacity; DISPLAY.md's three
    glyph rules become six placement rules; `Glyph::of` and both painters
    follow; judging scenes re-shot and re-judged. Whenever a display slot
    is free; not before the skirmish lands, since it owns the glyph files.
    Later, with the owner's models: hulls as meshes with a level-of-detail
    rule (mesh above a screen size, glyph below), the stencil icon, and a
    DESIGN.md line that a faction skews the hull's dialect and never the
    glyph.
12. The structural programme (owner, 2026-09-03), in the Opus audit's
    order: (1) small collapses: Attractor is Body, View::want and
    rock_body, a two-method Transport, Room folded into Socket, one
    frame preamble for belt-drawing screens, one sim test fixture module;
    (2) a weapon's ready moment on its entity; (3) a flight on its
    members, FlightId deleted; (4) a frame on its post; (5) one Vision
    per step, manoeuvring folded into propagation as one function,
    PerSeat; (6) one rock type from sim to pixel; (7) one mark state
    replacing Fill and Reason; (8) Layout yields placed marks and one
    Dial owns polar geometry; (9) mark geometry as primitives, painted
    once and rasterised once, then re-judged; (10) one Log type; (11) one
    seating authority on Lobby, freeze returning the seating; (12) the
    agent's knowledge as one row per rock; (13) the flow as one `Stage`
    enum with the room inside it as `Authority { Local, Guest, Host }`,
    screens returning the next Stage and three asks (Host, Join, Leave);
    pulled forward by the owner on 2026-09-03 with (11) as its
    prerequisite, on the controls agent after its lobby table fix; it
    also lists other loose-field lifecycles in game and server for the
    owner's ruling; (14) the roster out of the hashed
    state only if the two cost reports move on a measured prototype.
    DESIGN ruling: Unplanned is deleted; a send that cannot be planned
    is retried silently and the bot keeps its patience timer. Comments:
    all in-body comments go except a required verdict line; rustdoc one
    sentence on public items only. ARCHITECTURE.md rewritten per unit.
    Opus for 2, 3, 4, 5, 12; Sonnet for the rest. Also lands the Sonnet
    audits' 530 lines inside these units.
    The size sweep as first framed (owner, 2026-09-03). Five audits found about 530
    deletable lines: duplicated facts (~160, four already relocated out of
    view.rs by the controls unit; the seat-index rule derived three ways
    across protocol, game and server goes onto `Lobby`), tests (~185),
    comments (~120), agents and display (~56: two `is_structure`s, two
    material palettes, three clock-angle conversions, the look binary's
    `Mark` build break), surface (~10). Owner's rulings: land all of it;
    remove all or almost all comments, in-body comments entirely except a
    required verdict line, rustdoc one sentence on public items only; and
    a deeper structural audit on Opus for shapes that delete logic and
    data storage (parallel stores in State, the View to Scene to Layout
    chain, the lobby's five shapes, the match lifecycle types, the agents'
    five shapes), in flight 2026-09-03. Every brief carries a line ceiling
    from now on, and check.sh gains a duplication check.
13. The held list from play, each a ruling then a unit; fog in the
    display; gamepad; the twelve-slot wheel; a display for an unplannable
    send.

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
