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
sim/       probe-sim       the state system: state, rules, orbit, roster,
                           belt, and history (snapshots, the stamped log,
                           rewind, settling, replay); no engine, both
                           targets
protocol/  probe-protocol  every value two machines exchange, defined in
                           Protocol below; serialisable; no io; both
                           targets
agents/    probe-agents    the agent frontend: the Agent trait, the
                           scripted opponent, the personality a lobby's
                           bot plays by; bin: `harness`, native only
game/      probe-game      the player frontend: display/, net/, screens/;
                           bin: the playable on Mirage; `look`, behind a
                           `look` feature, synthetic scenes through the
                           engine's offscreen Session to screenshots
server/    probe-server    the match server: rooms, lobby authority,
                           forwarding, records; a library `game` embeds
                           on native to host, and a binary; native only
```

Dependencies point one way: `protocol` depends on `sim`; `agents` depends
on `sim` and `protocol`, since a lobby names the bot an agent plays and a
record is a protocol value; `game` depends on `sim`, `protocol` and
`agents`, and on `server` under the `host` feature, which is on by
default and carries the server only on a native target, so the playable
hosts wherever it can and the browser build carries none of it; `server`
depends on `protocol` and `sim`. `sim` depends on `libm` and on serde's derive,
which adds no arithmetic. The browser build carries `sim`, `protocol`,
`agents` and `game`. `check.sh` fails if `sim`'s dependency tree names
the engine. `server` is created by the unit that needs it, never as an
empty crate.

The two frontends have one role each and share everything below them: a
human and a scripted agent both read a `View` and speak a `Command`, and
both drive the same `Session`. Nothing in `sim` or `protocol` knows which
frontend is speaking.

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
pub struct Issued { pub seat: SeatId, pub seq: u32, pub command: Command }
pub struct Stamped { pub tick: Tick, pub issued: Issued }
pub struct Batch(Vec<Issued>);                      // one tick's, ordered
pub struct Sequence { seat: SeatId, next: u32 }
pub struct Setup { teams: Vec<TeamId>, seed: u64, clock: Tick }
pub enum BadSetup { NoSeats, TooManySeats }
pub enum Rejected { NoSuchSeat, DeadSeat, NoSuchRock, NoSuchRow,
                    StructureOutside, TooMany }
pub enum Refused { Duplicate, TooMany, Late, Ahead }

impl State {
    pub fn step(&self, issued: &Batch) -> (State, Outcome);
}
pub struct Outcome { rejected: Vec<(Issued, Rejected)>, shots: Shots }
```

`step` applies the batch to a copy of the snapshot, then runs the phases
over that copy as an immutable snapshot, and returns the next state and
the tick's `Outcome`. A `Batch` holds one tick's commands in `(seat, seq)`
order with no key twice, so the result cannot depend on the order they
arrived in: `Batch::insert` refuses a `(seat, seq)` it already holds as
`Duplicate` and a seat past `MAX_COMMANDS_PER_TICK` as `TooMany`, which
is the cap on what a peer can put in one tick. `seq` counts a seat's
commands from zero for the match and a `Sequence` per local seat stamps
it, so no frontend counts for itself.

`Setup::new` refuses a match with no seats or more than `MAX_SEATS` by
name, so a setup off the wire is checked once. `State::start(&setup)`
seats the players it names, each with their reserve and starting stock,
lays `Belt::fixed`, and carries the seed, which is hashed and unused
until map generation lands. A rejected command comes back with the
command that was refused and changes nothing. `Rejected::TooMany` is a
want above `MAX_WANT` per post and row, a roster constant.

## Sim: history

Every frontend drives a `Session`, and rollback is a property of it, not
of the network: a command applies at the tick it was stamped at wherever
it is applied, and a session that learns of one late restores that tick
and re-steps. `Snapshots` and `Log` are the two stores under it.

