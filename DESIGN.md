# Probe Game — design

The rules of the game, as the target. This document is the authority on what
the simulation computes; ARCHITECTURE.md is the authority on how the code is
shaped, DISPLAY.md on what the player sees. Where this document is silent,
prefer the reading that adds no new type, field, or rule. Numbers live in the
roster in `sim`, never here.

## Pillars

1. **Decisions are the player's; execution is the sim's.** The player states
   what they want and where. The sim fills it by fixed rules that contain no
   judgement. The player never references a unit.
2. **One verb.** Set the count of a row at a rock. Everything else the player
   does is a client gesture that issues that verb.
3. **Emergent, deterministic outcomes.** Counters and roles come from stats
   and geometry, never from tables of types. No randomness.
4. **Short matches.** A match ends at the clock, about fifteen minutes.
5. **Two to four players in any team shape.** One against one is the balance
   baseline; four-player free-for-all is a mode.

## Session

- Win: at the clock, the side holding the most rocks. A rock counts for a
  player with a structure there; ties break by total army value. A side
  with no entities and an empty reserve is out before the clock.
- Start: nothing on the map. Each player has a stockpile and a reserve, a
  count per row, of one shipyard and one constructor. The first wants place
  them; where a player starts is theirs to choose and hidden until seen.
- Teammates share sight and nothing else. A player edits only their own
  compositions.
- Agents play through the same fogged view and the same verb as humans.
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
  shape, and their readiness. The host starts the match when every player
  is ready and every seat is held or closed. A skirmish is a lobby whose
  seats are all on one machine.

## World

- True 3D. A thin, near-planar belt of rocks around one central mass.
- One law of motion for every body, rock or ship: the central mass's
  gravity, time-compressed, and nothing else. Between thrust impulses a
  body's motion is exact two-body motion, so an unpowered body keeps its
  orbit for the whole match. Rocks and structures never thrust; their paths
  are fixed from the start and every future position is known. Nothing is
  attached to anything: bodies on similar orbits stay near each other
  because the same gravity moves them.
- Units thrust with infinite fuel, under two constant limits per row: the
  movement limit, spent only on a transfer between rocks, and the
  manoeuvring limit, far below it, spent holding position and fighting.
  Movement and combat states how each is used.
- No collisions. Entity size is visual. Arrival is being within a distance.

## Materials

- Three: metals, volatiles, energy. Stockpiled per player, each with a
  capacity; excess is lost. All three are extracted from rocks. A player's
  capacity starts at what it starts holding, so nothing it begins with is
  lost, and the rows that carry capacity add to that. A hypothesis.
- Materials differ only by where they are. Map generation gives rocks
  regionally distinct cap triples.
- Cost vectors split roles: fast rows are volatile-heavy, armoured rows
  metal-heavy, long-range and fast-building rows energy-heavy.
- Capacity comes from the rows that carry it, storage among them.

## Entity

One row schema. Every entity is a copy of its row plus position, velocity,
HP, and its home rock.

| field | notes |
|---|---|
| mass | radar reveals it roughly |
| acceleration | the movement limit; zero for structures and rocks |
| manoeuvring | the manoeuvring limit, far below acceleration; zero for structures and rocks |
| HP | |
| plating | flat damage reduction per hit |
| sight, radar | detection ranges |
| weapons | see Weapons |
| cost | a material triple |
| capacity | stockpile capacity contributed, per material |
| orbit, caps | rocks only |

- **Rocks** are entities: huge mass, zero acceleration, indestructible,
  always visible, a per-material extraction cap.
- **Structures** have zero acceleration. A structure is built at its rock's
  position and velocity, and since neither ever thrusts, it stays with the
  rock.
- **Units** have acceleration above zero.
- All builders build everything. A shipyard is a fast builder.
- Roles emerge from ratios. There is no role field.

## Weapons

Every weapon has a kind, and each kind carries its own fields.

- **Damage.** Hitscan, with a range, a rate, a damage, and a falloff that
  reduces damage with distance. Plating is subtracted per hit.
- **Build.** Spends stockpile at `rate` toward frames at its home rock;
  repairs damaged friendlies there; scraps surplus there at `rate` with
  full refund. Scrap is work. It has no range: a builder reaches everything
  at its own rock and nothing elsewhere.
- **Extract.** Pulls up to `rate` of each material from its home rock. The
  rock's cap per material is the ceiling: at the cap, it is split equally
  among the extractors there, and any share an extractor cannot use is
  split among the rest.

**Build is flow.** Every shortfall is a frame draining the stockpile
continuously at the builders' combined rate. Effort combines across builders
and splits evenly across the frames at their rock. Nothing is reserved. A
frame is slowed only by the materials it needs, in proportion to what the
stockpile covers, and resumes as income arrives. Spend never exceeds the
work a frame has left. Lowering a count cancels frames and refunds what they
consumed. Build targets: shortfalls, then scrap, then repair. A completed
frame becomes an entity at the rock: a structure at the rock's state, a
unit at its place's anchor, offset along the rock's radial direction by one
spacing (Movement and combat) per unit already there, so no two spawn
coincident.

**Target selection is by threat.** A weapon fires at the enemy in range that
the player can see with the highest DPS through the shooter's plating per
point of its HP; ties by nearest, then lowest id. Fire requires sight, never
radar. Fire is never gated by the leash.

**Shots resolve in order.** Every weapon carries the instant it is next
ready. Within a tick, shots resolve in ready-time order, then shooter id,
each against the state at the start of the tick plus the damage already
assigned this tick; a target whose assigned damage is lethal is skipped.
Damage applies at the end of the step, so a unit destroyed this tick still
acts this tick.

