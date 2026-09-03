# Probe Game — architecture

How the code is shaped. This document describes the target; it never
records interim status. DESIGN.md is the authority on the rules the sim
computes and DISPLAY.md on what the player sees; this document is the
authority on types, modules, and the seams between crates. If
implementation reveals a problem here, stop and propose a change to this
document.

## Principles

1. **Invalid states are unrepresentable.** A structure has no velocity, a
   burn cannot exceed its row's limit, a composition with nothing in it
   does not exist, a command names a place and a row and nothing else. Where the
   type system cannot say it, one runtime check says it, with a comment
   naming the shape change that would delete the check.
2. **The step is a pure function.** `State::step(&self, ..) -> State`.
   Every phase reads the tick-start snapshot and returns an effect value;
   the next state is built from the snapshot and the effects. No phase
   mutates.
3. **One law of motion, two kernels.** Every body moves by exact two-body
   motion about the central mass. `Orbit::at` reads a fixed elliptic orbit
   at a tick, for rocks and anchors; `universal::propagate` advances a
   thrusting body one tick, for ships. One test ties the kernels together:
   `kepler_agrees_with_the_universal_propagator` asserts
   `Orbit::at(t + dt)` equals `propagate(orbit.at(t), dt)` over a spread of
   orbits and spans. Every send is one solved transfer between anchors;
   the only code that steers a ship is the manoeuvring rule.
4. **Determinism by construction.** `f64` only, transcendentals through
   `libm`, ordered containers, id-ordered iteration, no clocks, no
   randomness. The state hash is derived from every field, never listed.
5. **The sim knows nothing of the engine.** `sim` builds for both targets
   with no dependency on `game` or on Mirage. `game` adapts the sim's
   view to a scene and draws it; `harness` drives the sim headlessly, and
   `look` drives the display headlessly over hand-built scenes.

## Crates

```
sim/      probe-sim     the rules; no engine, both targets
                        bin: `harness`, behind a `harness` feature once
                        it lands — agents, matrices, determinism checks
game/     probe-game    lib: scene, glyphs, rings, wheel, camera, belt, hud
                        bin: the playable on Mirage; `look`, behind a
                        `look` feature, synthetic scenes through the
                        engine's offscreen Session to screenshots
```

`game` depends on `sim`; `sim` depends on `libm` and nothing else.
`check.sh` fails if `sim`'s dependency tree names the engine.

## Sim: numbers

- `Real(f64)`: the stored scalar. Hashes and compares by bit pattern, so
  every state type derives `Hash` and `PartialEq` and the hash is complete
  by construction. Arithmetic happens on `f64` at the site; `Real` is for
  fields, never for parameters.
- `Vec3 { x, y, z }` of `f64`: the sim's own vector with a closed set of
  operations: add, sub, scale, dot, cross, length, normalized. Converting
  to the engine's `f32` math happens in `game`, never here.
- `Materials { metals, volatiles, energy }` of `f64`: the triple, with
  component-wise arithmetic and `min`. A row's cost, a rock's caps, a
  row's capacity and a player's stock are all `Materials`; the field name
  carries the meaning, since the arithmetic is identical.
- `Stockpile { stock: Materials, capacity: Materials }`: the only place
  materials are held. Income clamps to capacity, spending never goes
  below zero, refunds clamp. No other type touches a stock directly.
- Transcendentals: `libm::sin`, `cos`, `sinh`, `cosh`, `atan2`, `exp`,
  `log`, `pow`. `f64::sqrt` is exact under IEEE 754 and allowed.
  `sim/clippy.toml` disallows the `std` transcendentals and `HashMap`,
  `HashSet`, `Instant`, `SystemTime`.
- `Tick(u64)`: the sim clock. `Moment(f64)`: a fractional tick, used for
  weapon ready times and shot ordering. `TICK` and `TICKS_PER_SECOND` in
  `lib.rs` are the only place seconds meet ticks.

## Sim: the state

