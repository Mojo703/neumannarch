# Neumannarch — design

The rules of the game, as the target. This document is the authority on what
the simulation computes; DISPLAY.md is the authority on what the player
sees, and the code on how it is shaped. Where this document is silent,
prefer the reading that adds no new type, field, or rule. Numbers live in the
roster in `sim`, never here. Distances are metres, times seconds and
rates per second throughout. The roster is the table of rows a match is
played with; a row is one kind of entity, a ship or a structure, with its
stats; a want is a count of a row a player asks for at an asteroid.

## Fantasy

Each player is a von Neumann probe that has entered an uninhabited
system, arriving alongside probes sent by other countries, and fights
them for supremacy in the system. The probe's payload is its reserve, a
shipyard and a constructor; every builder is the probe replicating
itself from the system's asteroids; a faction is the country that sent the
probe; the win is holding the system when the clock runs out.

## Pillars

1. **Decisions are the player's; execution is the sim's.** The player states
   what they want and where. The sim fills it by fixed rules that contain no
   judgement. The player never references a unit.
2. **One verb.** Set the count of a row at an asteroid. Everything else
   the player does is a client gesture that issues that verb.
3. **Emergent, deterministic outcomes.** Counters come from stats and
   geometry, never from tables of types. A row states its role, and the
   role decides only the row's glyph and whether it runs passes or holds
   its station. No randomness.
4. **Short matches.** A match ends at the clock, minutes rather than hours.
5. **Two to four players in any team shape.** One against one is the balance
   baseline; four-player free-for-all is a mode.

## Session

- Win: at the clock, the side holding the most asteroids. An asteroid
  counts for a player with a structure there; ties break by total entity
  value, what everything a side holds cost to build, ships and structures
  alike. A side with no entities and an empty reserve is out before the
  clock.
- Start: nothing on the map. Each player has a stockpile and a reserve, a
  count per row, of one shipyard and one constructor. A match opens in
  the placement draft, with time stopped: no body moves, nothing is
  extracted, nothing builds, nothing is sent and nothing fires until the
  clock starts, and the belt is
  whole and visible, its asteroids and their caps read by everyone. The
  draft is a sequence of placement stages, one per reserve structure per
  seat: the first round's in an order drawn from the seed, the second
  round's in the reverse order, so the seat that went first goes last
  for its second asteroid. One placement stage runs at a time. It ends
  the moment its seat places, or after a stated span if it has not, and
  the next begins at once. A seat whose placement stage ran out keeps
  the right to place and may do so at any later tick, alongside the
  running one, first come first served. A placement is a want the seat's
  reserve fills: the row appears at the asteroid at once, complete, and
  the placement stage it was held for is placed. An asteroid any seat
  has a body homed at is taken. A want that would draw the seat's
  reserve is refused by name before that seat's first placement stage
  has begun, and refused by name at a taken asteroid, whatever its
  count. Any other want is accepted during the
  draft and stands as a want, the way a build order is queued before a
  round starts; it is filled once the clock runs. The draft ends, and
  the clock starts, on the tick every seat has placed both structures,
  or a stated span after the last placement stage ended, whichever is
  first; a seat still holding reserve then places from the asteroids
  left free, at any time. A bot places on the first tick of its
  placement stage.
- Teammates share nothing but a side. A player edits only their own
  compositions.
- Agents play through the same view and the same verb as humans.
  A bot holds a seat like a player and is run by the machine of the
  player who added it.
- Every machine runs the whole match. A command takes effect at the tick
  its player issued it. The issuer's machine applies it at once; every
  other machine applies it when it arrives, restoring that tick and
  replaying from it, so all machines converge on one history. A tick is
  settled once every seat's commands up to it are known; the settled
  history is the match's record and its replay, and two machines whose
  settled histories differ have desynced, which ends the match.