**No damage or armour types.** Plating and falloff produce the counters.

## Compositions

A place is a rock's inner band or its outer band. A composition is a
player's want at a place: a count per row. Each place holds at most one
composition per player. The verb sets one count.

- Every entity belongs to one place, its home. A unit in transit counts
  toward its home. Structures exist only in inner bands.
- **Reserve.** A shortfall is filled from the player's reserve before
  anything else: the entity appears at the place, complete, at once.
- **Surplus.** When a place has more of a row than its want, the
  highest-indexed units of that row there are surplus. A shortfall of that
  row, places taken in order of rock then band then player, is filled from
  the nearest surplus, rock to rock as of that tick, ties by lowest rock;
  a surplus unit assigned to a place cancels that place's least-progressed
  frame of its row, refunded. Surplus with no shortfall anywhere is marked
  surplus and scrapped by a builder at its place. Structures never move; a
  surplus structure is scrapped.
- **Shortfall.** When a place wants more of a row than it has, counting
  units there and in transit and never frames, and neither the reserve nor
  a surplus of that row anywhere can fill it, frames open at that place for
  builders at that rock to fill.
- A unit's home changes only by the surplus rule.
- A composition with no want, no units, and no frames does not exist.

## Movement and combat

- **Anchors.** Every place has an anchor: the rock's orbit shifted along
  that orbit, ahead of the rock along its motion, by the band's amplitude,
  a distance. An anchor is an orbit: at any tick it gives a position and a
  velocity. It is never an entity and never drawn. All seats at a place
  share its anchor.
- **Sends.** Units re-homed in one tick from one place to another make one
  flight. A flight is one solved transfer between the two anchors: two
  impulses, the first at the departure tick and the second at the arrival
  tick, each spread into a burn per row that thrusts at that row's movement
  limit for as long as the impulse needs. The arrival tick is the earliest
  at which every row's two burns fit inside the flight without overlapping,
  so a send arrives together and the slowest row sets the tick. A flight's
  own anchor is its departure state under the first impulse, propagated;
  that anchor is what every ship of the send follows. A unit is flying from
  the tick it joins a flight until it is within the arrival distance of its
  destination anchor.
- **The attractor.** Each tick every unit has one attractor, a position and
  a velocity, the first of these that applies: its flight's anchor, while
  the arrival tick has not passed; its home anchor, once the arrival tick
  has passed, and the flight ends for that ship when it is within the
  arrival distance of that anchor; the point at half its longest weapon
  range from the nearest enemy its seat can see inside the leash, on the
  line from that enemy toward the ship, moving at that enemy's velocity;
  otherwise its home anchor. It never retreats. Radar contacts are not
  chased.
- **Manoeuvring.** Each tick a unit thrusts once, within its manoeuvring
  limit, by the sum of two terms. The first is a pull to its attractor,
  proportional to the offset from the unit's position to the attractor's
  and to the difference between the unit's velocity and the attractor's.
  The second is one term per ship of any seat nearer than the cutoff,
  along the line between the two. Two distances, the spacing and the
  cutoff, are constants of the rule. Below the spacing the term pushes the
  pair apart, strongest at half the spacing. Between the spacing and the
  cutoff it pulls them together, strongest halfway, and never as hard as it
  pushes, so a crowd cannot compress itself. It is zero at the spacing, at
  the cutoff, and beyond the cutoff, and it is zero where the two coincide,
  since there is no line between them.
- **Right of way.** A pair's term is one magnitude for the pair, split
  between the two by mass: each ship's share is the other ship's mass over
  the pair's total mass, so the lighter ship moves more. The manoeuvring
  limit then caps what a ship can do about being pushed, so a heavy row
  with a low limit holds its line and a light row gives way.
- A flying unit is neither a shooter nor a target: battles happen at
  rocks.
- The leash is the unit's sight, measured from its home anchor.
- The outer anchor sits farther ahead of the rock than the inner anchor by
  more than the longest weapon range plus twice the cutoff, so no ship
  holding in the outer band fights a ship holding in the inner band. Band
  amplitudes, weapon ranges and the cutoff are small against the spacing of
  rocks, so no two rocks' bands overlap.

## Fog

Per player, from the union of their sensors. **Sight** is exact. **Radar**
gives position, velocity, and rough mass. **Terrain** is every rock, its
orbit, its caps, and all its future positions, always. Sight is shared
across a player's entities. The standings are revealed at the clock and
never before; until then a player knows of another side's rocks and army
only what sight and radar have shown.

## Build order

1. The display language over synthetic scenes (DISPLAY.md).
2. The headless sim: roster, orbits, the verb, reserve, surplus and
   shortfall, anchors and flights, manoeuvring, fire, fog.
3. The playable on the engine over the sim's view.
4. A scripted agent and the balance harness.
5. Map generation from seed with regional caps.
6. Factions as skews over one roster; rows beyond the first eight.

## Questions for the harness

Not rules. Each is a hypothesis the harness confirms or kills.

- Do per-rock caps and regional materials stop wants from stacking into one
  force?
- Does a drifting map force contact, or does turtling win?
- Does "nearest shortfall" surprise the player often enough to matter?
- Does a force staged in the outer band get ambushed there, and is that
  good?
- What band amplitudes and weapon ranges read well in the densest regions?
- Do plating and falloff give enough counters without a matrix?
- Do regional caps make three materials distinct?
- Does a send that must arrive together lose too much to the slowest row?
- Do outcomes hold when the tick rate is doubled?
- How fast must rocks orbit, relative to a match, for neighbours to
  change?