```rust
pub struct Session {
    setup: Setup,
    initial: State,
    live: State,                           // at the tick the frontend shows
    snapshots: Snapshots,                  // the states Retention keeps
    log: Log,                              // stamped commands by tick
    outcomes: BTreeMap<Tick, Outcome>,     // of the ticks inside the window
    acknowledged: Vec<Tick>,               // per seat: no command before this tick is unknown
    local: Vec<SeatId>,                    // the seats this machine owns
}
pub enum Retention { Window { ticks: u32, every: NonZeroU32 } }
pub enum Rewound { Nothing, From(Tick) }
pub struct Unseated { pub seat: SeatId }

impl Session {
    pub fn new(setup: Setup, retention: Retention, local: &[SeatId])
        -> Result<Session, Unseated>;
    pub fn advance(&mut self) -> &Outcome;                 // one tick on
    pub fn insert(&mut self, stamped: Stamped) -> Result<Rewound, Refused>;
    pub fn acknowledge(&mut self, seat: SeatId, up_to: Tick);
    pub fn acknowledged(&self, seat: SeatId) -> Option<Tick>;
    pub fn settled(&self) -> Tick;                         // min over seats
    pub fn hash_at(&self, tick: Tick) -> Option<u64>;      // kept, or re-stepped
    pub fn outcome_at(&self, tick: Tick) -> Option<&Outcome>;
    pub fn outcome(&self) -> Option<&Outcome>;             // of the tick shown
    pub fn state(&self) -> &State;
    pub fn setup(&self) -> &Setup;
    pub fn commands(&self) -> Vec<Stamped>;                // before `settled`
}
```

- **Insert.** A stamped command at or after the tick the session shows is
  logged for the step that reaches it, and nothing is re-stepped:
  `Rewound::Nothing`. One before it restores the newest kept state at or
  before that tick, logs the command, and re-steps to the tick shown,
  replacing the outcomes on the way; the frontend is told
  `Rewound::From` that tick, whose outcome and every later one may now
  differ, and so may every state after it. A tick further back than the
  window is `Refused::Late` and one further ahead than the window is
  `Refused::Ahead`; keeping peers inside the window is the transport's
  pacing rule, below, so either is a peer that has already been cut off.
- **Retention** decides which ticks keep a state. Nothing outside
  `history` names a snapshot: `hash_at` and the rewind both ask the ring
  for the newest kept state at or before a tick and re-step from there,
  so sparse snapshots with re-simulation between them change `Retention`
  and the ring and nothing else. `Window` keeps every `every`th tick
  inside `ticks` ticks behind the tick shown, and the newest kept tick at
  or before the window's start, so every tick a command may still arrive
  at has a state to restore. The shipped policy keeps every tick of
  `WINDOW_SECONDS`.
- **Settling.** `acknowledge` records that no command of a seat before
  `up_to` is unknown, and never goes backward; `settled` is the smallest
  over the seats, and the state at it and every state before it is final.
  `advance` acknowledges the local seats up to the tick it reaches, so a
  machine that owns every seat settles every tick it plays. Hashes are
  compared only at settled ticks, since unsettled ticks legitimately
  differ between machines. `settled` names a tick a machine has not
  stepped when every seat's next command is still ahead of it, and
  `hash_at` answers `None` there.
- **Shots and views.** An outcome is kept per tick while the state it
  produced is inside the window, so a view of any tick the frontend shows
  is built with that tick's shots; no field of the state records a shot.
  `outcome()` is the one that produced the tick shown. A rewind replaces
  the outcomes it re-steps.
- **The record** is a `protocol` value over `setup` and `commands`, which
  are the setup and the log to the settled tick. `Session::new` refuses a
  local seat the setup does not seat as `Unseated`, since a match would
  answer it by never settling.
- The harness checks: a record reproduces the live hash; the same match
  played twice hashes the same; a match with every command inserted late,
  in scrambled order inside the window, hashes the same at every settled
  tick as the same commands applied on time.

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
  or an agent, built once per tick from that tick's shots. It is a
  projection and derives nothing itself: every fact it carries is asked
  of the type that owns it — `State::holding` for a post's row, present
  and flying; `Frame::fraction` and `Frame::starved_material` for a frame
  under construction; `Shots::exchanges` for the tick's fire; `Sight` and
  `Radar` for what a seat sees and what it merely detects; `Row::mass_class`
  for what radar can tell of a contact. Own compositions with wants,
  counts present and flying, and each frame's progress with the material
  it has spent nothing on this second for want of; the reserve and
  stockpile; seen entities with id, row, seat, body, HP, whether they
  fly, the place they belong to and, for the seat's own fliers, the place
  they left, both `None` for a flying entity of another team, whose
  destination sight does not give; radar
  blips with body and mass class, which are the entities inside a team
  sensor's radar range and outside its sight; the tick's `Exchange`s,
  which say per place and seat whether a shot the seat saw was fired or
  landed there; the gravity its terrain's orbits are read at; every rock
  with its orbit
  and caps; the standings, `Some` only once the clock has run out.
  Nothing in a `View` refers to anything a seat cannot see. The roster is
  match-constant and travels with the initial state, so a client holds it
  from the session rather than from a view.
- `State::hash() -> u64`: FNV-1a over `Hash` of the whole state. The hasher
  widens every `usize` to `u64` and writes integers little-endian, so a
  `Vec` length prefix hashes the same on wasm32 and native.
