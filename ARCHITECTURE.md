# Neumannarch — architecture

How the code is shaped. This document describes the target; it never
records interim status. DESIGN.md is the authority on the rules the sim
computes and DISPLAY.md on what the player sees; this document is the
authority on types, modules, and the seams between crates. If
implementation reveals a problem here, stop and propose a change to this
document.

## Principles

1. **Invalid states are unrepresentable.** A structure has no velocity,
   a burn cannot exceed its row's limit or last no ticks, a composition
   with nothing in it does not exist, a command names an asteroid and a
   row and nothing else. Where the type system cannot say it, one
   runtime check says it, with a comment naming the shape change that
   would delete the check.
2. **The step is a pure function.** `State::step(&self, ..) -> State`.
   Every phase reads the tick-start snapshot and returns an effect value;
   the next state is built from the snapshot and the effects. No phase
   mutates.
3. **One law of motion, two kernels.** Every body moves by exact two-body
   motion about the central mass. `Orbit::at` reads a fixed elliptic orbit
   at a tick, for asteroids; `universal::propagate` advances a
   thrusting body one tick, for ships. One test ties the kernels together:
   `kepler_agrees_with_the_universal_propagator` asserts
   `Orbit::at(t + dt)` equals `propagate(orbit.at(t), dt)` over a spread of
   orbits and spans. Every send is one solved transfer between two asteroids,
   flown from a schedule the solver integrated to the tolerance the step
   flies it to; beyond it the only code steering a ship is the holding rule.
4. **Determinism by construction.** `f64` only, transcendentals through
   `libm`, ordered containers, id-ordered iteration, no clocks, no
   randomness. The state hash is derived from every field, never listed.
5. **The sim knows nothing of the engine.** `sim` builds for both targets
   with no dependency on `game` or on Mirage. `game` adapts the sim's
   view to a scene and draws it; `harness` drives the sim headlessly, and
   `look` drives the display headlessly over hand-built scenes.

## Crates

```
sim/       neumannarch-sim       the state system: state, rules, orbit, roster,
                           belt, and history (snapshots, the stamped log,
                           rewind, settling, replay); no engine, both
                           targets
protocol/  neumannarch-protocol  every value two machines exchange, defined in
                           Protocol below; serialisable; no io; both
                           targets
agents/    neumannarch-agents    the agent frontend: the Agent trait, the
                           scripted opponent, the personality a lobby's
                           bot plays by; bin: `harness`, native only
game/      neumannarch-game      the player frontend: display/, net/, screens/;
                           bin: the playable on Mirage; `look`, behind a
                           `look` feature, synthetic scenes through the
                           engine's offscreen Session to screenshots
server/    neumannarch-server    the match server: rooms, lobby authority,
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
- `Materials([f64; 3])`: the triple over `Material::EVERY`, with
  component-wise arithmetic and `min`. A row's cost, an asteroid's caps, a
  row's capacity and a player's stock are all `Materials`, since the
  arithmetic is identical. `Index<Material>` and `IndexMut<Material>` are
  the one way in, and `amounts()` walks the three in `Material::EVERY`;
  there are no named fields, so no caller can read one material without
  naming it. `Materials::new` takes metals, volatiles and energy in that
  order.
- `Stockpile { stock: Materials, capacity: Materials }`: the only place
  materials are held. Income clamps to capacity, spending never goes
  below zero, refunds clamp. No other type touches a stock directly.
- `PerSecond { filling: Materials, completed: Materials }`: one second's
  accumulation of a `Materials` fact, beside the stockpile because it is
  `Materials` arithmetic and nothing else. It is `pub(crate)`; a seat's
  and an asteroid's readers hand out the completed `Materials` alone.
- Transcendentals: `libm::sin`, `cos`, `sinh`, `cosh`, `atan2`, `exp`,
  `log`, `pow`. `f64::sqrt` is exact under IEEE 754 and allowed.
  `sim/clippy.toml` disallows the `std` transcendentals and `HashMap`,
  `HashSet`, `Instant`, `SystemTime`.
- `Tick(u64)` is the count of steps taken and nothing else. The lockstep,
  the log, the snapshots, the settled hashes, a command's stamp and the
  draft's stages are all about steps, so they take a `Tick`.
- `Time(u64)` is the match's own time, the tick less the tick the clock
  started at, zero for every tick of the draft. Everything physical takes
  a `Time`: an asteroid's orbit, a send's departure and arrival, a schedule's
  burns, a frame's fed window, the per-second rotation, the standings and
  the win. The two are separate types, so a schedule cannot be handed a
  step count and the belt cannot turn while the clock is stopped.
  `State::time` is the one place a tick becomes a time.
- `Moment(f64)` is a fractional `Time`, used for weapon ready times and
  shot ordering. `TICK` and `TICKS_PER_SECOND` in `lib.rs` are the only
  place seconds meet either.

## Sim: the state

```rust
pub struct State {
    tick: Tick,
    length: Tick,                   // the match runs this long once the clock starts
    draft: Draft,                   // the stages, the one running, the tick it ended
    gravity: Gravity,               // the central mass's μ
    seats: Vec<Seat>,               // team, alive, stockpile, reserve, income, spend
    roster: Roster,                 // the movement limit and Vec<Row>
    asteroids: Vec<Asteroid>,               // orbit, caps, radius; indexed by AsteroidId
    entities: BTreeMap<EntityId, Entity>,   // dead ones removed
    next_entity: EntityId,          // the id the next spawn takes
    wants: BTreeMap<Post, Wants>,   // sparse: only posts with something
    frames: Vec<Frame>,
    ready: Vec<Ready>,              // damage weapons' next ready moments
}

pub struct Post { pub asteroid: AsteroidId, pub seat: SeatId }      // one composition
pub struct Posting { post: Post, row: RowId }               // one row of one composition
pub struct Draft { stages: Vec<Stage>, running: usize,
                   began: Tick, ended: Option<Tick> }
pub struct Stage { pub seat: SeatId, pub row: RowId,
                   pub placed: Option<AsteroidId> }
pub struct Seat { team: TeamId, alive: bool, stockpile: Stockpile,
                  base_capacity: Materials, reserve: BTreeMap<RowId, u32>,
                  income: PerSecond, spend: PerSecond }
pub struct Asteroid { orbit: Orbit, caps: Materials, radius: Real,
                  pull: PerSecond }
pub struct Frame { post: Post, row: RowId, progress: Real }
pub struct Ready { entity: EntityId, weapon: u8, at: Moment }

pub struct Entity {
    id: EntityId, seat: SeatId, row: RowId, home: AsteroidId, hp: Real,
    motion: Motion,
}
pub enum Motion {
    Fixed,                          // a structure: its body is its asteroid's
    Steered { body: Body, flight: Option<Flight> },
}
pub struct Body { pub pos: Vec3, pub vel: Vec3 }            // inertial frame

pub struct Flight { source: AsteroidId, schedule: Schedule }
pub struct Schedule { burns: [Burn; 2], arrive: Tick }
struct Burn { from: Tick, ticks: NonZeroU32, accel: Vec3 }  // held whole ticks
pub struct Route { source: AsteroidId, destination: AsteroidId, seat: SeatId }
pub struct Send { route: Route,                                 // one schedule,
                  schedule: Schedule, members: Vec<EntityId> }   // every member
