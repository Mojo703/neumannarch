# Neumannarch — design

The rules of the game, as the target. This document is the authority on what
the simulation computes; ARCHITECTURE.md is the authority on how the code is
shaped, DISPLAY.md on what the player sees. Where this document is silent,
prefer the reading that adds no new type, field, or rule. Numbers live in the
roster in `sim`, never here. Distances are metres, times seconds and
rates per second throughout. The roster is the table of rows a match is
played with; a row is one kind of entity, a ship or a structure, with its
stats; a want is a count of a row a player asks for at a rock.

## Fantasy

Each player is a von Neumann probe that has entered an uninhabited
system, arriving alongside probes sent by other countries, and fights
them for supremacy in the system. The probe's payload is its reserve, a
shipyard and a constructor; every builder is the probe replicating
itself from the system's rocks; a faction is the country that sent the
probe; the win is holding the system when the clock runs out.

## Pillars

1. **Decisions are the player's; execution is the sim's.** The player states
   what they want and where. The sim fills it by fixed rules that contain no
   judgement. The player never references a unit.
2. **One verb.** Set the count of a row at a rock. Everything else the player
   does is a client gesture that issues that verb.
3. **Emergent, deterministic outcomes.** Counters and roles come from stats
   and geometry, never from tables of types. No randomness.
4. **Short matches.** A match ends at the clock, minutes rather than hours.
5. **Two to four players in any team shape.** One against one is the balance
   baseline; four-player free-for-all is a mode.

## Session

- Win: at the clock, the side holding the most rocks. A rock counts for a
  player with a structure there; ties break by total army value. A side
  with no entities and an empty reserve is out before the clock.
- Start: nothing on the map. Each player has a stockpile and a reserve, a
  count per row, of one shipyard and one constructor. A match opens in
  the placement draft, with time stopped: no body moves, nothing is
  extracted and nothing builds until the clock starts, and the belt is
  whole and visible, its rocks and their caps read by everyone. The
  draft is a sequence of stages, one per reserve structure per seat:
  the first round's stages in an order drawn from the seed, the second
  round's in the reverse order, so the seat that went first goes last
  for its second rock. One stage runs at a time. A stage ends the
  moment its seat places, or after a stated span if it has not, and
  the next begins at once. A seat whose stage ran out keeps the right
  to place and may do so at any later tick, alongside the running
  stage, first come first served. A placement lands at a rock no draft
  placement has taken, and a rock a draft placement stands on is
  taken. A reserve want before the seat's first stage has begun is
  refused by name; a want at a taken rock is refused by name. Any
  other want is accepted during the draft and stands as a want, the
  way a build order is queued before a round starts; it is filled once
  the clock runs. The draft ends, and the clock starts, on the tick
  every seat has placed both structures, or a stated span after the
  last stage ended, whichever is first; a seat still holding reserve
  then places from the rocks left free, at any time. A bot places on
  the first tick of its stage.
- Teammates share nothing but a side. A player edits only their own
  compositions.
- Agents play through the same view and the same verb as humans.
  A bot holds a seat like a player and is run by the machine of the
  player who added it.
- Every machine runs the whole match. A command takes effect at the tick
  its player issued it. The issuer's machine applies it at once; every
  other machine applies it when it arrives, restoring that tick and
  replaying from it, so all machines converge on one history. A tick is settled once every seat's commands up to it are
  known; the settled history is the match's record and its replay, and
  two machines whose settled histories differ have desynced, which ends
  the match.
- A match is set up in a lobby. The player who opens it is its host and
  owns its shape: the map's seed, the clock, the teams, and what holds
  each seat: a player, a bot with a personality, open, meaning a player
  may still take it, or closed, meaning the seat is not in the match. A guest owns only their own seat's team, within the host's
  shape, and their readiness. A seat a guest holds is the guest's until
  the host removes them, which opens it. The host starts the match when
  every player is ready, every seat is held or closed, and the host
  holds a seat, its own or a bot's, so every machine in a match plays at
  least one seat. A skirmish is a lobby whose seats are all on one
  machine.

## World

- True 3D. A thin, near-planar belt of rocks around one central mass.
- One law of motion for every body, rock or ship: the central mass's
  gravity, time-compressed, and nothing else. Between thrust impulses a
  body's motion is exact two-body motion, so an unpowered body keeps its
  orbit for the whole match. Rocks and structures never thrust; their paths
  are fixed from the start and every future position is known. Nothing is
  attached to anything: bodies on similar orbits stay near each other
  because the same gravity moves them.
- Units thrust with infinite fuel, under two constant limits: the
  movement limit, one value for every unit, spent only on a transfer
  between rocks, and a manoeuvring limit per row, far below it, spent
  holding position, keeping apart and fighting, in flight as at home. Every send crosses the same distance in
  the same time whatever is in it; rows differ in how they hold, chase
  and give way. Movement and combat states how each is used.