- `State::standings() -> Standings`: per team, the rocks where it has a
  structure, the cost total of its living entities, and whether any of its
  seats is still in; `over()` says the clock has run out and `leaders()`
  applies DESIGN.md's tie-break. The clock is a field of the state, so the
  end is a query.

## Game: the display library

- `Scene`: everything one frame draws, built from a `View` by
  `Scene::from_view(view, roster, client)`, from the belt alone by
  `Scene::of_belt(rocks, gravity, tick)`, which is what the lobby and
  loading screens draw before a match exists to have a view of, or by
  hand in `look`. `Scene::centre` is the middle of its rocks, which a
  camera frames the map from. Rocks with
  positions, radii and caps; entities with position, glyph and seat; per ring,
  per seat, the run as a list of `Mark { glyph, fill, dim, reason }` where
  `fill` is `Solid`, `Hollow`, `Filling(f32)` or `Dashed`, `dim` is what
  the pointer says is leaving or arriving, and `reason` is the state the
  glyph stands in, whose `sentence` is what hovering it shows; per ring,
  per seat, an optional
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
- `tint::toward(base, caps, strength)`: a rock's colour from its caps, so
  a region reads as one hue. `belt` paints a rock's mesh with it and `hud`
  strokes that rock's rings with it, faintly, since a lobby and a match
  draw their rings through the same code; a selected ring is white and
  wider, so its brightening wins over the tint rather than mixing with it.
- `stencil::Stencil`: one glyph painted on the HUD — the frame in its fill
  state, its marks, the dim a preview draws it at, and the belt a starved
  frame carries in its material's `hue`. The HUD and the wheel paint
  through it, so a glyph is drawn one way.
- `hud::glyph_at`: the run glyph under a point, off the same `Layout` the
  paint reads, which is what the hover sentence is found through.
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
- The binary is the playable: a `Game` whose `tick` and `frame` are the
  live screen's of the `Flow` (Game: net and screens, below) and nothing
  else; in `Play`, the tick inserts the local controllers' stamped
  commands into the `Session`, advances it within the pacing rule, reads
  the tick's `View` and feeds the `Fights`. `Play`'s frame builds a
  `Scene` and draws it — `belt` then `hud`, so the HUD is never occluded
  — over one full-window transparent egui layer that claims no widgets,
  and every other screen is drawn on that same layer. Input is keyboard
  and mouse through the engine's action vocabularies, which `controls.rs`
  declares: a ring click selects and focuses, a wheel band click edits
  one want and repeats while held, a left drag from ring to ring is the
  send, the right or middle button and the pan keys drag the belt, the
  zoom axis zooms or, during a send, sets how many go, and Escape opens
  the pause screen and closes it again. The gamepad bindings DISPLAY.md
  states are a later unit. Its own headless drive, behind the `look`
  feature, plays a whole skirmish through the engine's offscreen
  `Session` — title to lobby to a placement to the standings — picks a
  team out of an open list, removes a guest from a room it serves, and
  clicks a disabled Quit; it writes `game/look/title.png`,
  `title_quit_reason.png`, `lobby.png`, `lobby_choice.png`,
  `glyph_reason.png` and `results.png`. Every test of it holds one lock,
  since each title serves a room on the protocol's one port. `check.sh`
  runs it under `xvfb-run`, so the drive and its screenshots are verified
  on every change.

## Look