```

A place is an asteroid, so `AsteroidId` is the place: a post is an asteroid and
a seat, the verb names an asteroid, and an entity's home is an asteroid.
A `Posting` is one row of one post, which is what the verb addresses and
what every map over a row at a place is keyed by: the holdings, the
plans, the shortfalls, the openings. It orders by asteroid, then seat,
then row, so a walk over one of those maps is the walk over posts and
rows the rules already take. Where a map is the viewer's own the seat is
the viewer's, and there is no second type for it.
`State::entities_at` answers who is homed at an asteroid and
`State::standing_at` who is at it now; the two differ only for a unit whose
send is still forming.

- The draft is a fact of the state and the clock hangs off it. It is a
  sequence of stages, one per reserve row a seat holds, capped at
  `STAGES_PER_SEAT`: the first round in an order `Draft::of` draws from
  the seed through `hash::digest(&(seed, seat))`, ties by seat id, so no
  dice type and no platform call decides it, and the second round in
  that order reversed, so the seat that went first goes last. A seat's
  own rows go down in order of Build rate, the greatest first, ties by
  row id, so the shipyard takes the seat's first asteroid and its
  economy's home is the asteroid it chose first. One stage runs at a
  time. `Draft::place` ends the running stage the moment its seat places
  and begins the next at that tick; `Draft::pass` ends it at
  `STAGE_SPAN` after it began and begins the next. A seat whose stage
  ran out keeps its unplaced stage and may place at any later tick,
  alongside the running one, which is why `Draft::awaits` answers by the
  stage's place in the sequence and not by whose turn it is:
  `Some(true)` where the seat's unplaced stage has begun, `Some(false)`
  where it has not, `None` where the want is no pick at all. A `Stage`
  carries the seat, the row and the asteroid it placed at, so the draft
  alone answers what is free, what a seat has placed and which stages
  ran out unplaced, and the panel is drawn from it with nothing derived;
  nothing counts the reserve, which no placement spends until the clock
  starts. `State::apply` treats a want of one as a pick:
  `Rejected::NotYet` before the stage, `Rejected::AsteroidTaken` at an
  asteroid a placement took, otherwise the asteroid is taken there and
  then, so two picks in one tick are first come first served in the
  batch's own order. `State::close_draft` passes a stage that has run
  out and ends the draft on the tick every stage has placed, or at
  `GRACE` after the last stage ended. While it runs, `State::step`
  applies commands and advances the tick and does nothing else: no phase
  runs, so no body moves, nothing is extracted, nothing is built and no
  want is filled, and a want accepted during the draft stands until the
  clock starts. The skipped phases are not redundant with match time
  standing still: fulfilment, extraction and construction act on a tick,
  not on a span, and would fill, credit and build at time zero. The
  draft's end is the clock's start, so `State::time` counts from it and
  is zero throughout the draft, and the match is over when `State::time`
  reaches `length`. Standings, the win, the view's countdown and the
  bots' payback all read match time and need no rule of their own.
- Ids are newtypes over the index into their store: `AsteroidId(u32)`,
  `EntityId(u32)`, `RowId(u16)`, `SeatId(u8)`. `State` implements `Index`
  for each, so a rule reads `state[id]`, and `IndexMut` for `SeatId` and
  `AsteroidId`, so a rule that credits a seat or an asteroid writes `state[id]`
  too rather than matching on an absence that an id off the state's own
  entities cannot have. Dead entities are removed at the
  end of the step and ids are never reused within a match, so an id held
  across a tick is validated by lookup, never by trust.
- `Wants` is a `BTreeMap<RowId, u32>` with no zero entries. A post whose
  wants are empty, whose entities are gone and whose frames are closed is
  removed at the end of the step; that is the whole existence rule.
- A `PerSecond` accumulates one `Materials` fact over one second: each
  tick fills it, and `State::advance` closes every seat's and every
  asteroid's when the tick it reaches is a multiple of
  `TICKS_PER_SECOND`, which is where a second is defined and the only
  place it is. What it reports is the last completed second's total,
  zero until the first second closes, and it stands until the next
  closes. A seat's `income` is what its extractors pulled, gross, so the
  display can draw what capacity lost; its `spend` is what its frames
  drained; an asteroid's `pull` is what every extractor there took, of
  any seat. Three methods fill one and no other code does: `Seat::earn`
  fills a seat's income, `Seat::drain` its spend, and
  `Asteroid::extract` a asteroid's pull. `Seat::refund` returns a
  cancelled frame's materials and fills none of the three, since a
  refund is neither income nor spend. They are fields of the state, so a
  restored snapshot replays them exactly and they hash with everything
  else.
- `Seat::new` takes the stock the seat starts with, which is also its base
  capacity; every tick the capacity is that base plus the capacity of its
  living entities, so nothing a seat starts with is lost.
- An asteroid's `Orbit` is an elliptic conic as equinoctial elements with the
  tick its mean longitude is stated at; an asteroid never thrusts, so its body
  at any tick is `Orbit::at`. Its `radius` is its own size, for drawing;
  `Belt::ZONE_RADIUS_METERS` is the zone, one constant of the belt for
  every asteroid, which `belt.rs` owns and one test holds against the belt's
  own spacing. Beside it `belt.rs` owns the zone's other three constants,
  which every asteroid shares and no row states: `FIELD_SCALE_METERS`, the
  strength field's reach; `ARRIVAL_METERS`, the distance a term toward a
  place must stop within; and `SPACING_METERS`, the distance a pair settles
  at, which is also the step between two units spawning at one asteroid and,
  above the asteroid's own radius, the floor holding keeps them off. One test
  holds every shipped asteroid's floor inside its zone, so the zone is always a
  shell.
- `Roster` owns the movement limit and the rows. The shipped nine are
  built by `Roster::shipped()` from `roster/shipped.rs`, one extractor
  row per material among them, alike in every stat but the material
  their `Weapon::Extract` names; `Roster::add(Row) -> RowId`,
  `Roster::moving_at(Real) -> Roster` and `Roster::units_by(impl Fn(Row)
  -> Row) -> Roster`, which rewrites every unit row and leaves the
  structures alone, build the variants a test or `harness sweep` plays.
  `Roster::movement_limit()` is the one acceleration every unit
  transfers at, so it hashes with the state and a faction can skew it. A
  row's kind is `Row::kind()`: `Structure` when its manoeuvring limit is
  zero, else `Unit`; `Row::steering` is its `Weights`, one per term of
  the holding rule, and `Row::standoff()` is half its longest weapon
  range, `None` where it has no damage weapon, so an unarmed row cannot
  chase. No `Copy` of a row lives anywhere but the roster.
- A `Frame`'s progress is the work done so far, in cost units. Nothing
  complete is ever scrapped, so no entity carries work of its own; surplus
  is `count` above `want` and is read where it is needed, never stored.
- One `Ready` exists per damage weapon of a living entity, and is never
  earlier than the current tick.

## Sim: commands and the session

```rust
pub enum Command { Want { asteroid: AsteroidId, row: RowId, count: u32 } }
pub struct Issued { pub seat: SeatId, pub seq: u32, pub command: Command }
pub struct Stamped { pub tick: Tick, pub issued: Issued }
pub struct Batch(Vec<Issued>);                      // one tick's, ordered
pub struct Sequence { seat: SeatId, next: u32 }
pub struct Setup { teams: Vec<TeamId>, seed: u64, clock: Tick }
pub enum BadSetup { NoSeats, TooManySeats }
pub enum Rejected { NoSuchSeat, DeadSeat, NoSuchAsteroid, NoSuchRow, TooMany,
                    NotYet, AsteroidTaken }
pub struct Preview { shortfalls: BTreeMap<Posting, ShortfallFilling>,
                     refund: Materials }
pub struct ShortfallFilling { from_reserve: u32, sent_from: BTreeMap<AsteroidId, u32>,
                              to_build: u32 }
pub enum Refused { Duplicate, TooMany, Late, Ahead }