- A match is set up in a lobby. The player who opens it is its host and
  owns its shape: the map's seed, the clock, the teams, and what holds
  each seat: a player, a bot with a personality, open, meaning a player
  may still take it, or closed, meaning the seat is not in the match. A
  guest owns only their own seat's team, within the host's shape, and
  their readiness. A seat a guest holds is the guest's until the host
  removes them, which opens it. The host starts the match when every
  player is ready, every seat is held or closed, and the host holds a
  seat, its own or a bot's, so every machine in a match plays at least
  one seat. A skirmish is a lobby whose seats are all on one machine.

## World

- True 3D. A thin, near-planar belt of asteroids around one central mass,
  the star.
- The seed lays the belt. Candidates are drawn across a ring about the
  star, each on its own orbit, and a smooth field of density over the
  ring keeps or drops each one, so the belt has crowded regions and
  sparse ones and two seeds never lay the same belt. A smooth field per
  material gives every asteroid its cap, so neighbours run rich or poor
  together, in the same materials, and a region is worth holding for what
  it yields. A third smooth field gives every orbit its eccentricity, and
  the seed gives each its own periapsis direction, so a region is stirred
  or calm and neighbours drift together or apart at the same rate. Every
  orbit rises and falls by the belt's thickness, which is small against
  the ring.
- One law of motion for every body, asteroid or ship: the central mass's
  gravity, time-compressed, and nothing else. Between thrust impulses a
  body's motion is exact two-body motion, so an unpowered body keeps its
  orbit for the whole match. Asteroids and structures never thrust; their paths
  are fixed from the start and every future position is known. Nothing is
  attached to anything: bodies on similar orbits stay near each other
  because the same gravity moves them.
- Units thrust with infinite fuel, under two constant limits: the
  movement limit, one value for every unit, spent only on a transfer
  between asteroids, and a manoeuvring limit per row, far below it,
  spent holding position, keeping apart and fighting, in flight as at
  home. Every send crosses the same distance in the same time whatever
  is in it; rows differ in how they hold, chase and give way. Movement
  and combat states how each is used.
- No collisions. Entity size is visual. Arrival is the rule's tolerance
  being met (Movement and combat), never a distance a player can read.

## Materials

- Three: metals, volatiles, energy. Stockpiled per player, each with a
  capacity; excess is lost. All three are extracted from asteroids. A player's
  capacity starts at what it starts holding, so nothing it begins with is
  lost, and the rows that carry capacity add to that. A hypothesis.
- Materials differ only by where they are. Map generation gives asteroids
  regionally distinct cap triples.
- Cost vectors split roles: nimble rows are volatile-heavy, armoured rows
  metal-heavy, long-range and fast-building rows energy-heavy.
- Capacity comes from the rows that carry it, storage among them.

## Entity

One row schema. Every entity is a copy of its row plus position, velocity,
HP, and its home asteroid.

| field | notes |
|---|---|
| role | what the row is for; it decides the glyph, and short-range fire runs passes where every other role holds its station; nothing else reads it |
| manoeuvring | the manoeuvring limit, below the movement limit; zero for structures and asteroids |
| holding weights | one weight per term of the holding rule; every weight is zero for a structure, which never moves, and above zero for every unit |
| HP | |
| plating | flat damage reduction per hit |
| effects | see Effects |
| cost | a material triple |
| capacity | stockpile capacity contributed, per material |
| orbit, caps | asteroids only |

- **Asteroids** are entities: huge mass, no manoeuvring, indestructible, a
  per-material extraction cap.
- **Structures** have no manoeuvring. A structure is built at its asteroid's
  position and velocity, and since neither ever thrusts, it stays with the
  asteroid.
- **Units** have manoeuvring above zero and move at the movement limit.
- All builders build everything. A shipyard is a fast builder.
- Counters come from the stats. The role is a field, read only by the
  glyph and by the pass.

## Effects

Every effect has a kind, and each kind carries its own fields.