```rust
pub struct State {
    tick: Tick,
    clock: Tick,                    // the match ends here
    gravity: Gravity,               // the central mass's μ
    seats: Vec<Seat>,               // team, alive, stockpile, reserve
    roster: Roster,                 // Vec<Row>, indexed by RowId
    rocks: Vec<Rock>,               // orbit, caps, radius; indexed by RockId
    entities: BTreeMap<EntityId, Entity>,   // dead ones removed
    next_entity: EntityId,          // the id the next spawn takes
    flights: BTreeMap<FlightId, Flight>,    // one per send in progress
    next_flight: FlightId,          // the id the next flight takes
    wants: BTreeMap<Post, Wants>,   // sparse: only posts with something
    frames: Vec<Frame>,
    ready: Vec<Ready>,              // damage weapons' next ready moments
}

pub struct Post { pub place: Place, pub seat: SeatId }      // one composition
pub struct Place { pub rock: RockId, pub band: Band }
pub enum Band { Inner, Outer }

pub struct Entity {
    id: EntityId, seat: SeatId, row: RowId, home: Place, hp: Real,
    motion: Motion,
}
pub enum Motion {
    Fixed,                          // a structure: its body is its rock's
    Free { body: Body, flight: Option<FlightId> },
}
pub struct Body { pub pos: Vec3, pub vel: Vec3 }            // inertial frame

pub struct Flight {
    from: Body, impulses: [Vec3; 2], depart: Tick, arrive: Tick,
    destination: Place, burns: BTreeMap<RowId, [Burn; 2]>,
}
pub struct Burn { from: Tick, to: Tick, accel: Vec3 }       // constant thrust
```

- Ids are newtypes over the index into their store: `RockId(u32)`,
  `EntityId(u32)`, `RowId(u16)`, `SeatId(u8)`, `FlightId(u32)`. `State` implements `Index`
  for each, so a rule reads `state[id]`. Dead entities are removed at the
  end of the step and ids are never reused within a match, so an id held
  across a tick is validated by lookup, never by trust.
- `Wants` is a `BTreeMap<RowId, u32>` with no zero entries. A post whose
  wants are empty, whose entities are gone and whose frames are closed is
  removed at the end of the step; that is the whole existence rule.
- `Seat { team, alive, stockpile, base_capacity, reserve }`. `Seat::new`
  takes the stock the seat starts with, which is also its base capacity;
  every tick the capacity is that base plus the capacity of its living
  entities, so nothing a seat starts with is lost.
- `Rock { orbit: Orbit, caps: Materials, radius: Real }`. `Orbit` is an
  elliptic conic as equinoctial elements with the tick its mean longitude
  is stated at; a rock never thrusts, so its body at any tick is
  `Orbit::at`.
- `Roster` owns the rows. The shipped eight are built by `Roster::shipped()`
  from `roster/shipped.rs`; `Roster::add(Row) -> RowId` serves harness
  variants. A row's kind is `Row::kind()`: `Structure` when acceleration is
  zero, else `Unit`. No `Copy` of a row lives anywhere but the roster.
- `Frame { post: Post, row: RowId, progress: Real }`: the work done so far,
  in cost units. An entity marked surplus carries its own scrapping the
  same way, in `Entity::scrap`, which is `None` until fulfilment marks it.
- `Ready { entity: EntityId, weapon: u8, at: Moment }`: one per damage
  weapon of a living entity; never earlier than the current tick.

## Sim: commands and the session

```rust
pub enum Command { Want { place: Place, row: RowId, count: u32 } }
pub struct Issued { pub seat: SeatId, pub command: Command }
pub enum Rejected { DeadSeat, NoSuchRock, NoSuchRow, StructureOutside, TooMany }

impl State {
    pub fn step(&self, issued: &[Issued]) -> (State, Outcome);
}
pub struct Outcome { rejected: Vec<(Issued, Rejected)>, shots: Shots }
pub struct Session { state: State, log: Vec<(Tick, Issued)>, outcome: Outcome }
```

`step` applies the commands to a copy of the snapshot, then runs the
phases over that copy as an immutable snapshot, and returns the next
state and the tick's `Outcome`. `Belt::fixed(gravity)` lays the shipped
rocks and
`State::start(clock, gravity, rocks, teams)` seats one player per team
with its reserve and starting stock. A rejected command comes back with
the command that was refused and changes nothing. `TooMany` is the one cap the sim states: a want above
`MAX_WANT` per post and row, a roster constant. Command volume is the
relay's concern. `Session::advance` returns the outcome's rejections and
keeps the outcome, so `Session::shots()` is what a view of the tick is
built with; no field of the state records a shot.
`Session::replay(initial, log, until)` rebuilds a state
from a log, and `harness` asserts the hash matches.