`look` is a binary of `game`, behind the `look` feature, over the
engine's `offscreen` feature and its default `ui` feature so the HUD
lands in the pixels. It holds the three fixed scenes from DISPLAY.md as
code, renders each through a `Session`, reads pixels back, and writes
PNGs under `game/look/`, which is `.gitignore`d: screenshots are the
judgement's input, never committed. `cargo run -p probe-game --features
look --bin look` runs it; `check.sh` builds it, with `cargo build -p
probe-game --features look --all-targets`, and runs the playable's own
drive under `xvfb-run`, so the default binary and the wasm build never
pull `image` or `offscreen`. It is the only way a display change is
verified.

## Agents

`agents` is the agent frontend, a library over `sim` and the `harness`
binary. `Agent::decide(&mut self, view: &View) -> Vec<Command>` is
called on `DECISION_INTERVAL`, at most `MAX_COMMANDS_PER_DECISION` per
call. `Seated { seat, agent }` builds the view and stamps the seat and
the sequence, so no caller does. `Scripted` is the shipped opponent: a
`Personality`'s constants read through `Survey` (one decision's tally of a
view, with memory folded in) into a `Plan` (a target composition per
place, diffed against the view into `Want`s), stepping a `Memory` (what
the view carries no history of) and a `Dice` (the one seeded,
deterministic source of variation an agent has). `Roles` reads the roster
once into the row an agent prefers per job, so no agent names a row by
id. `Personality::of(protocol::Bot)` is the one place a lobby's bot
becomes constants, and it is exhaustive, so a bot the protocol can name
always has a way of playing. A bot in a lobby is a `Seated` agent run by
the machine that owns its seat, through `game`'s bot controller; the sim
never knows.

`harness`, native only, no feature gate: `match` seats agents and plays
one to the clock, tracing standings as it goes; `replay` checks the
record reproduces the live hash and that the same match built twice from
independent initial states hashes the same; `rollback` inserts every
command late and scrambled and checks the settled hashes match the
on-time match; `matrix` plays named compositions pairwise from symmetric
starts and prints a table of rocks and the tie-break's verdict. Doubling
the tick rate and asserting the same outcome is a future check threading
through flights, weapon intervals and the manoeuvring gains.

## Protocol

`protocol` is every value two machines exchange, serialisable, with no
io and no engine, so `game`, `server` and the harness's record files all
speak it.

```rust
pub struct PlayerId(pub u32);                   // PlayerId::HOST opens a lobby
pub enum Bot { Turtle, Expand }                 // agents turns it into a Personality
pub enum Control {
    Open, Closed, Player { player: PlayerId, ready: bool }, Bot(Bot),
}
pub struct SeatSlot { pub team: TeamId, pub control: Control }
pub struct Lobby {                              // MAX_SLOTS == MAX_SEATS slots
    slots: Vec<SeatSlot>, seed: u64, clock: Tick, host: PlayerId,
}
pub enum LobbyEdit {
    SetSlot { slot, control }, Kick(PlayerId), SetTeam { slot, team },
    SetSeed(u64), SetClock(Tick), SetReady { ready },
}
pub enum Refused {
    NotHost, NotYours, NotSeated, NoSuchSlot, BadTeam, BadClock,
    AlreadySeated, NotAGuest, HeldByAGuest,
}
pub enum NotReady {
    NoSeats, OpenSeat { slot }, Unready { slot }, HostUnseated,
}
pub enum Refusal { Edit(Refused), NotReady(NotReady), Full, Version }
pub enum Holder { Open, Player(PlayerId), Bot(Bot) }  // Control minus Closed
pub struct Seating { holders: Vec<Holder>, host: PlayerId }
pub struct Started { setup: Setup, seating: Seating }  // paired by Started::new
pub struct Crew { player: PlayerId, watched: SeatId, seats: Vec<SeatId> }
pub enum Message {
    Join { version: u32 }, Welcome { player, lobby }, Edit(LobbyEdit),
    Lobby(Lobby), Refused(Refusal), Start(Started), Rematch,
    Command(Stamped), Acknowledge { seat, up_to: Tick }, Hash { tick, hash },
    Desync { tick }, Leave, Removed,
}
pub struct Record { setup: Setup, ticks: BTreeMap<Tick, Batch> }