impl State {
    pub fn step(&self, issued: &Batch) -> (State, Outcome);
    pub fn admits_want(&self, posting: Posting, count: u32) -> Result<(), Rejected>;
    pub fn preview(&self, seat: SeatId, wants: &[Command]) -> Result<Preview, Rejected>;
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

`State::admits_want` is the one place a want is refused: `apply` asks it
before it changes anything, and the display asks it for every button a
full wheel draws, with the count that button would issue, so a control is
never live and then refused. It takes the `Posting` the button stands
on, so no caller carries the three ids apart. `pick` is left with the
placing alone.

`State::preview` answers what the sim would do this tick if the seat
issued those wants and nothing else changed: it applies them to a clone
as `apply` would, so a want the state refuses comes back by name and
previews nothing, and reads what fulfilment would assign on that clone
less what it assigns on the state as it stands, so a shortfall already
being filled is not credited to the hover. Per posting it reports the
units the reserve would place, the units it would send and the asteroid
each leaves, and the units left to build, which is a count of units and
not of frames, since fulfilment opens one frame per row per tick.
Beside them `refund` is what the frames it would cancel have consumed,
by the share-of-cost rule `Cancellation::refund` owns and `step::fulfil`
refunds by, and `Preview::cost_to_build(&Roster)` is what the units left
to build would cost. Neither cost nor refund is stored beside the parts
it is derived from. Every count a preview carries is what the hover
*adds*, which is what `added_beyond` computes and what its name says: a
hover that lowers a want takes a shortfall away rather than filling one,
and a preview reports nothing there but the refund. A preview never runs
the schedule solve: it settles as though nothing is held back
(`SendSchedules::nothing_held_back`), so a send with no schedule this
tick, whose units stay home and open frames, is a thing the preview does
not know, which is accepted. One preview over the shipped belt with 420
entities takes 242 µs in release, which is why `Play` asks for one when
the hover changes and once a tick while it is held, never once a frame.

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

Each phase reads the snapshot and returns its effect, as a type that
borrows the snapshot or as the effect's own constructor.
`State` has `step`, queries, and nothing else; a phase that wants to live
on `State` is a missing type.

```rust
let snap = &applied;                                   // commands applied, the draft closed
let sweep = snap.sweep();                              // one spatial index a tick
let thrusts = Holding::of(snap, &sweep).run();         // Thrusts: one per steered unit
let moved   = Propagation::of(snap, &thrusts).run();   // Moved: bodies one tick on, flights ended
let filled  = Fulfilment::of(snap).run();              // Assigned: reserve, surplus, frames opened
let income  = Income::extracted(snap);                 // Income: per asteroid and seat
let work    = Construction::of(snap).run();            // Progress: spend, completions
let shots   = Fire::of(snap, &sweep).run();            // Shots: ordered, with pending damage
State::next(snap, moved, filled, income, work, shots)  // deaths, reaping, elimination, tick+1
```

Phases do not see each other's effects; `next` applies them in that fixed
order and then removes the dead, closes empty posts, eliminates seats with
nothing left, and advances the tick. Within a phase, iteration is in id
order; anything sorted is sorted by a total key ending in an id. A phase
that queries by range takes the tick's one `Sweep` as its second argument,
since a read-only index of the snapshot is not an effect.

## Sim: motion

- **Propagation.** `orbit::universal::propagate(body, gravity, dt) ->
  Body` is the two-body solution in universal variables with Stumpff
  functions, exact for every conic. `Body::after_tick(thrust, gravity)
  -> Body` adds the thrust's delta-v over one tick and propagates one
  tick; every ship advances by it once per tick. Asteroids are not
  stored as bodies; `State::asteroid_body(asteroid)` reads the
  asteroid's orbit at the current tick, which costs one Kepler solve
  however old the tick is. `State::body_of(&Entity) -> Body` is the one
  query for where an entity is: its asteroid's body when `Fixed`, its
  stored body when `Steered`.
- **The zone.** Every asteroid's zone is `Belt::ZONE_RADIUS_METERS`
  about its own body, so a force at an asteroid is read off the
  asteroid's orbit and nothing else: `State::asteroid_body` is the whole
  of "where a force here is". The zone is the chase's extent in the
  holding rule and the circle `View::zone` carries out for the display.
  Construction and `Fire` gate by the asteroid a unit stands at rather
  than by a distance, which the zone is what justifies: holding keeps a
  unit inside its own asteroid's zone and no two zones overlap, so the
  set is the same one and no rule pays for a distance test.
- **Schedules.** A `Schedule` is one send's thrust: two `Burn`s
  and the arrival tick, a burn being an acceleration held over whole ticks.
  A burn is built only from a delta-v and the roster's movement limit, so its
  tick count is the ceiling of the delta-v over the limit and its
  acceleration is at or below the limit by construction. `Schedule::coasting`
  builds both burns and answers `Some` only when they leave a coast between
  them: the first starts at the departure tick, the second ends at the
  arrival tick, they do not overlap, and their ticks together are at most
  `BURN_SHARE_OF_SPAN` of the span. That margin is the existence predicate,
  and it is free, so a candidate arrival tick failing it is never integrated.
  `Schedule::between(source, target, depart, arrive, limit, gravity)` solves
  one candidate arrival tick: `orbit::lambert::solve` from the
  source asteroid's body at `depart` to an aim point, prograde and single
  revolution, gives the impulses; `coasting` builds the schedule; the
  schedule is integrated; the aim moves by the miss at `arrive` and the
  solve repeats, at most `CORRECTIONS` times. It answers `Some` on the first
  pass landing inside `Schedule::ARRIVAL_POSITION_METERS` and
  `ARRIVAL_SPEED_METERS_PER_SECOND` of the destination asteroid, so a schedule
  that exists has been flown before it is stored. That integration is in
  three parts: each burn tick by tick through `Body::after_tick`, the coast
  between them as one `universal::propagate`. The step flies the same
  schedule tick by tick throughout. They differ by what the Kepler test
  bounds, far inside the tolerance the solver accepted against, and every
  machine runs this code, so the contract between solver and step is the
  tolerance, not the arithmetic.
- **Sends.** `Send::joining(state, source, destination, seat, members)` is
  the one way a send is made: the schedule of the send already forming
  between those asteroids for that seat, else a newly solved one. The solve
  walks candidate arrival ticks upward one second at a time from the
  departure tick, `Send::FORMING_TICKS` past the current one and one
  more, which is the tick a member first thrusts, and takes the first with
  a schedule at the roster's movement limit; past its bound it answers
  `None`. A `Send` holds that one schedule and its members, so every ship
  of it flies the same burns and arrives on the same tick, whatever rows
  they are. A flight is a `Schedule` and the asteroid it left, carried by the
  unit itself, so an arrived unit drops it and no store is reaped.
- **Forming.** A send forms for `Send::FORMING_TICKS` before it departs,
  and the forming send is that carried flight, not a store of its own: a
  flight whose schedule has not reached its first burn is forming, which
  `Flight::has_departed` answers and the search inside `Send::joining`
  finds by source, destination and seat. At most one such flight exists
  per those three, since a send is solved only where none is found, so a
  unit re-homed inside the window joins the one that is there. `Entity` reads
  the two states off that one tick: `is_flying` is true only from
  departure, and `Entity::standing` is the asteroid a unit is at — the asteroid
  it left while its send forms, its home otherwise, and `None` once it
  flies. Fire, holding, construction and extraction ask
  `standing`, so a forming unit is a shooter and a target where it
  stands; `State::holding` counts by the flight itself, so it counts
  toward its destination from the tick it joins.

## Sim: holding

DESIGN.md's holding rule is one phase and the only code that steers a unit at
an asteroid. Nothing else in the sim names a term, so the rule is replaceable
whole.

```rust
pub struct Holding<'a> { state: &'a State, sweep: &'a Sweep }
pub struct Thrusts(BTreeMap<EntityId, Vec3>);      // one per steered unit

pub struct Power(f64);                             // dps through no plating, times HP
pub struct Fields(BTreeMap<EntityId, Sample>);     // what each unit reads of both fields
pub struct Sample { own: f64, enemy: f64, own_gradient: Vec3, enemy_gradient: Vec3 }
pub struct Fraction(f64);                          // 0..=1

impl Holding<'_> { fn of(&State, &Sweep) -> Holding; fn run(self) -> Thrusts; }
impl Power       { fn of(&Row, hp: f64) -> Power; }
impl Fields      { fn of(&State) -> Fields; fn at(&self, EntityId) -> Sample; }
impl Sample      { fn hostile(&self) -> Option<Fraction>;
                   fn own_lean(&self) -> Vec3; fn retreat(&self) -> Vec3; }
impl Asteroid        { fn strayed(&self, Body, pos: Vec3) -> f64; }
impl Vec3        { fn capped(self, limit: f64) -> Vec3; }

fn wander(&Row, Tick, EntityId) -> Vec3;
fn separation(Body, &Row, impl Iterator<Item = Vec3>) -> Vec3;
fn cohesion(&Row, Sample) -> Vec3;
fn caution(&Row, Sample) -> Vec3;
fn returning(Body, &Row, asteroid: Body, strayed: f64) -> Vec3;
fn chase(Body, &Row, target: Body) -> Vec3;
```

- **The roll.** `Fields::of` groups the entities standing at each asteroid into
  a roll once per tick, sorted by belt-plane `x` then id, and discards the
  grouping once every asteroid's pair sums are folded in. Only a steered entity
  enters it, since only a unit has power; a structure is still a target,
  found through `State::standing_at` where the chase asks for one.
- **The kernel** is `(1 - (r/R)^2)^2` over `Belt::FIELD_SCALE_METERS`, whose
  value and gradient both vanish at `R`, so the two field terms are
  continuous and a unit past the scale contributes nothing. The gradient is
  analytic, never a difference. A field is summed pair by pair over one
  asteroid's roll, each pair once: the kernel is symmetric and its gradient
  antisymmetric, so one evaluation fills both units' own or enemy value and
  gradient, and the `x` order lets the inner walk stop at `R`.
- **No absolute strength leaves the field.** Caution scales by
  `Sample::hostile`, a fraction, and cohesion by `Sample::own_lean`, the
  gradient in units of the field's own value over the kernel's scale, capped
  at one. So a unit reads who is strong here without reading a strength, and
  the lean is near zero inside a crowd and near one at its edge, which is
  what lets separation set the spacing while cohesion still gathers a
  straggler.
- **Arrival steering.** A term toward a place is `weight * (the place's
  velocity + the direction to it times the arrival speed - the unit's own
  velocity)`, the arrival speed being what the row's manoeuvring limit can
  stop from within `Belt::ARRIVAL_METERS`. Every such term damps itself, so
  the rule carries no damping constant. Return's place is the asteroid and its
  miss, `Asteroid::strayed`, is signed: how far the unit is outside the zone,
  less how deep it is inside the asteroid's own radius plus one spacing. So the
  term pulls in past the zone, pushes out from inside the asteroid, and between
  them is the damping alone: the zone is a soft shell and no ship moves
  inside the asteroid. `step::spawn_body` starts its ladder at that floor, so
  nothing spawns inside an asteroid either. The sum of the terms is cut to the
  row's manoeuvring limit by `Vec3::capped`.
- **The shared target.** `state::Threat` is the one threat rule. Holding
  asks it over `State::standing_at` the asteroid, since the chase has no range
  gate; Fire asks it per weapon over the sweep within that weapon's range
  and against the damage already assigned this tick. They name the same
  enemy once the fight is joined, which is what "the unit it chases is the
  unit it fires at" means; before it, holding is closing on an enemy no
  weapon reaches yet.
- **The drift** is a pure function of the tick and the entity id: three fixed
  frequencies whose phase offset comes from the id. Nothing is stored and
  nothing is random, so a rewind reproduces a unit's wander exactly.
- **In flight** a unit stands nowhere, so it takes separation and nothing
  else; propagation adds its schedule's thrust on top, and it spends both
  limits.
- The weights are the row's, one per term (`roster::Weights`), and a
  structure's are `Weights::STILL`. They are hypotheses set by
  `harness sweep`, which plays two forces at one asteroid under roster variants
  and reports how long the force takes to close, whether it spreads while
  closing, how long the fight takes to decide and whether swapping the two
  sides swaps the winner.

## Sim: the rules as code

- **Fulfilment** walks every post in key order. For each row: the
  reserve first, then the nearest post holding a surplus of that row
  (distance between asteroid bodies now, ties by lower asteroid id, the
  highest-id units first), then one frame at the post if anything is
  still missing and none is open there for that row, so a row builds one
  at a time and the next opens the tick after the last completes; rows
  are separate keys, so they open beside each other. A row wanting
  nothing more cancels its open frames, least-progressed first, and each
  refunds what it consumed. A post's open frames of a row cover its want
  before its units do, so a unit at a post whose frames already cover
  the want is surplus; and the cancelling runs after the tick's sends
  and counts every unit leaving as gone, so a frame is cancelled only
  where the units that stay cover the want. Lowering a want a shortfall
  elsewhere wants sends the unit and keeps the frame building; lowering
  a want nothing else wants cancels the frame and keeps the unit. Units
  re-homed from one asteroid to one asteroid join the send forming
  between that pair for that seat, or open one, solved before they are
  counted as filling anything; a send with no schedule leaves its units
  home this tick and its share of the shortfall opens a frame like any
  other. Surplus with nowhere to go stays where it stands, complete:
  nothing marks it and nothing scraps it. `Assigned` carries the
  placements, the sends, the openings and the cancellations.
  It is reached in three stages so the preview can stop after the first.
  `Fulfilment::assign` is the assignment alone, a `ShortfallAssignment`
  of what the reserve places, what units move over which `Route`, and
  what is still short per posting; it touches no schedule.
  `Fulfilment::schedule`, which the step alone runs, solves one send per
  route and answers a `SendSchedules` of the sends that depart and the
  routes that stay. `Fulfilment::settle` turns the two into the
  `Assigned`, and is the one place openings and cancellations are
  decided: what is leaving a post is read off the assignment's routes
  less the ones that stay, never off the sends, so the preview can pass
  `SendSchedules::nothing_held_back` and reach the same code without a
  solve. A `Cancellation` carries the `Posting` its frame stands at, the
  frame and its progress, so the seat and the row are read off the
  posting and nothing repeats it. The step's result is unchanged by the split, which
  the replay and rollback tests and the harness hold.
- **Extraction** groups extract weapons by the asteroid their entity
  stands at. A weapon names one material, so an asteroid's cap for a
  material is split among the extractors of that material alone and an
  extractor of another material is not in that split: each takes its
  rate, the cap is split equally among them when the sum exceeds it, and
  unused shares redistribute until none is left or the cap is met.
  `Income::extracted` reads the tick's extraction off the snapshot into
  a `BTreeMap<(AsteroidId, SeatId), Materials>`, so asteroid then seat
  order is the key's and no code sorts it, and `Income::apply` credits
  every seat and every asteroid from it. Nothing else reads an `Income`
  or takes one apart.
- **Construction** works one seat's asteroids in turn over one copy of its
  stockpile. At each asteroid it assigns the builders' combined rate evenly
  across that seat's frames there, computes the per-material ratio of
  stock to demand, scales each frame's spend by the smallest ratio among
  the materials it uses, never above the work it has left, and records
  completions. What the frames cannot use goes to repairing the seat's
  damaged entities there, which costs nothing. A builder's reach is the
  asteroid it stands at, which is the zone by another name, since holding
  keeps everything homed at an asteroid inside that asteroid's zone.
  Completion spawns at the post: a structure `Fixed`, a unit
  `Steered` at the asteroid's body for the tick it first exists in, offset one
  spacing along the asteroid's radial direction per unit already standing
  there. `step::spawn_body` is that placement, beside the completion it
  serves rather than inside the rule that steers afterwards.
- **Fire** collects every ready damage weapon of an entity that is not
  flying, sorts by `(Moment, EntityId, weapon)`, and resolves each in
  order against the snapshot plus an `Assigned` of the damage dealt so
  far this tick, skipping targets whose assigned damage is lethal.
  Target choice is `state::Threat`, DESIGN.md's threat rule, over the
  enemies standing at the shooter's own asteroid inside the weapon's
  range; range is the only gate, since everything is visible. Holding
  asks the same type, so a ship goes where it shoots. Standing at one
  asteroid is the zone by another name, as no two zones overlap, and a
  flying entity stands nowhere and so is neither shooter nor target.
  Damage is the weapon's damage cut by its falloff over the range, less
  the target's plating, floored at zero. A weapon that fires is next
  ready one interval on, never before this tick; a weapon with nothing
  to shoot keeps the moment it has, so it fires the instant a target
  arrives. `Shots` is the list of hits and the new ready moments.
- **The sweep** is the tick's one spatial index: entities sorted by their
  belt-plane `x` then their id, with range queries by window. `within`
  yields its window in that order and allocates nothing, so a sum over it
  is deterministic without a sort of its own; a caller that needs another
  order sorts for itself. `State::sweep` builds it and `step` hands it to
  Fire and to Holding, whose fields, chase and separation are the other
  range queries; nothing scans every entity against every other.
- **Deaths, reaping, elimination** live in `State::next`: entities at or
  below zero HP are removed, their `Ready` entries with them; a seat with
  no entities and an empty reserve
  is dead, its posts and frames removed. A composition exists only while
  `wants` holds it, and `Wants` drops a row set to zero, so the existence
  rule needs no reaping of its own.

## Sim: what leaves the sim

```rust
pub struct View {
    seat: SeatId, tick: Tick,            // the step count, for the draft's windows
    time: Time, length: Time,            // the match's own time and how long it runs
    gravity: Gravity,
    draft: Draft,                        // the sim's own, cloned whole
    stockpile: Stockpile,
    income: Materials, spend: Materials,  // the viewer's last whole second
    reserve: BTreeMap<RowId, u32>,
    compositions: BTreeMap<Post, Composition>,   // every seat's holdings
    plans: BTreeMap<Posting, Plan>,      // the viewer's own wants and frames
    present: Vec<Present>,               // every entity of the match
    teams: Box<[TeamId]>,                // the seating, indexed by seat
    exchanges: Vec<Exchange>,            // the tick's fire, per asteroid and seat
    terrain: Vec<Terrain>,               // every asteroid: orbit, caps, radius, pull
    zone: f64,
    still_in: bool,                      // the viewer's own seat is not out
    standings: Standings,
}
pub struct Present { id: EntityId, row: RowId, seat: SeatId, body: Body,
                     hp: f64, home: AsteroidId, at: Berth }
pub enum Berth { Standing(AsteroidId), Flying { from: AsteroidId } }
pub struct Composition { builder: bool, rows: BTreeMap<RowId, Held> }
pub struct Plan { want: u32, building: Option<Building> }
pub struct Building { progress: f64, starved_of: Option<Material> }
pub struct Held { present: u32, surplus: u32, leaving: u32, arriving: u32 }

impl View {
    pub fn of(state: &State, seat: SeatId, shots: &Shots) -> View;
    pub fn team_of(&self, seat: SeatId) -> Option<TeamId>;
    pub fn is_enemy(&self, seat: SeatId) -> bool;   // its team differs from the viewer's
    pub fn plan_of(&self, posting: Posting) -> Option<&Plan>;
    pub fn want_of(&self, posting: Posting) -> u32;
}
```

- Everything is visible (DESIGN.md, Visibility), so a view is the whole
  state projected for one seat, built once per tick from that tick's
  shots. Wants and frames are the one exception, and the type says so: a
  composition exists for every seat that holds or moves anything at a
  asteroid, and only the viewer's own wants and frames leave the sim, as
  `plans`. The reserve and the stockpile are the viewer's too, and so are
  the `income` and the `spend` beside it: a seat's own extraction and its
  own frames. An asteroid's `pull` stands beside its caps in `Terrain`, every
  seat's extraction there summed, since an asteroid and its caps are visible to
  all. All three are the last completed second's totals.
- The `draft` is the sim's own, cloned whole, since the stages, the one
  running and the asteroids taken are visible to all (DESIGN.md, Start). The
  panel reads `Draft::stages` for the order with each stage's seat, row and
  asteroid, `Draft::running` for the stage now running and `Draft::began` for
  the tick it began; a stage before the running one with no asteroid is one
  that ran out. A bot asks `Draft::running` whether it is picking,
  `Draft::took` what is spoken for, `Draft::placements` what it has placed
  itself, and `Draft::ended` when the clock started.
- It derives nothing itself; every fact is asked of the type that owns
  it — `State::holdings` for what each seat holds at each asteroid,
  `Frame::fraction` and `Frame::starved_material` for a frame,
  `Shots::exchanges` for the tick's fire, `State::standings` for the
  standings.
- A view carries the seating's teams, so a side is a fact of the view and
  not a guess from the seat: `is_enemy` is the one question an agent asks
  about another seat, and a teammate is never one.
- `Berth` is where an entity is and `home` is where it belongs, which for
  a flier is where it is going. A unit whose send is still forming stands
  at its source and is homed at its destination, so `Standing(asteroid)` with
  a `home` elsewhere is exactly a unit leaving that asteroid, and no flag says
  whether an entity is in the air.
- `Held` counts one posting: `present` stands there
  and belongs there, `leaving` stands there in a send that has not
  departed, `arriving` is homed there and in a send, and `surplus` is
  what the surplus rule would send away, `State::surplus_at` counted,
  which is the same query fulfilment takes the units from, so the wheel's
  hollow dot and the send the sim makes can never disagree. A surplus is
  read off the want, and a want is the viewer's own, so `State::holdings`
  never counts one and the view fills it for the viewer's own postings
  alone: it is asked once, for the one seat that may see it. A ship in flight is
  counted only at the asteroid it flies to. `State::count`, which the
  shortfall rule reads, is what an asteroid is homed by, present and arriving
  together.
- A view is what is, and nothing about the pointer: `View::of` takes the
  state, the seat and the tick's shots and no more. What a hover would do
  is asked of the state directly, by the one owner of the pointer
  (Game: the display library), so no fact about the pointer is stored and
  answered a tick late.
- `still_in` is the viewer's own seat, still in the match or out
  (DESIGN.md, Session). A viewer that is out draws no sector of its own
  and so no button, which is why `Rejected::DeadSeat` cannot reach the
  display.
- A post builds one frame of a row at a time (DESIGN.md, Compositions),
  so a `Plan` carries at most one `Building`.
- The roster is match-constant and travels with the initial state, so a
  client holds it from the session rather than from a view.
- `State::hash() -> u64`: FNV-1a over `Hash` of the whole state. The hasher
  widens every `usize` to `u64` and writes integers little-endian, so a
  `Vec` length prefix hashes the same on wasm32 and native.
- `State::standings() -> Standings`: per team, the asteroids where it
  has a structure, the cost total of its living entities, and whether
  any of its seats is still in. `over()` says the clock has run out and
  `leaders()` applies DESIGN.md's tie-break, comparing the state's match
  time against its length, so the end is a query. `Standings::new`
  builds one from those parts, which is what a hand-built view in `look`
  and in tests needs.

## Game: the display library

```rust
pub struct Scene {
    asteroids: Vec<AsteroidView>,          // position, radius, caps
    entities: Vec<EntityView>,     // position, glyph, seat, weapon range
    wheels: Vec<WheelView>,        // one per asteroid that carries a wheel
    flights: Vec<FlightLine>,      // every flying ship, and a held drag
    stockpile_bar: Option<StockpileBarView>,
    zone: f64,                     // the zone radius every asteroid draws
    seat: SeatId,                  // whose view this is
    selection: Option<AsteroidId>,
    gesture: Option<WheelGesture>, // a wheel button or a Sending
}
pub struct FlightLine { from: Vec3, to: AsteroidId, previewed: bool }
pub enum WheelGesture { Button(ButtonAt, Preview), Send(Sending, Preview) }
pub struct ButtonAt { posting: Posting, button: WheelButton }
pub enum BarMark { Cost(Materials), Refund(Materials) }
pub struct WheelView { asteroid: AsteroidId, sectors: Vec<SectorView> }
pub struct SectorView { seat: SeatId, rows: Vec<RowView>, arc: Option<Arc> }
pub struct RowView { row: RowId, entries: Vec<Shown> }
pub struct Shown { entry: Entry, previewed: bool }
pub enum Entry {
    Present(u32),
    Leaving { count: u32, to: AsteroidId },
    Building(Building),
    Arriving { count: u32, from: AsteroidId },
    Wanted { count: u32, dashed: bool },
    Placed,                          // a draft placement, until the clock starts
}
pub enum Fill { Solid, Hollow, Filling(f32), Dashed }
pub enum WheelButton { Plus(u32), Minus(u32) }
pub struct Client<'a> { selection, pointed, gesture, fights: &'a Fights }

impl Scene {
    pub fn from_view(view: &View, roster: &Roster, client: Client<'_>) -> Scene;
    pub fn of_belt(asteroids: &[Asteroid], gravity: Gravity, tick: Tick) -> Scene;
    pub fn centre(&self) -> Vec3;
    pub fn wheel_of(&self, asteroid: AsteroidId) -> Option<&WheelView>;
}

impl Entry {
    pub fn fill(self) -> Fill;
    pub fn count(self) -> Option<u32>;   // None for Building: a frame has no count
    pub fn phrase(self, name: &str) -> String;
}

impl ButtonAt {
    pub fn edit(self, want: u32) -> Command;   // the click's own want
}
```

- `of_belt` is what the lobby and loading screens draw before a match
  exists to have a view of; `look` builds scenes by hand. `centre` is the
  middle of the asteroids, which a camera frames the map from.
- A scene carries a wheel for every asteroid a seat holds a composition
  at, and for the selection and the asteroid under the pointer, whose
  own sector stands empty until it wants something: without it no first
  want could be placed, and a bare asteroid could not grow before its
  click. A sector holds only the rows that have an entry; another seat's
  sector never carries a wanted or a building entry, since wants and
  frames are its own. The one exception is the draft: while it runs,
  every placed stage of `View::draft` stands on its asteroid as
  `Entry::Placed` in the placing seat's sector, in place of the viewer's
  own wanted entry for that row, and a small wheel shows that line, the
  one wanted line it ever shows, so a taken asteroid reads as taken from
  the belt.
- `Entry` is one fact about a row at an asteroid, and its variants are what
  DISPLAY.md's states are. `Shown::previewed` is what the pointer says
  would change, drawn at half alpha; `Entry::dim` is what is dim by its
  own state, so a preview and a state cannot be confused.
- `Client` is what the client, not the sim, decides about a frame.
- `fights::Fights`: the fight memory, kept by the client because no field
  of the state records a shot. `observe(&View)` once per tick starts an arc
  where the view reports shots exchanged, drains it as the seat's HP at the
  asteroid falls, trails the last second and a half of damage, and forgets an
  arc ten seconds after the last shot; `arcs()` is what the scene draws.
- A `WheelGesture` is what the pointer rests on and the sim's answer
  about it, carried together: a `Preview` sits inside each variant, so
  the reading each gesture gets is the one its variant can give. The
  `Send` variant's preview builds the destination's arriving entries, the
  wanted line beside them and one flight line per `Route` it sends
  along; the `Button` variant's builds the stockpile bar's `BarMark` and
  nothing on a wheel, since a hovered button shows only its signed step
  (DISPLAY.md, Editing). A preview that sends nothing has no entry and no
  line to build, so no code asks whether one is empty and no empty
  preview stands in for a missing one; and `Play` starts no send drag
  from an asteroid standing none of the seat's units, so a drag that
  would move nothing does not exist either.
- `send::Sending { from, to, count }`: the drag between two asteroids' wheels.
  `rows` is the `Moving { row, count }` list it takes, cheapest first,
  off the units standing at the source, and `commands` is the two count
  edits per row it moves. One type, so the entries the scene dims and the
  edits the release issues cannot disagree.
- `glyph::Glyph { frame, marks, size }`: `Glyph::of(&Row)` by DISPLAY.md's
  three rules, a pure function with a test per rule. `glyph::HALF` is a
  glyph's nominal half-width, which both layers size by, and
  `LONG_RANGE_FROM_METERS` is the range at which a damage weapon's mark
  becomes a bar instead of a dot.
- `tint::toward(base, caps, strength)`: an asteroid's colour from its caps, so
  a region reads as one hue. `belt` paints an asteroid's mesh with it; the
  zone circle on the HUD is one ink for every asteroid.
- `icon::Icon`: one material's icon, the closed rings of its SVG in the
  glyph's sixty-unit cell, parsed by `usvg` (no text, no fonts) from the
  one sheet `game/icons/materials.svg`, `Icon::parse(sheet, group)`
  reading the group with the material's id, into a `LazyLock` the first
  time any glyph is drawn, which is the mesh catalog at boot; a group
  that is missing or is not one closed filled path is a boot-time panic
  naming the material.
  `icon::of(material)` is the drawing and `placed(centre, width)` is it
  as a `Primitive::Path`, so the glyph's Extract mark, the stockpile's
  cells and the asteroid bars draw it through the same painter and
  rasteriser as the dot, the line and the ring. A path is filled
  even-odd on both layers: the rasteriser counts crossings, and
  `stencil::Cell` fills it as a mesh of trapezoids, one per band between
  vertex heights, so a nut's hole is a hole.
- `stockpile_bar::StockpileBar`: the stockpile and the clock, laid out
  across the top centre from `scene::StockpileBarView` (the seat's
  stockpile, income, spend, elapsed tick, clock and the `BarMark` a
  hovered button leaves), in the wheel's vocabulary: one box in the
  screens' scrim and line around every cell, the cells the wheel's cell
  gap apart, a cell running icon, stock numeral, bar, net numeral; the
  count font for every numeral, the bar a box of the count line's
  height outlined in the panel's line ink, filled to the stock in the
  hue faded toward the backdrop, the spend segment darker inside the
  tip and the income segment fainter past it, the overrun the fill
  itself running past the box's end when the stock is at capacity. The
  `BarMark` takes the place of one of those two segments and no more:
  a cost replaces the spend projection inside the tip, a refund the
  income projection past it, so the two readings never stack and the
  same two lengths mean one thing at a time. The
  clock's fill is the panel's dim ink so the elapsed numeral reads over
  it. `speaks_at` is the cell under the pointer, whose one phrase is the
  capacity.
- `bars::Bars`: every asteroid's resource bars, the mirror of its wheel:
  a row per material with a cap, at the wheel strip's height, the icon
  at the row's right end nearest the asteroid and the bar growing
  leftward from it, the rows' right ends on the wheel's arc mirrored,
  the longest band a wheel strip's width, and along the mirrored arc one
  spine per asteroid in the panel's dim ink, drawn by the wheel's own
  `spine_points` with `Side::Left`. The cap is a band of the count
  line's height in the hue lerped toward the backdrop by `CAP_ALPHA`,
  the pull the full hue laid over it from the right end; no scrim, no
  outline. One drawing serves both states: small and faint at rest, full
  and whole under the pointer or the selection, never a different shape,
  every colour faded by the `Placed` alpha. `Bars::over` takes the
  frame's `Placed` list and rests every asteroid without one at the
  small scale and the resting alpha, which is what `at_rest` gives the
  lobby, loading and results screens.
- `stencil::Stencil`: one glyph painted on the HUD — the frame in its fill
  state, its marks, the `alpha` it is drawn at, and the belt a starved
  frame carries in its material's `hue`. Every HUD glyph goes through it,
  so a glyph is drawn one way, and `DIM_ALPHA` is what a dimmed one takes.
- `glyph_quad::GlyphQuad`: a mesh value per `(Glyph, SeatId)`, its own
  texture rasterized in the seat's colour, for the belt. One billboarded
  quad per ship, one draw each; hollow, filling and dashed glyphs exist
  only on wheels, so the catalog holds solid cells alone.
- `wheel::Wheel`: one asteroid's wheel, laid out in screen points. Sectors
  stack down the asteroid's right in seat order, each as tall as its own
  sections, or one bar where it fights with nothing standing; within a
  sector a row stands as one upright section whose inner edge is on the
  wheel's arc, its glyph at the left and its lines as cells in one row
  after it, one per `Mark` — here, moving, wanted — each the count of
  the entries behind it and the mark that says what it counts, a cell
  as wide as its digits. Sections are laid out by cost, structures above
  units, `STRIPS_PER_COLUMN` to a column and the overflow in the next
  column beside it, each column as wide as the most its strips can grow
  to: every count one digit wider, and the signed step a hovered button
  shows, so nothing a strip gains runs under the next column. Every
  rectangle is computed once, so `paint`, `button_at` and
  `spoken_at` read the same geometry and cannot drift. `WheelButton::edit`
  is the `Command` a click issues and `WheelButton::wanted` is the count
  it would leave, which is the count the view asked the sim about, so it
  no longer clamps at `MAX_WANT`: past the cap is the sim's refusal to
  make. `WheelButton::delta` is the signed step a
  hovered button shows beside its strip.
- `wheel::Detail`: a wheel is drawn `Full` where the pointer or the
  selection rests on it and `Small` everywhere else, two fixed scales and
  two fixed slot counts, never a size that follows the crowd. A small
  wheel carries a section only for the rows standing or moving there and
  never a wanted slot, but for a draft placement's `Entry::Placed` line.
  `Buttons { step, wants, refusals }` is passed only to a
  full wheel, so a small wheel takes no input by construction, and the
  buttons stand two to a section, between its glyph and its cells,
  plus over minus. `ButtonRefusals { adding, removing }` is what the sim
  says about one row's two buttons, one `State::admits_want` each with
  the count that button would issue. `Sizing { detail,
  scale }` is how a wheel is drawn this frame: the detail decides what
  it shows and the scale, eased toward the detail's own, how large.
- `wheel::Footprint`: the rectangle an asteroid's wheel would take at each
  `Detail`, from the view alone, so the frame can lay wheels out and
  decide what the pointer is over before any wheel is built.
- `wheels::Wheels::over(scene, roster, viewport, aim, ease)`: the
  frame's layout and the frame's hit test in one pass. `Aim { viewer,
  pointer, hovered, step, view, state }` is what the pointer and the
  keyboard say, over the two the display reads: `View::want_of` for what
  a row is wanted at, `State::admits_want` for what each of its buttons
  would take. `Aim` holds no map and decides no rule of its own:
  `buttons_at` asks those two for the rows of the one full wheel it is
  building. A refused button is drawn spent,
  `Wheel::button_at` never returns it, so no click can edit it, and
  `spoken_at` answers `Spoken::Refused { why }` at the strip's right
  edge, where a live button shows its step. `label::refusal` turns a
  `Rejected` into that phrase: "Not yet" and "Asteroid taken" have one,
  and the rest are drawn spent without one, which is what
  `Rejected::TooMany` at the cap wants and what the four an asteroid's
  own wheel cannot reach get. `Ease` is where a wheel's
  scale and alpha are eased toward their targets over `Span::Fast`:
  `Motion`, a store the play screen owns and steps once per frame by the
  engine's own `dt`, never egui's clock, in which a wheel first drawn
  starts at rest so it grows rather than appears; `Still` in tests and
  in `look`. Size and alpha are two axes: a wheel is full where the
  pointer or the selection rests, so two may be full at once, and whole
  only where the pointer is or, with nothing hovered, at the selection;
  every other wheel is faint, a covered wheel no fainter; wheels are
  painted faintest first. Which wheel is hovered is decided against the
  footprints as they stood before any wheel grew, the smallest footprint
  under the pointer winning so a small wheel inside the full one's reach
  still takes the pointer, and a hovered wheel stays hovered until the
  pointer leaves its full footprint by `HOVER_MARGIN`, so growing under
  the pointer never changes what is hovered and an overshoot closes
  nothing. `at`, `button_at` and `spoken_at` are what a click, a drag and
  the hover phrase read; `spoken_at` answers with a `Spoken`, a wheel's
  row, an asteroid bar or a refused button, which composes its own phrase
  from the roster. `Wheels` owns the frame's `Bars` too, laid from the
  same eased `Placed` list, so an asteroid's bars grow and fade with its
  wheel.
- `ease`: `Span { Fast, Slow }`, the two spans every eased value on
  screen settles over and no other, `duration` 20 ms and 100 ms; `Fast`
  for what the pointer causes, a wheel's growth, hover and buttons, `Slow`
  for what the camera and the screens do, pan, zoom, refocus and the
  draft panel's coming and going. `toward(value, target, dt, span)`
  closes that span's share of a value's gap to its target in one frame.
- `screens::order::Order`: the draft's panel, DISPLAY.md's initiative
  list, the third panel inside a match, titled Draft.

```rust
pub struct Order { frame: Rect, title: Pos2, rows: Vec<Row>, alpha: f32 }
struct Row { rect, stage: Option<(SeatId, Glyph)>, name: String, standing: Standing }
pub enum Standing { Waiting, Running { left: f32 }, RanOut, Placed(AsteroidId) }

impl Order {
    pub fn over(window: Rect, draft: &Draft, tick: Tick, roster: &Roster,
                names: &[String], alpha: f32) -> Order;
    pub fn paint(&self, painter: &egui::Painter);
}
```