## Sim: the step

Each phase is a type that borrows the snapshot and returns its effect.
`State` has `step`, queries, and nothing else; a phase that wants to live
on `State` is a missing type.

```rust
let snap = &applied;                                   // commands applied
let thrusts = Maneuver::of(snap).run();                // Thrusts: one per free unit
let moved   = Propagation::of(snap, &thrusts).run();   // Moved: bodies and flights one tick on
let filled  = Fulfilment::of(snap).run();              // Assigned: reserve, surplus, frames opened
let income  = Extraction::of(snap).run();              // Income: per seat
let work    = Construction::of(snap).run();            // Progress: spend, completions
let shots   = Fire::of(snap).run();                    // Shots: ordered, with pending damage
State::next(snap, moved, filled, income, work, shots)  // deaths, reaping, elimination, tick+1
```

Phases do not see each other's effects; `next` applies them in that fixed
order and then removes the dead, closes empty posts, eliminates seats with
nothing left, and advances the tick. Within a phase, iteration is in id
order; anything sorted is sorted by a total key ending in an id.

## Sim: motion

- **Propagation.** `orbit::universal::propagate(body, gravity, dt) -> Body`
  is the two-body solution in universal variables with Stumpff functions,
  exact for every conic. Every ship advances by it once per tick, after the
  tick's thrust. Rocks are not stored as bodies; `State::rock_body(rock)`
  reads the rock's orbit at the current tick, which costs one Kepler solve
  however old the tick is. `State::body_of(&Entity) -> Body` is the one
  query for where an entity is: its rock's body when `Fixed`, its stored
  body when `Free`.
- **Anchors.** `Orbit::shifted(along: f64) -> Orbit` copies an orbit with
  its mean longitude moved by `along / a`, so `State::anchor(place)` is the
  rock's orbit shifted ahead by `place.band.amplitude()` and the anchor's
  body at a tick is `Orbit::at`. `Band::amplitude()` holds the two
  constants. An anchor is an orbit, and `Orbit::at` is its body at a
  tick; it is never an entity and never a body in state.
- **Flights.** One `Flight` is one send. `Flight::plan(state, from, to,
  rows) -> Option<Flight>` solves `orbit::lambert::solve` from the source
  anchor's body now to the destination anchor's body at a candidate arrival
  tick, prograde and single revolution, then spreads each impulse into one
  `Burn` per row at that row's `accel`, centred on the impulse's tick. It
  walks candidate arrival ticks upward from the current tick and takes the
  first at which no row's burns overlap and both lie inside the flight;
  past its bound it returns `None`. `Burn::spread` takes the impulse and the
  row's limit and holds the thrust over whole ticks, so a burn's magnitude
  cannot exceed the limit.
  `Flight::anchor(tick, gravity)` is `from` under the first impulse,
  propagated to `tick`; every ship of the send follows it. A unit is flying
  from the tick it joins a flight until it is within the arrival distance of
  its destination anchor. A flight with no
  member left is removed at the end of the step.
- **The attractor.** `Attractor::of(state, &Entity, &Sight, &Sweep)` reads
  the snapshot and
  applies DESIGN.md's order: the flight's anchor, the home anchor after the
  arrival tick, the half-range point off the nearest seen enemy inside the
  leash, else the home anchor. `state::sight::Sight` answers which entities
  a seat sees, over the `Sweep` (Sight, below), and is the only such query.
- **Manoeuvring.** `Maneuver::of(&State).run() -> Thrusts` is one thrust
  per free unit in id order, each the pull to its attractor plus one pair
  term per ship within the cutoff, clamped to the row's `maneuver`. Its
  constants are the stiffness, the damping, the spacing, the cutoff and the
  pair strength; the module stores nothing in state and reads only the
  snapshot, so it can be replaced whole.

## Sim: the rules as code

- **Fulfilment** walks every post in key order. For each row: the reserve
  first, then the nearest post holding a surplus of that row (distance
  between rock bodies now, ties by lower rock id, the highest-id units
  first), then the frames at the post brought to what remains. Bringing
  the frames to the count still missing is the whole opening and
  cancelling rule: a unit assigned or placed leaves one fewer missing, so
  one frame closes and refunds what it consumed. Units re-homed from one
  place to one place in one tick become one `Flight`; a plan that fails
  comes back as an `Unplanned` and those units stay home this tick.
  Surplus with nowhere to go is marked, and a structure is marked where it
  stands. `Assigned` carries the placements, the sends, the unplanned, the
  openings, the cancellations, and the whole set of marks.