impl Lobby {                                    // the shape and the numbering
    pub fn skirmish(host: PlayerId) -> Lobby;            // every seat one machine's
    pub fn room(host: PlayerId) -> Lobby;                // the host and one open seat
    pub fn edit(&mut self, by: PlayerId, edit: LobbyEdit) -> Result<(), Refused>;
    pub fn freeze(&self) -> Result<Started, NotReady>;
    pub fn seat_of(&self, slot: usize) -> Option<SeatId>;
    pub fn slot_of(&self, player: PlayerId) -> Option<usize>;
    pub fn players(&self) -> Vec<PlayerId>;
    pub fn readied(&self, player: PlayerId) -> bool;
    pub fn admit(&mut self, player: PlayerId) -> Option<SeatId>;
    pub fn release(&mut self, who: PlayerId) -> bool;
    pub fn next_seed(&self) -> u64;
}
impl Seating {                                  // who holds and who runs a seat
    pub fn owner(&self, seat: SeatId) -> Option<PlayerId>;
    pub fn seat(&self, player: PlayerId) -> Option<SeatId>;
    pub fn seats(&self) -> impl Iterator<Item = (SeatId, Holder)>;
    pub fn seats_of(&self, player: PlayerId) -> impl Iterator<Item = SeatId>;
    pub fn players(&self) -> Vec<PlayerId>;
    pub fn peers_of(&self, player: PlayerId) -> usize;
    pub fn run_by(&self, player: PlayerId) -> Option<Crew>;
}
impl Record {
    pub fn of(session: &Session) -> Record;
    pub fn played(setup: Setup, ticks: BTreeMap<Tick, Batch>) -> Record;
    pub fn replay(&self, until: Tick) -> State;
}
pub trait Wire { fn encoded(&self) -> Vec<u8>; fn decode(&[u8]) -> Result<Self, Malformed>; }
```

`Lobby::freeze` is the one way a match starts, on every machine, from the
same value; `sim`'s `Setup` holds only what the state needs (teams per
seat, seed, clock), and the `Seating` beside it holds who runs each seat,
which the sim never learns. A closed slot is not in the match, so a seat
is a slot's place among the slots that are not closed: one private
numbering computes that, and `seat_of` is the mapping the table's own
colours read.

Ownership is asked of the `Seating` and nowhere else. `owner` answers
which machine runs a seat — a person's own, and the host's for a bot,
since only a host seats one — and `players`, `peers_of` and `seats_of`
follow from it, so the room forwarding a command and the machine playing
it never count the machines differently. `run_by` is a machine's whole
part of a match: the seats it runs and the one it watches, never empty,
so a `Crew` is proof that its machine can play. Readiness lives inside
`Control::Player`, where it means something; a bot and a closed seat have
none to hold. `Bot` names a shipped opponent and `agents`'
`Personality::of` matches it exhaustively, so a bot the protocol can name
always has a way of playing.

Every rule a control is enabled by is a function here, evaluated on a
copy: `Lobby::edit` for a holder, a team, a kick, the seed, the clock and
readiness, and `Lobby::freeze` for the start. A player holds one slot at
most (`Refused::AlreadySeated`), only a guest's slot is kicked
(`Refused::NotAGuest`), and a guest's slot is opened by that kick alone
(`Refused::HeldByAGuest`), so the screen offering those values and the
room applying them cannot disagree. A lobby freezes only where the host
runs a seat of it (`NotReady::HostUnseated`): every machine in a started
match holds a `Crew`, which is what makes building one unrefusable.

`Message::Refused` is how a room answers what it did not do: a lobby edit
that was not the sender's, a start of a lobby that is not a match yet, a
join of a room with nowhere to sit, and a join carrying a version that is
not `VERSION`, which the room checks before it seats anything.
`Message::Removed` is what a kicked machine hears, distinct from the
`Leave` a host's own departure broadcasts, since the two are different
facts on the title's join field. `Message::Rematch` is the host asking a
room to open its lobby again, which the room answers with that lobby.
`Message::Hash` means two things by direction: a machine reports its own
hash at a settled tick, and the room sends back the hash every machine
reported the same there, which is the one word that says a tick is
agreed on every machine.

`Wire` is the one encoding, blanket-implemented for every serialisable
value: CBOR, the same bytes on native and in the browser. Reading is the
only place a wire value is checked, and it goes through the constructor:
`Setup` deserialises through `Setup::new`, so a seat count off the wire
is checked once, `Started` through `Started::new`, so a setup and a
seating off the wire name the same seats and a machine can build the
match either describes, and `Record` folds its flat list of commands into
one `Batch` per tick, so a record that replays is the only one that
exists.

## Game: net and screens

- **Controllers.** `Controller::of(&Seating, &Crew, roster)` is one
  controller per seat, in seat order: `Human` for the seat the crew's own
  player holds, `Bot` for each seat the crew runs a `Seated` agent for,
  and `Remote` for a seat another machine runs, which issues nothing here.
  Only a host seats a bot, so a bot seat is the host's machine's and every
  other machine holds it as `Remote`. Every controller
  yields `Stamped` commands at the session's latest tick, which the
  machine inserts into its own session at once and hands to the
  transport. Nothing waits. A `Human` holds what the frame's gestures
  asked for until the next tick stamps it, and never more than
  `MAX_COMMANDS_PER_TICK`, so the tick's batch takes every command a
  controller yields and the insert cannot be refused.
- **The machine.** `Machine` is this machine's whole part of one match:
  the session, one controller per seat, the crew it runs and the pace.
  `Machine::tick` is the lockstep loop — hear, pace, issue, step, tell —
  and takes the transport as an argument, so nothing about it needs a
  screen or the engine and two of them run in one process under test.
  `Machine::of` takes a `Started` and a `Crew` of its seating and refuses
  nothing: the crew names seats the setup holds, so the session cannot
  turn it down, and the seat the display follows is the crew's watched
  one. The seats a machine runs and how many peers it has both come from
  that seating, so a host running a bot from a closed slot of its own is
  a peer like any other and its guests wait for the first agreed hash.
- **Transport.** `Transport` is a trait of five calls — `send`,
  `acknowledge`, `report`, `received`, `leave` — carrying stamped
  commands, acknowledgements and hash reports both ways. `Local` returns
  nothing and exists so the loop has one shape. `Socket` speaks
  `protocol::Message` over a WebSocket to the room, through one `Link`
  per target: a runtime on a thread of its own on the desktop, the page's
  own WebSocket in the browser. `Flow` owns the transport, not the match,
  so a room outlives the match played in it; a machine takes it as
  `&mut dyn Transport` and the choice is one runtime value. Received
  commands are inserted into the session, which rewinds as needed; the
  loop reads `Rewound` only to reset client-side memories (fights, hover)
  that may now be stale. Every match calls all five, so the loop has one
  shape whether or not it has peers, and every match over the messages it
  hears names every variant rather than catching a rest. A screen reads
  the same socket as `Word`, which is what a room says to a screen and
  nothing else, so a message only a room hears or only a machine reads is
  refused where the bytes are decoded rather than matched away in a
  screen.
- **Pacing.** A machine advances no further than the retention window
  ahead of the lowest acknowledged tick among peers, and holds past that;
  a machine ahead of the settled tick by `LEAD_THRESHOLD` drops one step
  in `SLOW_EVERY` of the steps it is offered until level, which is a
  quarter off its tick rate. A dropped step is counted against the steps
  offered and never against the tick reached, which a dropped step does
  not change. Each local seat is acknowledged every
  `ACKNOWLEDGE_INTERVAL` ticks and the settled tick's hash reported every
  `REPORT_INTERVAL` ticks, each settled tick once. The engine's tick
  interval is fixed at boot, so a machine paces its own sim: an engine
  tick is an opportunity to step, and the pace says whether to take it.
  A seat whose machine has left the room is acknowledged for the rest of
  the match by the room on its behalf, so the others settle every tick
  and the match goes on without it. Every one of these numbers is a
  constant with units and a hypothesis in its rustdoc.
- **Screens.** One `Flow` owns one `Stage`, which is the screen showing
  and the room behind it: `Title { listener, asking }`, and `Lobby`,
  `Loading`, `Play` and `Results`, each with its screen and its
  `Authority`. `Pause` is the match's own, since play holds whether it is
  open. A stage's frame answers the next stage out of its own parts, so
  every transition is one method named for what it does — `opens`,
  `starts`, `plays`, `ends`, `rematches`, `leaves`, `removed` — and no
  screen reaches into another. The title alone asks the flow for
  something: `Ask::Host` and `Ask::Join(address)` open a socket, which no
  screen does itself. The title's own screen outlives every stage, since
  DISPLAY.md keeps the address it holds.
- **The authority.** `Authority` is who owns the lobby a match is set up
  in and what carries that match: `Local`, where every seat is on this
  machine and nothing goes on the wire; `Guest`, a room another machine
  serves, holding the id its welcome gave; and `Host`, a room this machine
  serves and plays in. It answers `me`, the transport, and the lobby rule
  itself: `Local` applies an edit to its own copy, and a guest and a host
  alike send it and take the lobby the room broadcasts, so one rule runs
  on both paths. Skirmish opens a `Local` lobby; Host serves a room in
  this process and joins it at the loopback; Join opens a socket to a
  typed address, and either way the lobby is the one the welcome brings,
  so the title says it is connecting until then. Start freezes the lobby
  into a `Started` — the room's own copy where there is a room, which
  broadcasts it — and every machine builds the initial state from it and
  reports the tick-zero hash; loading holds until the room says every
  machine agreed it. A desync and a peer too far behind are the two
  states play holds in, both over the dimmed HUD, and the belt reads no
  input under either, nor under the pause screen: the gesture the pointer
  was in the middle of is dropped with the frame the hold begins.
  Results holds the score, the belt the match ended on and the lobby it
  came from, and returns to that lobby for a rematch or to the title.
  Results is reached when the view's `Standings` arrive, which is the
  clock: DESIGN.md's fog reveals them then and never before, so a client
  cannot see another side's elimination, and the end at elimination is
  the unit that can.
  The lobby draws the belt behind everything at `PREVIEW_ZOOM`, the
  widest view whose rings stand apart, and lays its seats out by team:
  one heading per team that holds a seat, and its seats under it, each a
  holder choice, a Kick beside a guest, a team choice and a readiness
  mark. A closed seat is not drawn, so a team holding none is not drawn
  either, except the one closed seat's row the host is left under the
  last team, which is how a seat and a team are opened.
- **The title's room.** The title stage holds the listener for the room
  Host would join, bound when it opens, and hands it to the authority
  with the welcome. Host is enabled exactly where a listener is held, so
  a click on it cannot fail, and a title the player leaves for anything
  else drops the listener with the stage, which releases the port.
- **The styled register.** Every screen outside the belt is painted and
  hit-tested by hand through one `Panel`, over the engine's UI layer, and
  claims no widget: the HUD's palette, thin lines, no window chrome,
  glyphs through the same `Stencil` the HUD paints with. A screen's
  geometry is one value per screen — `Places`, with a rect per control —
  which the paint, the hit test and the headless drive all read, so a
  control cannot be drawn where it is not clicked or clicked where the
  drive cannot find it.
- **The controls.** `screens/control.rs` is the whole vocabulary, and no
  screen paints a control of its own: `Controls::action`, `::choice` and
  `::value` are DISPLAY.md's three kinds, each taking a `Rule` — `Allows`
  or `Refuses(sentence)` — computed from the same function the action
  will call. `Controls::finish` paints the open list and the one hover
  sentence after every control, so neither is drawn over, and answers
  whether a click landed on no control, which is how a list closes. A
  refusal is never shown after the fact: the only outcome a control
  cannot know before the click is a join, and the join field carries it.

## Server

`server` holds rooms. Each room owns one `Lobby` and is the authority on
it: the host's edits and each guest's own-seat edits are applied in
arrival order and the result broadcast; any other edit is refused by
name to its sender. A machine enters a room by asking to join, takes the
first open slot, and is welcomed with the id it holds and the lobby as it
stands; a room with nowhere to sit, or a join carrying another version of
the protocol, refuses it by name. A kick is the host's edit like any
other: the room opens that slot, tells the machine that held it, and
drops it. The shape of a lobby is its host's, so a host that leaves one
ends it: the room reopens and every other machine is sent away.

A room's phase owns the machines in it, so no second list of them can
disagree with what the room is doing: a room setting a match up holds the
machines that joined it, in the order they did, and a room forwarding one
asks the match, which knows the machines its seating gives a seat to and
which of them have left. The id the next machine takes only rises for the
life of the room, so a room that reopens hands out no id it has held
before.

A room keeps the lobby the match was set up in while it forwards that
match, so the host's `Rematch` opens it again with its shape kept, the
slots of machines that have left standing open. Every machine at the
results follows the lobby the room broadcasts.

Once started, the room forwards every stamped command and acknowledgement
of a seat to every machine but the one that sent it, and drops one of a
seat its sender does not own. It collects hash reports per settled tick,
declares a desync to every machine when two reports differ there, and
sends back the hash where every machine reported the same, which is how a
machine knows a tick is agreed. It keeps the one record it forwarded —
the log every machine converges on — and holds it in memory when the
match ends; a per-machine record would need a machine to upload one,
which no message asks for. It holds no tick clock and steps no sim.

`Room` is pure and knows nothing of sockets: it answers `Post`s addressed
to the sender, to one named machine, to everyone, or to everyone else,
and `stream.rs` is the only part that touches the network. `game` embeds
it under the `host` feature so host-by-address needs no separate process;
the binary serves a room list later.

## Module layout

```
sim/src/
  lib.rs            TICKS_PER_SECOND, TICK; the public surface
  belt.rs           Belt::fixed, State::start
  setup.rs          Setup, MAX_SEATS, BadSetup
  real.rs           Real
  vec3.rs           Vec3
  materials.rs      Material, Materials, Stockpile
  time.rs           Tick, Moment
  ids.rs            RockId, EntityId, RowId, SeatId, TeamId
  place.rs          Band, Place, Post
  roster/           mod.rs Roster; row.rs Row, Weapon, Kind, MassClass;
                    shipped.rs the eight
  orbit/            body.rs Body, Gravity; elements.rs Orbit;
                    stumpff.rs; universal.rs; lambert.rs
  state/            mod.rs State, Index impls, queries; seat.rs; rock.rs;
                    entity.rs Entity, Motion; flight.rs Flight, Burn;
                    attractor.rs; sight.rs Sight; radar.rs Radar;
                    wants.rs Wants; frame.rs Frame, its fraction and what
                    it went short of; ready.rs; command.rs Command,
                    Issued, Stamped, Batch, Sequence, Rejected, Refused,
                    apply; sweep.rs; view.rs View; hash.rs;
                    standings.rs Standings
  step/             mod.rs step, next; maneuver.rs; propagation.rs;
                    fulfilment.rs; extraction.rs; construction.rs;
                    fire.rs Fire, Shots, Exchange
  history/          mod.rs; snapshots.rs Retention and the ring behind it;
                    log.rs the stamped log by tick;
                    session.rs Session: advance, insert, acknowledge,
                    settled, hash_at, outcome_at, setup, commands