- No collisions. Entity size is visual. Arrival is the end of a schedule
  (Movement and combat), never a distance.

## Materials

- Three: metals, volatiles, energy. Stockpiled per player, each with a
  capacity; excess is lost. All three are extracted from rocks. A player's
  capacity starts at what it starts holding, so nothing it begins with is
  lost, and the rows that carry capacity add to that. A hypothesis.
- Materials differ only by where they are. Map generation gives rocks
  regionally distinct cap triples.
- Cost vectors split roles: nimble rows are volatile-heavy, armoured rows
  metal-heavy, long-range and fast-building rows energy-heavy.
- Capacity comes from the rows that carry it, storage among them.

## Entity

One row schema. Every entity is a copy of its row plus position, velocity,
HP, and its home rock.

| field | notes |
|---|---|
| manoeuvring | the manoeuvring limit, below the movement limit; zero for structures and rocks |
| holding weights | one weight per term of the holding rule |
| HP | |
| plating | flat damage reduction per hit |
| weapons | see Weapons |
| cost | a material triple |
| capacity | stockpile capacity contributed, per material |
| orbit, caps | rocks only |

- **Rocks** are entities: huge mass, no manoeuvring, indestructible, a
  per-material extraction cap.
- **Structures** have no manoeuvring. A structure is built at its rock's
  position and velocity, and since neither ever thrusts, it stays with the
  rock.
- **Units** have manoeuvring above zero and move at the movement limit.
- All builders build everything. A shipyard is a fast builder.
- Roles emerge from ratios. There is no role field.

## Weapons

Every weapon has a kind, and each kind carries its own fields.

- **Damage.** Hitscan, with a range, a rate, a damage, and a falloff that
  reduces damage with distance. Plating is subtracted per hit.
- **Build.** Spends stockpile at `rate` toward frames at its home rock and
  repairs damaged friendlies there. Its reach is the rock's zone: a
  builder reaches everything inside it and nothing elsewhere.
- **Extract.** Pulls up to `rate` of one material, the weapon's, from
  its home rock. The rock's cap for that material is the ceiling: at the
  cap, it is split equally among the extractors of that material there,
  and any share an extractor cannot use is split among the rest. The
  roster ships one extractor row per material, so what a rock yields is
  a per-rock decision against its caps.

**Build is flow.** Every shortfall is a frame draining the stockpile
continuously at the builders' combined rate. Effort combines across builders
and splits evenly across the frames at their rock. Nothing is reserved. A
frame is slowed only by the materials it needs, in proportion to what the
stockpile covers, and resumes as income arrives. Spend never exceeds the
work a frame has left. Lowering a count cancels frames and refunds what they
consumed. Build targets: shortfalls, then repair. A completed
frame becomes an entity at the rock: a structure at the rock's state, a
unit at its rock's position, offset along the rock's radial direction by
one spacing per unit already there, so no two spawn
coincident. The
spacing is one constant of the zone.

**Target selection is by threat.** A weapon fires at the enemy in range
with the highest damage per second through the shooter's plating per
point of its HP;
ties by nearest, then lowest id. Range is the only gate.

**Shots resolve in order.** Every weapon carries the instant it is next
ready. Within a tick, shots resolve in ready-time order, then shooter id,
each against the state at the start of the tick plus the damage already
assigned this tick; a target whose assigned damage is lethal is skipped.
Damage applies at the end of the step, so a unit destroyed this tick still
acts this tick.

**No damage or armour types.** Plating and falloff produce the counters.

## Compositions

A place is a rock. A composition is a player's want at a place: a count
per row. Each place holds at most one composition per player. The verb
sets one count.

- Every entity belongs to one place, its home. A unit in transit counts
  toward its home.
- **Reserve.** A shortfall is filled from the player's reserve before
  anything else: the entity appears at the place, complete, at once.
- **Surplus.** When a place has more of a row than its want, the
  highest-indexed units of that row there are surplus. Shortfalls are
  filled in order of rock then player. Each is filled from the nearest
  surplus, rock to rock as of that tick, ties by lowest rock. A surplus
  unit sent to a shortfall cancels that place's least-progressed frame of
  the same row and refunds it. Surplus with no shortfall anywhere stays
  where it is, complete, until a shortfall wants it. Nothing complete is
  ever scrapped or refunded; a structure stays until it is destroyed.
- **Shortfall.** When a place wants more of a row than it has, counting
  units there and in transit and never frames, and neither the reserve nor
  a surplus of that row anywhere can fill it, frames open at that place for
  builders at that rock to fill, one frame of a row at a time; rows build
  in parallel.
- A unit's home changes only by the surplus rule.
- A composition with no want, no units, and no frames does not exist.

## Movement and combat