- **Extraction** groups extract weapons by rock; per material, each takes
  its rate, the cap is split equally among them when the sum exceeds it,
  and unused shares redistribute until none is left or the cap is met.
  `Income` is one `Materials` per seat.
- **Construction** works one seat's rocks in turn over one copy of its
  stockpile. At each rock it assigns the builders' combined rate evenly
  across that seat's frames there, computes the per-material ratio of
  stock to demand, scales each frame's spend by the smallest ratio among
  the materials it uses, never above the work it has left, and records
  completions. What the frames cannot use goes to scrapping the marked
  surplus at that rock, which refunds a whole cost when it finishes, and
  what scrapping cannot use goes to repairing the seat's damaged entities
  there, which costs nothing. `Progress` names all three, so the split is
  readable. Completion spawns at the post: a structure `Fixed`, a unit
  `Free` at the place's anchor for the tick it first exists in, offset one
  spacing along the rock's radial direction per unit already there.
- **Fire** collects every ready damage weapon of an entity that is not
  flying, sorts by `(Moment, EntityId, weapon)`, and resolves each in
  order against the snapshot plus a `BTreeMap<EntityId, f64>` of damage
  assigned so far this tick, skipping targets whose assigned damage is
  lethal. Target choice is DESIGN.md's threat rule over entities at the
  same rock, not flying, inside the weapon's range, that the shooter's
  seat's team sees. Damage is the weapon's damage cut by its falloff over
  the range, less the target's plating, floored at zero. A weapon that
  fires is next ready one interval on, never before this tick; a weapon
  with nothing to shoot keeps the moment it has, so it fires the instant a
  target arrives. `Shots` is the list of hits and the new ready moments.
- **Sight** is a `Sweep`: entities sorted by their belt-plane `x` once per
  step, with range queries by window. Fire and fog use it; nothing scans
  every entity against every sensor.
- **Deaths, reaping, elimination** live in `State::next`: entities at or
  below zero HP are removed, their `Ready` entries with them; a send with
  no member left is removed; a seat with no entities and an empty reserve
  is dead, its posts and frames removed. A composition exists only while
  `wants` holds it, and `Wants` drops a row set to zero, so the existence
  rule needs no reaping of its own.

## Sim: what leaves the sim

- `View::of(&State, seat, &Shots) -> View`: the fogged state for a display
  or an agent, built once per tick from that tick's shots. Own
  compositions with wants, counts present and flying, and each
  frame's progress as a fraction of its cost; the reserve and stockpile;
  seen entities with id, row, seat, body, HP, whether they fly, and the
  place they belong to, which is `None` only for a flying entity of
  another team, whose destination sight does not give; radar
  blips with body and mass class, which are the entities inside a team
  sensor's radar range and outside its sight; the tick's `Exchange`s,
  which say per place and seat whether a shot the seat saw was fired or
  landed there; the gravity its terrain's orbits are read at; every rock
  with its orbit
  and caps; the standings. Nothing in a `View` refers to anything a seat
  cannot see. The roster is match-constant and travels with the initial
  state, so a client holds it from the session rather than from a view.
- `State::hash() -> u64`: FNV-1a over `Hash` of the whole state. The hasher
  widens every `usize` to `u64` and writes integers little-endian, so a
  `Vec` length prefix hashes the same on wasm32 and native.
- `State::standings() -> Standings`: per team, the rocks where it has a
  structure, the cost total of its living entities, and whether any of its
  seats is still in; `over()` says the clock has run out and `leaders()`
  applies DESIGN.md's tie-break. The clock is a field of the state, so the
  end is a query.
- `Session { state, log, outcome }`: `advance(issued)` logs, steps, and
  returns what the tick refused; `shots()` is the tick's shots beside the
  state; `replay(initial, log, until)` rebuilds a state from a log, and a
  test asserts the replayed hash equals the live one over a scripted
  match.

## Game: the display library