  One row per `Draft::stages` entry in order, packed without a gap
  under the title, at the screen's left below the stockpile bar, the panel as
  wide as its rows. A row's `Standing` is read off the draft alone:
  `Placed` where the stage carries an asteroid; `Running` for the stage at
  `Draft::running`'s position, `left` the share of `STAGE_SPAN` since
  `Draft::began` still to run; `RanOut` for an unplaced stage before it,
  or every unplaced stage once no stage runs; `Waiting` after it. Once
  no stage runs and the draft has not ended, a last row with no stage,
  named Clock, is `Running` with `left` the share of `GRACE` still to
  run. The bar is one length in every row: full while `Waiting`,
  draining while `Running`, empty once `RanOut`, and the asteroid's name in
  its place once `Placed`. The running row is whole and every other
  faint at `wheels::RESTING_ALPHA`; a stage's glyph is hollow in the
  seat's colour until placed and solid after, through `Stencil`, and
  carries the seat. `names` are the seats' names in seat order from
  `lobby::seat_names(&Seating, me, seed)`, which `Machine` keeps the
  `Seating` for: You, "Player N", or for a bot `lobby::bot_name(bot,
  seed, seat)`, one of `MAX_SLOTS` names the game crate holds per
  personality, indexed by the seed plus the seat, so two bots of one
  personality in one match never share a name; the lobby's Holder choice
  still names the personality. `Play` owns the panel's alpha, eased by
  `ease::toward` over `Span::Slow` toward one while the draft runs and zero once
  `Draft::ended`, and draws the panel while it is above zero.
- `camera::BeltCamera`: the focus point moving at the local orbital
  velocity, pan by meters or by pointer pixels, and zoom within a range
  stated against the belt's own scale. A pan, a zoom or a new focus sets
  the target; `settle(dt)`, once per frame, eases the shown focus and
  distance toward it, and the engine camera and the viewport read the
  shown values.
- `viewport::Viewport`: one frame's projection, the engine camera built
  once, with the window and the painter's own measure. Everything that
  projects a world point goes through it. An asteroid with no wheel is picked
  by screen-space distance to its projected centre against
  `wheel::PICK_RADIUS`, which is how an asteroid a seat holds nothing at is
  selected and its first want placed.
- Two draw modules, one per DISPLAY.md layer. `belt`: one function from a
  `Scene` and a `Viewport` to the engine's draws — asteroids, one light, and
  ships, in 3D. `hud`: one function from a `Scene`, a `Viewport` and an
  `egui::Painter` to what is painted over the belt's own projection —
  every asteroid's zone circle in one ink, every standing armed ship's range
  circle in its owner's colour, and the flight lines. The wheels and the
  bars are painted after it, through `Wheels::paint`, and the stockpile
  bar after them over an opaque backdrop, so nothing on the belt covers a
  wheel and nothing shows through it; `Wheels::clear_of(rect)`
  drops every section and every bar whose frame intersects the stockpile bar's
  box before painting or hit-testing, and `Controls::avoid(rect)` keeps
  a hover note out of it, so nothing of the HUD is drawn inside the
  box or half under its edge.
- The binary is the playable: a `Game` whose `tick` and `frame` are the
  live screen's of the `Flow` (Game: net and screens, below) and nothing
  else; in `Play`, the tick inserts the local controllers' stamped
  commands into the `Session`, advances it within the pacing rule, reads
  the tick's `View`, refreshes the preview of the hover it holds, and
  feeds the `Fights`. `Play` is the one owner of what the pointer rests
  on: its frame decides that from the wheels' own hit test, asks
  `State::preview` the moment the hover changes and once a tick while it
  is held, and hands the answer to `Scene` inside the `WheelGesture`, so
  the stockpile bar and the wheels read one preview and none is a tick
  behind the pointer. A paused or held match holds no hover and asks
  nothing. `Play`'s frame builds a
  `Scene` and draws it — `belt` then `hud`, so the HUD is never occluded
  — over one full-window transparent egui layer that claims no widgets,
  and every other screen is drawn on that same layer. Input is keyboard
  and mouse through the engine's action vocabularies, which
  `controls.rs` declares: a click on a wheel or an asteroid selects and
  focuses it, a button click edits one want and repeats after a third of a
  second and every tenth after that, Shift raises the step from one to
  five, a left drag from wheel to wheel is the send, the right or middle
  button and the pan keys drag the belt, the zoom axis zooms or, during
  a send, sets how many go, and Escape opens the pause screen and closes
  it again. The gamepad bindings DISPLAY.md states are a later unit. Its
  own headless drive, behind the `look` feature, plays a whole skirmish
  through the engine's offscreen `Session` — title to lobby to a
  placement through a wheel's button to the standings — picks a team out
  of an open list, removes a guest from a room it serves, and clicks a
  disabled Quit; it writes `game/look/title.png`,
  `title_quit_reason.png`, `lobby.png`, `lobby_choice.png`, `wheel.png`
  and `results.png`. Every test of it holds one lock, since each title
  serves a room on the protocol's one port. `check.sh` runs it under
  `xvfb-run`, so the drive and its screenshots are verified on every
  change.

## Look

`look` is a binary of `game`, behind the `look` feature, over the
engine's `offscreen` feature and its default `ui` feature so the HUD
lands in the pixels. It holds the three fixed scenes from DISPLAY.md as
code, and a fourth, a stockpile mid-match (income, spend, one material
at capacity, one stalling) beside an asteroid with mixed caps and an
extractor's glyph on its wheel, and a fifth, the draft mid-way: a real
two-seat skirmish session, its seed chosen so the host goes first, one
stage placed, one run out, one running half drained, one waiting, the
bare asteroid beside the taken one carrying the hollow wheel with a refused
button's reason. It renders each through a `Session`,
reads pixels back, and writes
PNGs under `game/look/`, which is `.gitignore`d: screenshots are the
judgement's input, never committed. `cargo run -p neumannarch-game --features
look --bin look` runs it; `check.sh` builds it, with `cargo build -p
neumannarch-game --features look --all-targets`, and runs the playable's own
drive under `xvfb-run`, so the default binary and the wasm build never
pull `image` or `offscreen`. It is the only way a display change is
verified.

## Agents

`agents` is the agent frontend, a library over `sim` and the `harness`
binary. `Agent::decide(&mut self, view: &View) -> Vec<Command>` is
called on `DECISION_INTERVAL`, at most `MAX_COMMANDS_PER_DECISION` per
call. `Seated { seat, agent }` builds the view and stamps the seat and
the sequence, so no caller does. `Scripted` is the shipped opponent: a
`Personality`'s constants read through `Survey` into a `Plan`, stepping
its `Commitments` and a `Dice`.

- `Survey` is one decision's tally of one view and derives everything it
  answers from that view alone, own and enemy alike: the asteroids a
  seat holds, occupies and builds at, its army, the enemy's army and the
  asteroids it holds, the threat at each asteroid, and the worst plating
  and range the enemy fields. It does not derive income: the view
  carries the seat's own, measured by the sim, and a plan reads
  `view.income` rather than a second answer to one fact. An enemy is a
  seat the view says is on another team, so a teammate is neither a
  threat nor a target. Nothing is remembered, since nothing is hidden
  (DESIGN.md, Visibility).
- `Commitments` is what the agent has decided and the view cannot say: the
  asteroids it has claimed and how long it will wait for each, the asteroids a
  lapsed claim bars for a while, and the asteroid it has committed an attack
  to. `settle(&View, &Roster)` retires a claim the seat has taken and
  bars one it gave up on.
- `Plan` is a target composition per asteroid, diffed against the view into
  `Want`s, in priority order: the opening, defence, economy, then army.
  An attack commits to the nearest enemy asteroid once the army it can see it
  needs is standing.
- `Plan::draft` is the whole of a bot's opening; there is no rule by
  seat index any more. While its stage runs it asks for one of that
  stage's row, and only that; off its stage it asks for nothing, and
  while the draft runs it re-asserts the placements the draft records
  for it so its own standing want is never dropped. The asteroid is the
  free asteroid with the greatest `fit × away / (1 + near / REACH)`,
  ties by lowest asteroid id, where `fit` is the asteroid's caps
  weighted by the shares of `Plan::intended`, the same cost mix the
  extractor rule spends at; `away` is `gap / (gap + REACH)` over the
  distance to the nearest asteroid an enemy seat has taken, one where
  none has; and `near` is the distance to the nearest asteroid this seat
  has taken, zero where it holds none. So a bot takes an asteroid rich
  in what it means to build, away from its enemies and beside its own.
  Both the enemies' asteroids and its own are read off `View::draft`,
  since nothing stands during the draft for the survey to see. A seat's
  second pick weighs what its first lacks: `Plan::wanted_shares` is
  `mix[m]/Σmix × (1 - held[m]/Σheld)` per material, where `held` is the
  caps of the asteroids the seat has already drafted, so the more of a
  material those asteroids supply the less it weighs and a material they
  have none of keeps its whole share of the mix, and `fit` is the
  asteroid's caps weighted by those shares. No share is ever negative,
  so the rule carries no case of its own and the pair covers the mix
  between them. A mix of one material that the first asteroid already
  supplies scores every free asteroid at zero, which the tie-break by
  lowest asteroid id settles.
- The asteroid a bot's constructor drafts is its first expansion:
  `Plan::draft` claims it in `Commitments` from the tick it lands, so
  `expand` keeps the constructor there instead of counting it spare and
  pulling it home, and the claim retires itself the moment a structure
  stands there. The asteroid is in the survey's `developed` from the
  start, since a builder stands at it, so the extractor rule wants
  extractors there by income and the yards rule wants a yard there, in
  that order, and the constructor builds them. `Commitments` keeps its
  claims and bars in match time, not in steps. `Seated::issue` breaks
  its own decision cadence while the bot's stage runs, so a bot places
  on the first tick it sees its stage running rather than waiting out
  the cadence; DESIGN.md says it places on the stage's first tick, and a
  stage begins inside a step, so the first tick a view can show it is
  the tick after.
- `economy` runs in one order and the order is the rule: the structures
  already standing, `expand`'s masons, the extractors, then the yards, then
  the stores. Income comes before the buildings it pays for, so a second
  yard cannot eat the energy an extractor needs and stall the seat with a
  frame it can never fill; the stores, the least urgent spend, are charged
  last. The demand is therefore matched to the builders that stand and the
  masons the plan commits, not to the yards it is about to want.
- The economy sizes extraction by matching income to build capacity, all
  of it read off the plan's own targets, the personality and the roster:

```rust
fn counted(&Survey, f64, &[(RowId, f64)]) -> Vec<(RowId, u32)>;
fn widest(&BTreeMap<AsteroidId, f64>) -> Option<AsteroidId>;
impl Plan {
    fn wanted(&self, &Survey) -> impl Iterator<Item = (&Row, u32)>;
    fn building_rate(&self, &Survey) -> f64;               // per second
    fn intended(&self, &Survey, &Personality) -> Materials; // a cost mix
    fn demand(&self, &Survey, &Personality) -> Materials;   // per second
    fn extractors(&mut self, &Survey, &Personality);
}
```

  `wanted` is every row the plan has a target for, at every asteroid, with
  its count; there is no second tally of what the bot will have.
  `building_rate` is the combined Build rate of those rows. That rate is
  the sim's own ceiling on spend, since build is flow at the builders'
  rate, so the economy grows with the base rather than with an appetite
  no extractor could meet. `intended` is the cost mix that rate will be
  spent on: those same rows, plus the army rows at `Personality::weights`
  sized by `counted`, which is also what `force` asks for. `demand` is
  that mix normalised to its own total and multiplied by the rate, so it
  is a `Materials` per second whose total is the build rate.
- Shortfall per material is `demand` less `View.income`, floored at zero.
  Income is measured by the sim; the agent derives none of its own.
- Spare at an asteroid for a material is its cap less what every other seat
  pulls there and less what this seat already pulls:
  `caps[m] - (pull[m] - mine).max(0.0) - mine`, where `mine` is this
  seat's standing extractors of that material at that asteroid at their rate,
  capped by the cap. A seat's own pull is not lost headroom for a count
  that includes the extractors making it, and subtracting only what
  others take says so without over-counting in the seconds before `pull`
  has caught up with an asteroid's newest extractors. Each placement lowers
  the spare, so it bounds the whole count at the asteroid and not the
  placements alone.
- Payback: an extractor is placed only where `min(rate, spare)` times the
  smaller of the clock's remaining seconds and `PAYBACK_HORIZON` covers
  its cost's total, so a bot near the clock builds nothing that cannot
  repay before the match ends.
- Placement walks the materials in the roster's order while a material
  is short, taking the developed asteroid with the greatest spare, ties
  by lowest asteroid id, and stops when no asteroid has spare left or
  the placement cannot repay. Each placement lowers the shortfall by its
  yield. The want is what stands at the asteroid plus the placements,
  through `Plan::want` like every other, so `affordable` charges it
  against the budget; a material with nothing short still wants what
  stands, since nothing complete is scrapped.
- The two personalities differ in this rule only through `demand`: the
  yards, masons and stores they want and the army value
  `Personality::army_value` gives them. Nothing here reads the dice.
- `Roles` reads the roster once into the row an agent prefers per job, so
  no agent names a row by id. Extraction is a job per material, not one
  job: `Roles::extractors` is the best-rated row that pulls each
  material, in the roster's own material order, which is the order the
  economy places them in.
- `Personality::of(protocol::Bot)` is the one place a lobby's bot becomes
  constants, and it is exhaustive, so a bot the protocol can name always
  has a way of playing. A bot in a lobby is a `Seated` agent run by the
  machine that owns its seat, through `game`'s bot controller; the sim
  never knows.

`harness`, native only, no feature gate: `match` seats agents and plays
one to the clock, tracing standings as it goes; `replay` checks the
record reproduces the live hash and that the same match built twice from
independent initial states hashes the same; `rollback` inserts every
command late and scrambled and checks the settled hashes match the
on-time match; `matrix` plays named compositions pairwise from symmetric
starts and prints a table of asteroids and the tie-break's verdict; `draft`
plays one personality against itself over `MIRRORS` seeds and prints, per
seed, which team's window opened first, the asteroids each seat drafted with
their caps, and who won, then how often the first picker won, so the pick
order's weight is a number and not a hunch; `sweep`
sets the holding rule's constants, playing two forces at one asteroid under
roster variants and printing, per variant, how long the force takes to
close to weapon range, how far it spreads while closing, how long the fight
takes to decide, what the winner has left, and whether swapping the two
sides swaps the winner. Doubling
the tick rate and asserting the same outcome is a future check threading
through flights, weapon intervals and the holding rule's gains.

## Protocol

`protocol` is every value two machines exchange, serialisable, with no
io and no engine, so `game`, `server` and the harness's record files all
carry it directly.

```rust
pub struct PlayerId(pub u32);                   // PlayerId::HOST opens a lobby
pub enum Bot { Turtle, Expand }                 // agents turns it into a Personality
pub enum Holder {
    Open, Closed, Player { player: PlayerId, ready: bool }, Bot(Bot),
}
pub struct SeatSlot { pub team: TeamId, pub holder: Holder }
pub struct Lobby {                              // MAX_SLOTS == MAX_SEATS slots
    slots: Vec<SeatSlot>, seed: u64, clock: Tick, host: PlayerId,
}
pub enum LobbyEdit {
    SetSlot { slot, holder }, Kick(PlayerId), SetTeam { slot, team },
    SetSeed(u64), SetClock(Tick), SetReady { ready },
}
pub enum Refused {                              // why an edit was not applied
    NotHost, NotYours, NotSeated, NoSuchSlot, BadTeam, BadClock,
    AlreadySeated, NotAGuest, HeldByAGuest,
}
pub enum NotReady {
    NoSeats, OpenSeat { slot }, Unready { slot }, HostUnseated,
}
pub enum Occupant { Player(PlayerId), Bot(Bot) }  // Holder minus Open and Closed
pub struct Seating { holders: Vec<Occupant>, host: PlayerId }
pub struct Started { setup: Setup, seating: Seating }  // paired by Started::new
pub struct Crew { player: PlayerId, watched: SeatId, seats: Vec<SeatId> }
pub enum Request {                              // a machine's request of a room
    Join { version: u32 }, Edit(LobbyEdit), Start, Rematch, Leave,
}
pub enum Notice {                               // what a room reports to its screen
    Welcome { player, lobby }, Lobby(Lobby), Started(Started),
    Refused(Refused), NotReady(NotReady), Full, Version, Left, Removed,
}
pub enum Relayed {                              // match traffic, either way
    Command(Stamped), Acknowledge { seat, up_to: Tick }, Hash { tick, hash },
    Desync { tick },
}
pub enum Message { Request(Request), Notice(Notice), Relayed(Relayed) }  // the wire tag
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
    pub fn admit(&mut self, player: PlayerId) -> bool;
    pub fn release(&mut self, who: PlayerId) -> bool;
    pub fn next_seed(&self) -> u64;
}
impl Seating {                                  // who holds and who runs a seat
    pub fn owner(&self, seat: SeatId) -> Option<PlayerId>;
    pub fn seat(&self, player: PlayerId) -> Option<SeatId>;
    pub fn seats(&self) -> impl Iterator<Item = (SeatId, Occupant)>;
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
pub trait Codec { fn encoded(&self) -> Vec<u8>; fn decode(&[u8]) -> Result<Self, Malformed>; }
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
`Holder::Player`, where it means something; a bot and a closed seat have
none to hold. `Bot` names a shipped opponent and `agents`'
`Personality::of` matches it exhaustively, so a bot the protocol can name
always has a way of playing.

Every rule a holder is enabled by is a function here, evaluated on a
copy: `Lobby::edit` for a holder, a team, a kick, the seed, the clock and
readiness, and `Lobby::freeze` for the start. A player holds one slot at
most (`Refused::AlreadySeated`), only a guest's slot is kicked
(`Refused::NotAGuest`), and a guest's slot is opened by that kick alone
(`Refused::HeldByAGuest`), so the screen offering those values and the
room applying them cannot disagree. A lobby freezes only where the host
runs a seat of it (`NotReady::HostUnseated`): every machine in a started
match holds a `Crew`, which is what makes building one unrefusable. A
frozen `Seating`'s `Occupant` holds no `Open` variant: a slot that is
still open refuses the freeze first, so the type a `Seating` holds never
represents a seat with nobody in it.

Each way a room answers what it did not do is its own `Notice` variant,
never one shared reason: `Refused` for a lobby edit that was not the
sender's, `NotReady` for a start of a lobby that is not a match yet,
`Full` for a join of a room with nowhere to sit, and `Version` for a
join carrying a version that is not `VERSION`, which the room checks
before it seats anything. `Notice::Removed` reaches a kicked machine,
distinct from `Notice::Left`, which a host's own departure broadcasts
instead — the room's own notice for each, never the `Request::Leave` a
machine sends to report its own departure, since the three are
different facts on the title's join field.
`Request::Rematch` is the host's request that a room open its lobby
again, which the room answers with a `Notice::Lobby`. `Relayed::Hash`
means two things by direction: a machine reports its own hash at a
settled tick, and the room returns the same hash once every machine
reported it there, which is what marks a tick agreed on every machine.

`Codec` is the one encoding, blanket-implemented for every serialisable
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
  `Machine::tick` is the lockstep loop — receive, pace, issue, step,
  relay — and takes the transport as an argument, so nothing about it
  needs a screen or the engine and two of them run in one process under
  test. `Machine::of` takes a `Started` and a `Crew` of its seating and
  refuses nothing: the crew names seats the setup holds, so the session
  cannot turn it down, and the seat the display follows is the crew's
  watched one. The seats a machine runs and how many peers it has both
  come from that seating, so a host running a bot from a closed slot of
  its own is a peer like any other and its guests wait for the first
  agreed hash.
- **Transport.** `Transport` is a trait of four calls — `send`,
  `acknowledge`, `report`, `received` — carrying stamped commands,
  acknowledgements and hash reports both ways, as `protocol::Relayed`.
  `Local` returns nothing and exists so the loop has one shape.
  `Connection` is the room this machine has joined and its own transport
  in one: it sends `protocol::Request` and `Relayed` to the room over
  one `WebSocket` per target — a runtime on a thread of its own on the
  desktop, the page's own WebSocket in the browser — and reads `Notice`
  and `Relayed` back. Each inbound frame is decoded once, off the wire's
  tagged `Message`, into the inbox its type names, so a room's notice
  and a match's traffic are never read by the wrong side; a screen calls
  `notices`, a match calls `received`, and neither drains the other's.
  `Flow` owns the connection, not the match, so it outlives the match
  played over it; a machine takes it as `&mut dyn Transport` and the
  choice is one runtime value. Received commands are inserted into the
  session, which rewinds as needed; the loop reads `Rewound` only to
  reset client-side memories (fights, hover) that may now be stale.
  Every match calls all four, so the loop has one shape whether or not
  it has peers, and it matches every `Relayed` variant by name, catching
  none away.
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
  tick is an opportunity to step, and the pace decides whether to take
  it. A seat whose machine has left the room is acknowledged for the
  rest of the match by the room on its behalf, so the others settle
  every tick and the match goes on without it. Every one of these
  numbers is a constant with its unit in its name.
- **Screens.** One `Flow` owns one `Screen`, which is the screen showing
  and the room behind it: `Title { listener, join }`, and `Lobby`,
  `Loading`, `Play` and `Results`, each with its screen and its `Room`.
  `Pause` is the match's own, since play holds whether it is open. A
  screen's frame answers the next screen out of its own parts, so every
  transition is one method named for what it does — `opens`, `starts`,
  `plays`, `ends`, `rematches`, `leaves`, `removed` — and no screen
  reaches into another. The title alone names the flow's connection to
  open: `Connect::Host` and `Connect::Join(address)`, which no screen
  opens itself. The title's own screen outlives every `Screen`, since
  DISPLAY.md keeps the address it holds.
- **The room.** `Room` is who owns the lobby a match is set up in and
  what carries that match: `Local`, where every seat is on this machine
  and nothing goes on the wire; `Guest`, a room another machine serves,
  holding the id its welcome gave; and `Host`, a room this machine
  serves and plays in. It answers `me`, the transport, and the lobby
  rule itself: `Local` applies an edit to its own copy, and a guest and
  a host alike send it and take the lobby the room broadcasts, so one
  rule runs on both paths. Skirmish opens a `Local` lobby; Host serves a
  room in this process and joins it at the loopback; Join opens a
  connection to a typed address, and either way the lobby is the one
  the welcome brings, so the title reports it is connecting until then.
  Start freezes the lobby into a `Started` — the room's own copy where
  there is a room, which broadcasts it — and every machine builds the
  initial state from it and reports the tick-zero hash; loading holds
  until the room reports every machine agreed it. A desync and a peer
  too far behind are the two states play holds in, both over the dimmed
  HUD, and the belt reads no input under either, nor under the pause
  screen: the gesture the pointer was in the middle of is dropped with
  the frame the hold begins. Results holds the score, the belt the
  match ended on and the lobby it came from, and returns to that lobby
  for a rematch or to the title. Results is reached when the view's
  standings say the clock has run out; every earlier tick carries them
  too, and no screen but this one reads them yet. Ending a match at an
  elimination is a later unit.
  The lobby draws the belt behind everything at `PREVIEW_ZOOM`, the
  widest view whose zone circles stand apart, and lays its seats out by team:
  one heading per team that holds a seat, and its seats under it, each a
  holder choice, a Kick beside a guest, a team choice and a readiness
  mark. A closed seat is not drawn, so a team holding none is not drawn
  either, except the one closed seat's row the host is left under the
  last team, which is how a seat and a team are opened.
- **The title's room.** The title screen holds the listener for the
  room Host would join, bound when it opens, and hands it to the room
  with the welcome. Host is enabled exactly where a listener is held,
  so a click on it cannot fail, and a title the player leaves for
  anything else drops the listener with the screen, which releases the
  port.
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
name to its sender. A machine enters a room by requesting to join, takes
the first open slot, and is welcomed with the id it holds and the lobby
as it stands; a room with nowhere to sit, or a join carrying another
version of the protocol, refuses it by name. A kick is the host's edit
like any other: the room opens that slot, notifies the machine that held
it, and drops it. The shape of a lobby is its host's, so a host that
leaves one ends it: the room reopens and every other machine is sent
away.

A room's phase owns the machines in it, so no second list of them can
disagree with what the room is doing: a room setting a match up holds
the machines that joined it, in the order they did, and a room
forwarding one holds the `Forwarding` itself, which knows the machines
its seating gives a seat to and which of them have left. The id the
next machine takes only rises for the life of the room, so a room that
reopens hands out no id it has held before.

A room keeps the lobby the match was set up in while it forwards that
match, so the host's `Request::Rematch` opens it again with its shape
kept, the slots of machines that have left standing open. Every machine
at the results follows the lobby the room broadcasts.

Once started, the room forwards every stamped command and acknowledgement
of a seat to every machine but the one that sent it, and drops one of a
seat its sender does not own. It collects hash reports per settled tick,
declares a desync to every machine when two reports differ there, and
sends back the hash where every machine reported the same, which is how a
machine knows a tick is agreed. It keeps the one record it forwarded —
the log every machine converges on — and holds it in memory when the
match ends; a per-machine record would need a machine to upload one,
which no request carries. It holds no tick clock and steps no sim.

`Room` is pure and knows nothing of sockets: it answers `Outbound`s
addressed by `Recipient` to the sender, to one named machine, to
everyone, or to everyone else, and `stream.rs` is the only part that
touches the network. `game` embeds it under the `host` feature so
host-by-address needs no separate process; the binary serves a room
list later.

## Module layout

```
sim/src/
  lib.rs            TICKS_PER_SECOND, TICK; the public surface
  belt.rs           Belt::fixed, the zone's four constants, State::start
  setup.rs          Setup, MAX_SEATS, BadSetup
  real.rs           Real
  vec3.rs           Vec3
  materials.rs      Material, Materials, Stockpile
  time.rs           Tick, Moment
  ids.rs            AsteroidId, EntityId, RowId, SeatId, TeamId
  post.rs           Post
  fixture.rs        the crate's one test world, behind cfg(test)
  roster/           mod.rs Roster and the movement limit; row.rs Row,
                    Weapon, Kind, Weights; shipped.rs the seven
  orbit/            body.rs Body, Gravity; elements.rs Orbit;
                    stumpff.rs; universal.rs; lambert.rs
  state/            mod.rs State, Index impls, queries; seat.rs; asteroid.rs;
                    entity.rs Entity, Motion, flying and standing;
                    schedule.rs Flight, Schedule, Burn, the solve;
                    send.rs Send, its forming window and the search for
                    its arrival tick;
                    wants.rs Wants; frame.rs Frame, its fraction and what
                    it went short of; ready.rs; command.rs Command,
                    Issued, Stamped, Batch, Sequence, Rejected, Refused,
                    apply; sweep.rs Sweep, the tick's spatial index;
                    threat.rs Threat, Aim, Assigned: the one target rule;
                    preview.rs Preview, ShortfallFilling, State::preview;
                    view.rs View, Present; hash.rs;
                    draft.rs Draft, Stage, STAGE_SPAN, GRACE,
                    STAGES_PER_SEAT;
                    standings.rs Standings
  step/             mod.rs step, next, spawn_body; propagation.rs;
                    fulfilment.rs; extraction.rs; construction.rs;
                    fire.rs Fire, Shots, Exchange;
                    holding/ mod.rs Holding and Thrusts; power.rs Power;
                    field.rs Standing, Fields, Sample, Fraction and the
                    kernel; terms.rs Steering, Place and the drift
  history/          mod.rs; snapshots.rs Retention and the ring behind it;
                    log.rs the stamped log by tick;
                    session.rs Session: advance, insert, acknowledge,
                    settled, hash_at, outcome_at, setup, commands
protocol/src/
  lib.rs            the surface, DEFAULT_PORT, the port a room is served
                    on, and VERSION, the version a room takes a join of
  ids.rs            PlayerId
  lobby.rs          Lobby, SeatSlot, Holder, Bot, LobbyEdit, freeze
  seating.rs        Seating, Occupant, Started, Crew: who runs which seat
  message.rs        Request, Notice, Relayed, the wire's Message tag
                    over them
  record.rs         Record
  wire.rs           Codec, the one encoding
agents/src/
  lib.rs            Agent, Seated, DECISION_INTERVAL; the surface
  dice.rs  commitments.rs  roles.rs  survey.rs  plan.rs  personality.rs
  scripted.rs       Scripted
  bin/harness.rs    native only: match, replay, rollback, matrix, draft
game/src/
  lib.rs            the surface
  controls.rs       Controls: the buttons and axes the playable reads
  display/          mod.rs; scene.rs the frame's data; wheel.rs one asteroid's
                    wheel and wheels.rs the frame's layout and hit test;
                    glyph.rs glyph_quad.rs camera.rs viewport.rs
                    stencil.rs tint.rs fights.rs send.rs belt.rs hud.rs,
                    as before; stockpile_bar.rs the top centre's box;
                    ease.rs the one span and its step;
                    hue.rs the three materials' colours;
                    label.rs titled, is_a_phrase and the words test
  net/              controller.rs Controller, Human; transport.rs
                    Transport; local.rs Local; connection.rs Connection,
                    the room this machine has joined and its own
                    transport in one; websocket/ native.rs and
                    browser.rs, one per target; listener/ Listener, the
                    room this machine serves: served.rs with the
                    server, nowhere.rs without it; machine.rs Machine,
                    the lockstep loop; pace.rs pacing
  screens/          mod.rs Playable; flow.rs Flow, Screen, Room,
                    Connect and every transition; panel.rs the styled
                    register; control.rs the three kinds of control;
                    field.rs the one typed line; held.rs the two states
                    play holds in;
                    title.rs lobby.rs loading.rs play.rs pause.rs
                    results.rs, each with the Picked or Action its
                    controls name
  main.rs           the playable, and its headless drive
  bin/look.rs       behind the `look` feature
server/src/
  lib.rs            the surface game embeds
  rooms.rs          Room, its phase with the machines in it, and the
                    outbound posts it answers with; the lobby it is the
                    authority on is kept across the match a rematch
                    opens it after
  forwarding.rs     Forwarding: its seating, hashes, desync
  records.rs        Log, the log being forwarded; Records, the kept ones
  stream.rs         the accept loop and one task per machine
  main.rs           the standalone binary
```

One concern per file; a file that needs a section comment is two files.
A section of this document is its type block plus the facts the block
cannot state; a paragraph that restates a signature or a field list is
deleted.

`sim` has one test fixture, `fixture::World`: a state and the ways to
drive it — a match off a `Setup`, a hand-seated belt, or a ring of
circular asteroids at a chosen gravity; `tick` and `run` step it, `moves`
advances motion alone, and `fix`, `hold`, `free` and `launch` place
entities. Every test that builds a state builds it there, so the world a
rule is tested in is one shape.

## Conventions

- Right-handed, +Y up, the belt plane is XZ, the central mass at the
  origin, the engine's frame exactly, so `game` converts scale and type
  and never axes.
- Lengths in meters, time in seconds inside `orbit`, ticks everywhere
  else; angles in radians; a phase is a fraction of a turn in `0..1`.
- Every `f64` parameter or field states its unit in its name or its type.
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

Tolerated runtime failures, each with the shape change that would delete
it; a new one is added here in the unit that introduces it:

- Two ships at exactly one point push each other nowhere, since a push has
  no direction there. Their drift differs by id, so they part within a tick.
  A spacing that could not be zero would delete it, which no placement rule
  can promise once ships are free to move.
- Holding's chase scans an asteroid's roll once per unit standing there,
  so one asteroid holding a thousand units costs about 11.7 ms a tick
  against a budget of 8.3; a hundred at one asteroid costs about 0.5 ms.
  Ranking the roll once per asteroid and per plating, and breaking only
  the distance tie per unit, would delete it, since the threat order
  depends on the unit through its plating alone.
- `orbit::universal` caps Newton's iteration at sixty steps; reaching the
  cap means the span was outside the contract. A `Span` type bounded by
  the body's period would delete the cap.
- `Send::joining` takes the first flight it finds between a source, a
  destination and a seat that has not departed, trusting that at most one
  exists — which holds because a send is solved only where none is found,
  and the one that is found always departs before the window could open a
  second. A store keyed by those three would make it unrepresentable, at
  the price of a second place for a send to live and be reaped; the
  flight the units already carry is the cheaper truth.
- `Schedule::between` gives up after `CORRECTIONS` passes. The margin is
  the predicate and the cap is only its guard: at the shipped
  `BURN_SHARE_OF_SPAN` no candidate inside the margin has ever needed a
  fourth pass, and a candidate the cap rejected would be one the sim could
  not have flown within the tolerance. A proof that the aim correction
  converges for every schedule inside the margin would delete the cap.
- A frame carrying a machine's request arrives at a machine's own
  connection only from a room that has broken the protocol, and is dropped
  as bytes that are not a message are. A wire typed by direction would
  delete the arm; one tagged union cannot express it.
- A room drops a notice or a desync sent by a member, since only the room
  produces them. The same direction-typed wire would delete both arms.
- `Flow::frame` replaces the screen with a placeholder while the old
  screen's room moves into the next. Only an `Option` every reader unwraps
  would delete it, which is worse.
- The lobby and results screens ignore a welcome, a refusal, and at the
  results a start, since the lobby the room broadcasts is the truth about
  what took and a machine is welcomed once. A notice type per screen would
  delete the arms.
- `label::refusal` answers with no phrase for five of `Rejected`'s seven.
  `TooMany` wants none, since the button is simply drawn spent; the other
  four cannot reach a drawn button, because a wheel asks only about the
  viewer's own seat, an asteroid the view listed and a row of the roster,
  and a viewer whose seat is out draws no button at all. Splitting
  `Rejected` into the faults of an address the state does not hold and
  the refusals of a rule, and carrying only the second to the view, would
  delete the four arms; it was left because the split ripples through
  every crate that reads an `Outcome`.
- `Play::previewed` takes a `Rejected` from `State::preview` as an empty
  preview. No hover can raise one: `Wheel::button_at` never answers with a
  button the sim refuses, so no refused want is ever previewed, and a
  drag's edits are clamped to `MAX_WANT` at an asteroid the seat stands
  units at. A `Preview` of a want and a refusal of it in one type, so the
  display could not hold the first without answering the second, would
  delete the arm.
- A `Preview` settles as though every send it names departs, so it can
  skip the schedule solve. Where no schedule exists this tick the sim
  will instead leave those units home and open frames, and the hover will
  have said otherwise for one tick. Solving inside the preview would
  delete it, at the price of the solve on every pointer move.