protocol/src/
  lib.rs            the surface, DEFAULT_PORT, the port a room is served
                    on, and VERSION, the version a room takes a join of
  ids.rs            PlayerId
  lobby.rs          Lobby, SeatSlot, Control, Bot, LobbyEdit, freeze
  seating.rs        Seating, Holder, Started, Crew: who runs which seat
  message.rs        Message
  record.rs         Record
  wire.rs           Wire, the one encoding
agents/src/
  lib.rs            Agent, Seated, DECISION_INTERVAL; the surface
  dice.rs  memory.rs  roles.rs  survey.rs  plan.rs  personality.rs
  scripted.rs       Scripted
  bin/harness.rs    native only: match, replay, rollback, matrix
game/src/
  lib.rs            the surface
  controls.rs       Controls: the buttons and axes the playable reads
  display/          mod.rs; scene.rs glyph.rs glyph_quad.rs ring.rs
                    wheel.rs camera.rs screen.rs stencil.rs tint.rs
                    fights.rs send.rs belt.rs hud.rs, as before;
                    hue.rs the three materials' colours; label.rs titled
  net/              controller.rs Controller, Human; transport.rs
                    Transport; local.rs Local; socket.rs Socket;
                    link/ native.rs and browser.rs, one per target;
                    room.rs Room, the socket to it, and Word, what it
                    says to a screen;
                    hosting/ Hosting, the room this machine serves:
                    served.rs with the server, nowhere.rs without it;
                    machine.rs Machine, the lockstep loop; pace.rs pacing
  screens/          mod.rs Playable; flow.rs Flow, Stage, Authority, Ask
                    and every transition; panel.rs the styled register;
                    control.rs the three kinds of control; field.rs the
                    one typed line; held.rs the two states play holds in;
                    title.rs lobby.rs loading.rs play.rs pause.rs
                    results.rs, each with the Picked its actions ask for
  main.rs           the playable, and its headless drive
  bin/look.rs       behind the `look` feature