- `Scene`: everything one frame draws, built from a `View` by
  `Scene::from_view(view, roster, client)` or by hand in `look`. Rocks with
  positions and radii; entities with position, glyph and seat; per ring,
  per seat, the run as a list of `Mark { glyph, fill, dim }` where `fill`
  is `Solid`, `Hollow`, `Filling(f32)` or `Dashed` and `dim` is what the
  pointer says is leaving or arriving; per ring, per seat, an optional
  `Arc { fraction, trailing }`; flights as lines to a rock; radar blips as
  a position, a drift past the nearest rock and a mass class; the
  selection and the `Hover`, which is a wheel band or a `Sending`. Every
  rock draws its inner ring; an outer ring draws only where that band
  holds something or its rock is selected.
- `Client { selection, hover, fights }`: what the client, not the sim,
  decides about a frame, the third argument of `Scene::from_view`.
- `fights::Fights`: the fight memory, kept by the client because no field
  of the state records a shot. `observe(&View)` once per tick starts an arc
  where the view reports shots exchanged, drains it as the seat's HP at the
  place falls, trails the last second and a half of damage, and forgets an
  arc ten seconds after the last shot; `arcs()` is what the scene draws.
- `send::Sending { from, to, count }`: the drag gesture. `rows` is what it
  moves, off the end of the source run so the cheapest rows go first, and
  `commands` is the two count edits per row it moves. One type, so the
  glyphs the scene dims and the edits the release issues cannot disagree.
- `glyph::Glyph { frame, marks, size }`: `Glyph::of(&Row)` by DISPLAY.md's
  three rules, a pure function with a test per rule. `glyph::HALF` is a
  glyph's nominal half-width, which both layers size by.
- `stencil::Stencil`: one glyph painted on the HUD — the frame in its fill
  state, its marks, and the dim a preview draws it at. The HUD and the
  wheel paint through it, so a glyph is drawn one way.
- `glyph_quad::GlyphQuad`: a mesh value per `(Glyph, SeatId)`, its own
  texture rasterized in the seat's colour, for the belt. One billboarded
  quad per ship, one draw each; hollow, filling and dashed glyphs exist
  only on rings, so the catalog holds solid cells alone.
- `ring::Layout::of(runs, geometry)`: angles and overlap offsets for every
  mark on a ring, pure, tested for the DISPLAY.md guarantees: distinct
  glyphs per row, overlap at forty, no overrun. `span(run)` is the share a
  run owns, which is what its fight arc is drawn over, so the arcs cannot
  drift from the runs.
- `wheel::Wheel`: the roster wheel's annulus, slots and bands as
  screen-space sectors, its own hit test, and its painting.
  `WheelBand::edit` is the `Command` a click issues.
- `camera::BeltCamera`: the focus point moving at the local orbital
  velocity, pan by meters or by pointer pixels, and zoom within a range
  stated against the belt's own scale.
- `screen::Screen`: one frame's projection, the engine camera built once,
  with the window and the painter's own measure. Everything that projects
  a world point goes through it. Picking is screen-space distance to a
  projected rock centre against the ring radii, since rings have a fixed
  screen radius.
- Two draw modules, one per DISPLAY.md layer. `belt`: one function from a
  `Scene` and a `Screen` to the engine's draws — rocks, one light, and
  ships, in 3D. `hud`: one function from a `Scene`, a `Screen`, the open
  wheel and an `egui::Painter` to painted shapes — rings, runs, arcs, the
  wheel, flight lines, radar contacts, the selection and the hover
  preview; it calls `ring::Layout` and paints glyphs through `Stencil`.
- The binary is the playable: a `Game` whose `tick` advances a `Session`
  with the commands the input issued, reads the tick's `View` and feeds
  the `Fights`, and whose `frame` builds a `Scene` and draws it — `belt`
  then `hud`, so the HUD is never occluded — over one full-window
  transparent egui layer that claims no widgets. Input is keyboard and
  mouse through the engine's action vocabularies: a ring click selects and
  focuses, a wheel band click edits one want and repeats while held, a
  left drag from ring to ring is the send, the right or middle button and
  the pan keys drag the belt, and the zoom axis zooms or, during a send,
  sets how many go. The gamepad bindings DISPLAY.md states are a later
  unit. The pause menu is the one panel: Escape toggles it, and at the
  clock it shows the standings as text. Its own headless drive, behind the
  `look` feature, plays it through the engine's offscreen `Session` and
  writes `game/look/play_*.png`.