- **The zone.** Every rock has a zone: the region within one radius of
  it, the same radius for every rock, one constant of the belt. A unit at
  a rock holds inside the zone, a builder reaches
  everything inside it, a unit chases any enemy inside it, and the
  display draws it. Zones are small against the spacing of rocks, so no
  two overlap.
- **Sends.** Units re-homed in one tick from one place to another make one
  send; units re-homed within a stated window of ticks join the send that
  is forming. A send is one schedule of thrust, solved when the send
  begins: a sequence of thrusts, one per tick, each within the movement
  limit, whose integration by the sim's own propagation carries a ship
  from the source rock's orbit to the destination rock's orbit at the
  arrival tick, to within a stated tolerance in position and in speed. A
  schedule exists at an arrival tick when its burns together take at most
  a stated share of the span and its integration meets the tolerance. The arrival tick is the earliest at
  which a schedule exists. A schedule departs on the tick after the send's
  window closes, the first tick its ships thrust on. Every ship of the send
  departs at once and flies the one schedule, so their offsets from each
  other and from the rock at departure are carried to arrival: a force
  that leaves spread through its zone arrives spread through the
  destination's. While its send forms a unit stands at its rock, a
  shooter and a target there, and is counted toward its destination. A
  unit is flying from the tick its schedule departs until it ends. The tolerance is the schedule's. Separation in flight
  can push a ship in company off its schedule by arrival, and it holds
  from wherever it ends.
- **Power.** Every unit has a power: its damage per second, through no
  plating, times its remaining HP. It is the one number the fields below sum and it falls
  as a unit is hurt.
- **The fields.** Each tick, at each rock, each side has a strength
  field: at any point, the sum over that side's units at the rock of
  the unit's power times a smooth kernel of its distance from the point,
  the kernel's scale a constant of the zone. A unit reads the two fields
  at its own position, its side's and the enemy's, and the fraction of
  the strength there that is its own side's, so it knows who is strong
  here without knowing any absolute number.
- **Holding.** At its rock a unit moves by the holding rule. Each tick it
  sums the steering terms below and thrusts by the sum, capped at its
  row's manoeuvring limit. Each term's weight is the row's. A term that
  pulls toward a place pulls toward a desired velocity, the difference
  between that velocity and the unit's own, so the rule damps itself and
  a unit arrives without ringing. The desired speed toward a place is
  the speed the row's manoeuvring limit can stop from within the arrival
  distance, one constant of the zone, so a strong row is also a fast one
  and no row states a speed. Wander: a
  held force drifts through the zone and never sits still. Return: a
  pull back that grows with distance outside the zone, and a push out
  that grows with depth inside the rock's own radius plus one spacing,
  so the zone is a soft shell around the rock and no ship moves inside
  the rock. Separation: a weak push from any ship of any seat nearer
  than the spacing, the distance a pair settles at; weak, since
  space is large. Cohesion: a pull up the gradient of its own side's
  field, toward where its allies' strength is. Caution: a push down the
  gradient of the enemy's field, weighted by how outnumbered the unit is
  where it stands, so a unit in a strong group ignores it and a lone
  unit falls back toward its allies. Chase: a pull toward the enemy
  inside the zone its fire rule would choose (Weapons, Target
  selection), to half the unit's longest weapon range from it, holding
  there; the unit it chases is the unit it fires at, and a unit with
  no damage weapon has no chase and only its other terms. Cohesion is
  weighted so a force closes on its target as one body, which is what
  makes a battle predictable; there is no facing. The rule is one module
  and is replaceable whole. In flight a unit thrusts by its schedule and
  by separation alone.
- A flying unit is neither a shooter nor a target: battles happen at
  rocks.

## Visibility

Everything is visible to every player always: every entity, its row, its
seat, its position and velocity, every rock with its orbit, its caps and
all its future positions, and the standings. Wants and frames are the
one exception: a player sees their own and not another's. The standings at any tick
are what the win rule would decide were the clock now: per side, the
rocks held, the army value and whether it is still in. Nothing is hidden
and nothing is remembered, since there is nothing to remember.

## Build order

1. The display language over synthetic scenes (DISPLAY.md).
2. The headless sim: roster, orbits, the verb, reserve, surplus and
   shortfall, sends, holding, fire.
3. The playable on the engine over the sim's view.
4. A scripted agent and the balance harness.
5. Map generation from seed with regional caps.
6. Factions as skews over one roster; rows beyond the first nine.

## Questions for the harness

Not rules. Each is a hypothesis the harness confirms or kills.

- Do per-rock caps and regional materials stop wants from stacking into one
  force?
- Does a drifting map force contact, or does turtling win?
- Does "nearest shortfall" surprise the player often enough to matter?
- What zone radius and weapon ranges read well in the densest regions?
- Do plating and falloff give enough counters without a matrix?
- Do regional caps make three materials distinct?
- Do outcomes hold when the tick rate is doubled?
- How fast must rocks orbit, relative to a match, for neighbours to
  change?