- **Damage.** Hitscan: the shot lands the instant it is fired, at any
  distance inside its range, so nothing travels and nothing is led. It
  states a range, a rate, a damage, and a falloff that reduces damage
  with distance. Plating is subtracted per hit.
- **Build.** Spends stockpile at `rate` toward frames at its home asteroid and
  repairs damaged friendlies there. Its reach is the asteroid's zone: a
  builder reaches everything inside it and nothing elsewhere.
- **Extract.** Pulls up to `rate` of one material, the effect's, from
  its home asteroid. The asteroid's cap for that material is the
  ceiling: at the cap, it is split equally among the extractors of that
  material there, and any share an extractor cannot use is split among
  the rest. The roster ships one extractor row per material, so what an
  asteroid yields is a per-asteroid decision against its caps.

**Build is flow.** Every shortfall is a frame draining the stockpile
continuously at the builders' combined rate. Effort combines across builders
and splits evenly across the frames at their asteroid. Nothing is reserved. A
frame is slowed only by the materials it needs, in proportion to what the
stockpile covers, and resumes as income arrives. Spend never exceeds the work a
frame has left. Lowering a count cancels frames and refunds what they consumed.
Build targets: shortfalls, then repair. A completed frame becomes an entity at
the asteroid: a structure at the asteroid's state, a unit at its asteroid's
position, offset along the asteroid's radial direction by the floor the
return term keeps, the asteroid's radius and one spacing, and one further
spacing per unit already there, so nothing spawns inside the asteroid and no
two spawn coincident. The spacing is one constant of the zone.

**Target selection is by threat.** A unit that does damage keeps the
target it last fired at, one keep per damage effect it carries, while
that target is alive and in range, whatever else stands in range. Where
it kept none, it fires at the enemy in range with the highest damage per
second through the shooter's plating per point of its HP; ties by
nearest, then lowest id. Range is the only gate on either.

**Shots resolve in order.** A unit holds the instant it is next ready to
fire, one per damage effect it carries. Within a tick, shots resolve in
ready-time order, then shooter id, each against the state at the start
of the tick plus the damage already assigned this tick; a target whose
assigned damage is lethal is skipped. Damage applies at the end of the
step, so a unit destroyed this tick still acts this tick.

**No damage or armour types.** Plating and falloff produce the counters.

## Compositions

A place is an asteroid. A composition is a player's want at a place: a count
per row. Each place holds at most one composition per player. The verb
sets one count.

- Every entity belongs to one place, its home. A unit in transit counts
  toward its home.
- **Reserve.** A shortfall is filled from the player's reserve before
  anything else: the entity appears at the place, complete, at once.
- **Surplus.** When a place has more of a row than its want, the
  highest-indexed units of that row there are surplus. Where a shortfall
  of the row exists elsewhere, a place's complete units are surplus
  before its frames of that row are unwanted, so lowering a want to send
  a unit away sends the unit and keeps the frame building; where no
  shortfall wants them, the frames are cancelled and refunded and the
  units stay. Shortfalls are filled in order of asteroid then player.
  Each is filled from the nearest surplus, asteroid to asteroid as of
  that tick, ties by lowest asteroid. A surplus unit sent to a shortfall
  cancels that place's least-progressed frame of the same row and
  refunds it. Surplus with no shortfall anywhere stays where it is,
  complete, until a shortfall wants it. Nothing complete is ever
  scrapped or refunded; a structure stays until it is destroyed.
- **Shortfall.** When a place wants more of a row than it has, counting
  units there and in transit and never frames, and neither the reserve nor
  a surplus of that row anywhere can fill it, frames open at that place for
  builders at that asteroid to fill, one frame of a row at a time; rows build
  in parallel.
- A unit's home changes only by the surplus rule.
- A composition with no want, no units, and no frames does not exist.

## Movement and combat

- **The zone.** Every asteroid has a zone: the region within one radius of
  it, the same radius for every asteroid, one constant of the belt. A unit at
  an asteroid holds inside the zone, a builder reaches
  everything inside it, a unit of a row whose role runs passes chases
  any enemy inside it, and the
  display draws it. Zones are small against the spacing of asteroids, so
  two rarely meet, and where two do the rules read no distance: a unit is
  at its home and nowhere else.