## Look

`look` is a binary of `game`, behind the `look` feature, over the
engine's `offscreen` feature and its default `ui` feature so the HUD
lands in the pixels. It holds the three fixed scenes from DISPLAY.md as
code, renders each through a `Session`, reads pixels back, and writes
PNGs under `game/look/`, which is `.gitignore`d: screenshots are the
judgement's input, never committed. `cargo run -p probe-game --features
look --bin look` runs it; `check.sh` only builds it, with `cargo build -p
probe-game --features look --all-targets`, so the default binary and the
wasm build never pull `image` or `offscreen`. It is the only way a
display change is verified.

## Harness

`harness` is a native binary over `sim`: scripted agents over `View` and
`Command`, matrices of composition against composition, and the
determinism checks: replay reproduces the hash, and the same match at
twice the tick rate. Its shape lands with the first sim system.

## Module layout

```
sim/src/
  lib.rs            TICKS_PER_SECOND, TICK; the public surface
  belt.rs           Belt::fixed, State::start
  real.rs           Real
  vec3.rs           Vec3
  materials.rs      Materials, Stockpile
  time.rs           Tick, Moment
  ids.rs            RockId, EntityId, RowId, SeatId, TeamId
  place.rs          Band, Place, Post
  roster/           mod.rs Roster; row.rs Row, Weapon, Kind; shipped.rs the eight
  orbit/            body.rs Body, Gravity; elements.rs Orbit;
                    stumpff.rs; universal.rs; lambert.rs
  state/            mod.rs State, Index impls, queries; seat.rs; rock.rs;
                    entity.rs Entity, Motion; flight.rs Flight, Burn;
                    attractor.rs; sight.rs; wants.rs Wants; frame.rs;
                    ready.rs; command.rs Command, Issued, Rejected, apply;
                    sweep.rs; view.rs View; hash.rs;
                    standings.rs Standings
  step/             mod.rs step, next; maneuver.rs; propagation.rs;
                    fulfilment.rs; extraction.rs; construction.rs; fire.rs
  session.rs        Session, replay
  bin/harness.rs    behind the `harness` feature, once it lands
game/src/
  lib.rs            the display library's surface
  scene.rs  glyph.rs  glyph_quad.rs  ring.rs  wheel.rs  camera.rs
  screen.rs         the frame's projection
  stencil.rs        one glyph painted on the HUD
  fights.rs         the fight memory over successive views
  send.rs           the drag gesture and the edits it issues
  belt.rs           Scene and Screen to the engine's 3D draws
  hud.rs            Scene, Screen, the open wheel and an egui::Painter to
                    painted shapes
  main.rs           the playable, and its headless drive
  bin/look.rs       behind the `look` feature
```

One concern per file; a file that needs a section comment is two files.

## Conventions

- Right-handed, +Y up, the belt plane is XZ, the central mass at the
  origin, the engine's frame exactly, so `game` converts scale and type
  and never axes.
- Lengths in meters, time in seconds inside `orbit`, ticks everywhere
  else; angles in radians; a phase is a fraction of a turn in `0..1`.
- Every `f64` parameter or field states its unit in its rustdoc.
- Iteration order is id order. A `BTreeMap` where a map is needed, a
  sorted `Vec` where a set is needed.
- `pub(crate)` by default; `pub` is the surface `lib.rs` re-exports.

## Dependencies

| crate | scope | why |
|---|---|---|
| `libm` | `sim` | transcendentals identical on every target; `std`'s are the platform's |
| `mirage-engine` | `game` | the engine, by path; `look`'s bin additionally needs its `offscreen` feature |
| `image` | `game`, behind the `look` feature | writing PNGs; already in the engine's tree |

Adding one requires a row here.

## Invariants (checked on every change)

- `sim` builds for `wasm32-unknown-unknown` and its tree does not name
  the engine.
- No `std` transcendental, `HashMap`, `HashSet`, `Instant` or `SystemTime`
  in `sim` (`sim/clippy.toml`).
- `State: Hash + PartialEq`, derived, so a new field is hashed without a
  hand edit.
- Replay of a log reproduces the live hash.
- No phase type holds `&mut State`.
- No `unwrap` or `expect` on data that came from a command.