server/src/
  lib.rs            the surface game embeds
  rooms.rs          Room, its phase with the machines in it, and the
                    posts it answers with; the lobby it is the authority
                    on is kept across the match a rematch opens it after
  playing.rs        Playing: its seating, forwarding, hashes, desync
  records.rs        Ledger, the log being forwarded; Records, the kept ones
  stream.rs         the accept loop and one task per machine
  main.rs           the standalone binary
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
| `libm` | `sim`, `agents` | transcendentals identical on every target; `std`'s are the platform's; an agent's arithmetic must replay identically too |
| `mirage-engine` | `game` | the engine, by path; `look`'s bin additionally needs its `offscreen` feature |
| `image` | `game`, behind the `look` feature | writing PNGs; already in the engine's tree |
| `serde` (derive) | `sim`, `protocol`, `agents` by way of them | the derives every wire value takes; adds no arithmetic, so determinism is untouched |
| `ciborium` | `protocol` | one wire encoding for native and the browser: CBOR, `no_std`-capable, self-describing so a record file survives a field addition, and it needs no io in the crate. It beat `postcard`, whose wire is smaller, on reachability — `postcard` is not fetchable in this environment — and on self-description; revisit `postcard` if wire size ever matters |
| `tokio-tungstenite` (with `tungstenite`) | `server`; `game` on native | one WebSocket implementation for both ends of the wire, so the handshake and the framing are the same code. Chosen over hand-rolling a socket, which the environment's blocked downloads would otherwise have forced; `ewebsock`, which would have unified the two client paths behind one API, is not fetchable here |
| `tokio` | `server`; `game` on native | the runtime `tokio-tungstenite` needs: one socket's reads and writes selected over without a timeout or a poll. Chosen over `axum`, which adds `hyper`, routing and a tower stack for a room that needs a listener and a handshake and nothing else, and which stays the right answer when the binary grows a room list; `smol` and `async-std` are not fetchable here |
| `futures-util` | `server`; `game` on native | the `Stream` and `Sink` halves of a WebSocket |
| `web-sys` (`WebSocket`), `js-sys`, `wasm-bindgen` | `game` on wasm32 | the browser's own socket, at the engine's own versions. One link per target rather than one crate over both: nothing fetchable here abstracts a native and a browser socket together, and the two are small enough that a shared abstraction would be longer than either |

Adding one requires a row here. A row naming "to be named" is a
placeholder for the unit that lands it, which replaces it with the crate
and the reason it was chosen over its alternatives.

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