- **Sends.** Units re-homed from one asteroid to another fly at once. A
  flying unit thrusts each tick at the movement limit by one rule: it
  reads its position and velocity relative to the rim of its
  destination's zone on its own side, the point one zone radius from the
  destination's body toward the flier, the body being known exactly at
  every tick from the orbit; it thrusts along the difference between its
  own relative velocity and the velocity it wants: straight at that point,
  at the speed the limit can stop from over the distance left, root twice
  the limit times that distance, less a stated margin; where one tick at
  the limit would carry it past the velocity it wants it thrusts exactly
  the difference, so it can never orbit its destination. It arrives on the
  tick its relative distance and relative speed are both within a stated
  tolerance, and stands at its destination from then, so a force meets
  its destination at the edge of the zone facing where it came from and
  the holding rule takes it from there. So every transfer takes close to
  the least time the limit allows, two root distance over the limit, plus
  the time to match the destination's speed; a unit is flying from the
  tick it is re-homed until it arrives; and while it flies it counts
  toward its destination and is neither a shooter nor a target. In flight
  a unit thrusts by this rule alone.
- **Holding.** At its asteroid a unit moves by the holding rule. Each tick it
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
  that grows with depth inside the asteroid's own radius plus one spacing,
  so the zone is a soft shell around the asteroid and no ship moves inside
  the asteroid. Separation: a weak push from any ship of any seat nearer
  than the spacing, the distance a pair settles at; weak, since
  space is large. Station: every unit that does damage holds a place on
  the asteroid's fight stage. The stage's centre is a point half the
  roster's longest damage range outward from the asteroid's body along
  its radial, inside the zone, so a fight sits below the asteroid on the
  player's screen, where the star is always up, and never under the
  structures, whichever sides own them. Its plane stands off the body
  along the orbit's normal by the asteroid's radius and one spacing, the
  floor the return term keeps, so no station stands on the body whatever
  side a team takes. The stage's lateral is the radial exactly and its
  axis is what is left of the asteroid's orbital tangent once the
  lateral is taken out of it, normalised, so the frame is square on
  every orbit, eccentric or not, and the stage's plane runs parallel to
  the orbit's, the floor above it. The teams that hold a station at the
  asteroid divide the circle about the stage evenly between them in team
  order, the lowest on the retrograde side and each next one an even
  turn further round, so two teams face each other across the stage and
  three or four stand evenly about it; the circle is divided again the
  tick a team takes its first station there or loses its last.
  A line is a whole team's units of one row at the asteroid, the seats
  of that team pooled into one, since a side belongs to a team and not
  to a seat. A unit's station is its place in that line, on its team's
  side, at its row's stand-off from the
  stage's centre. The stand-off is half the row's damage range less half
  a stated distance, one constant of the belt, so two teams' lines of
  one row stand that distance inside their range and long-range rows
  stand behind short-range rows. Three or four teams divide the circle
  more finely: two of their lines of one row stand no further apart than
  that, and nearer the closer their sides, so they close further inside
  their range. Where three or more sides divide the circle a line
  spreads no further each way than its own stand-off, and the wrap below
  carries what will not fit into the ranks behind, so no station stands
  nearer another team's side than its own and no two lines cross. Two
  sides stand opposite and their lines run parallel, so at two teams a
  line spreads its full width. A roster carrying a damage range no
  longer than that distance cannot be built, nor one whose
  stations would stand outside the zone. A row lies
  centred on its stand-off point and spreads both ways square to it, in
  id order, a stated count of stations each way at the station spacing,
  both constants of the belt, so a row leans neither way. A row of more
  units wraps into a further rank one station spacing
  behind, the stage holds a stated count of ranks, one constant of the
  belt, and the counts are set so every station stands inside the zone;
  a row of more units than the stage has stations fills it again from
  the front, so two units share a station and separation parts them. A
  side alone at an asteroid holds its side of the stage, so a garrison
  stands before an attacker arrives, and an arrival walks from the rim
  to its station.
  A unit that does no damage takes no place on the fight stage and
  holds a station on its own circle about the asteroid's body. The
  circle's radius is the floor the return term keeps, the asteroid's
  radius and one spacing, and one station spacing further out, so it
  clears the rock and stands well inside the stage. Its plane is drawn
  from the unit's own identifier, so no two units share one and a line
  of them needs no spacing along an arc; separation parts the pair
  wherever two circles cross, and a unit keeps its plane wherever it
  goes. The circle turns at a quarter of the speed the row's
  manoeuvring limit holds on a circle of that radius, from a starting
  phase drawn from the identifier too, so two units of one row do not
  begin together. The station is the point on the circle at the current
  phase: it moves every tick and the unit circles by steering to it, by
  the same station term at the same weight as a unit that does damage.
  So a unit that does no damage comes in from the rim like every other,
  reaches the whole zone to build from anywhere on its circle, and
  stands where an enemy can reach and kill it.
  Heading never gates fire.
  Pass: a unit of a row whose role runs passes runs at the enemy of the
  highest threat standing inside the zone, at any distance, taking that
  enemy afresh every tick and keeping none, and breaks off for its
  station on the tick that same enemy hits it; reaching it, it runs
  again. A hit from any other enemy turns it nowhere, and a hit
  on its way home starts no second return, so a run lasts at least as
  long as the enemy it runs at takes to reload, and a run at an enemy
  that never fires back, a builder or an extractor, lasts until one of
  them dies. Range gates the turn through the shot alone: a shot lands
  only from inside the shooter's range, so the enemy it runs at turns it
  where that enemy can reach it and no nearer. A pass counts its station
  reached within a stated distance, one constant of the belt, and a unit
  sent to another asteroid arrives running. Where no enemy stands in the
  zone the unit steers to its station and stays in its run, so a pass
  begins from the station the tick an enemy appears.
  A pass is no term of its own and carries no weight: it names the place
  the station term steers to, in the station's stead, at the row's
  station weight, so a runner and a row that holds its station steer by
  the same sum and differ only in where that place stands. The enemy a
  unit runs at and the enemy it fires at are two separate choices: the
  pass takes one enemy in the zone to run at, the unit takes what it
  fires at by the rule under Effects, and the two need not be the same.
  Only the enemy it runs at can turn it around, so a runner fires at
  whatever stands in the range of any damage effect it carries as it
  runs, and holds its run through fire from anyone else. A row whose
  role holds its station holds it and fires from it; return stays in the
  sum throughout, so a pass never leaves the zone. A unit does not fire
  at a target whose assigned damage this tick already kills it, so a
  force spreads its fire along the enemy line. There is no facing. The
  rule is one module and is replaceable whole.
- A flying unit is neither a shooter nor a target: battles happen at
  asteroids.

## Visibility

Everything is visible to every player always: every entity, its row, its
seat, its position and velocity, every asteroid with its orbit, its caps
and all its future positions, and the standings. Wants and frames are
the one exception: a player sees their own and not another's. The
standings at any tick are what the win rule would decide were the clock
now: per side, the asteroids held, the entity value and whether it is
still in. Nothing is hidden and nothing is remembered, since there is
nothing to remember.

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

- Do per-asteroid caps and regional materials stop wants from stacking into one
  force?
- Does a drifting map force contact, or does turtling win?
- Does "nearest shortfall" surprise the player often enough to matter?
- What zone radius and damage ranges read well in the densest regions?
- Do plating and falloff give enough counters without a matrix?
- Do regional caps make three materials distinct?
- Do outcomes hold when the tick rate is doubled?
- How fast must asteroids orbit, relative to a match, for neighbours to
  change?
