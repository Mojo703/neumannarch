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
- Deterministic lockstep. Commands travel over a relay; the command log is
  the replay.

## World

- True 3D. A thin, near-planar belt of rocks around one central mass.
- One law of motion for every body, rock or ship: the central mass's
  gravity, time-compressed, and nothing else. Between thrust impulses a
  body's motion is exact two-body motion, so an unpowered body keeps its
  orbit for the whole match. Rocks and structures never thrust; their paths
  are fixed from the start and every future position is known. Nothing is
  attached to anything: bodies on similar orbits stay near each other
  because the same gravity moves them.
- Units thrust at up to a constant acceleration with infinite fuel. Every
  thrust is part of a solved transfer from one orbit to another; nothing
  steers toward a point.
- No collisions. Entity size is visual. Arrival is being within a distance.

## Materials

- Three: metals, volatiles, energy. Stockpiled per player, each with a
  capacity; excess is lost. All three are extracted from rocks.
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
| acceleration | zero for structures and rocks |
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
unit at its slot.

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
- **Surplus.** When a rock has more of a row than its want, the
  highest-indexed units of that row there are surplus. They go to the
  nearest shortfall of that row, rock to rock as of the tick the surplus
  appears, ties by lowest id; a surplus unit assigned to a rock cancels
  that rock's least-progressed frame of its row, refunded. If no shortfall
  exists anywhere, they are marked surplus and scrapped by a builder at
  their place. Structures never move; a surplus structure is scrapped.
- **Shortfall.** When a place wants more of a row than it has, counting
  units there and in transit and never frames, and neither the reserve nor
  a surplus of that row anywhere can fill it, frames open at that place for
  builders at that rock to fill.
- A unit's home changes only by the surplus rule.
- A composition with no want, no units, and no frames does not exist.

## Movement and combat

- A unit whose home is elsewhere flies a solved transfer: from its state to
  its slot's orbit at an arrival tick, within its acceleration limit.
  Units re-homed in one tick to one place share an arrival tick, the
  slowest transfer's, so a send arrives together.
- A unit fires only while holding at its home place. A unit in flight is
  neither a shooter nor a target: battles happen at rocks.
- At its home a unit holds a slot: an orbit with the rock's period whose
  path circles the rock at its band's amplitude, its phase set by the
  unit's index among its owner's units at that place. Orbits of equal
  period stay bounded to each other, so a unit on its slot's orbit holds
  without thrust and circles the rock once per rock orbit. This is a sim
  rule so the display and the harness agree on where a held force is.
- The outer band's amplitude exceeds the inner band's by more than the
  longest weapon range, so a force in the outer band is out of every fight
  in the inner band and in range of everything else in the outer band.
  Band amplitudes and weapon ranges are small against the spacing of rocks,
  so no two rocks' bands overlap.
- While holding, the leash is the unit's sight, measured from its slot, cut
  to its own band. Inside it, a unit chases the nearest enemy the player
  can see: a solved transfer to a point at half its weapon range from the
  enemy's predicted state, re-solved as the enemy moves. It returns to its
  slot when it exceeds the leash. It never
  retreats. Radar contacts are not chased.

## Fog

Per player, from the union of their sensors. **Sight** is exact. **Radar**
gives position, velocity, and rough mass. **Terrain** is every rock, its
orbit, its caps, and all its future positions, always. Sight is shared
across a player's entities.

## Build order

1. The display language over synthetic scenes (DISPLAY.md).
2. The headless sim: roster, rocks held static, the verb, reserve, surplus
   and shortfall, holding in two bands, fire, fog.
3. The playable on the engine over the sim's view.
4. A scripted agent and the balance harness.
5. Orbits, gravity, intercept.
6. Map generation from seed with regional caps.
7. Factions as skews over one roster; rows beyond the first nine.

## Questions for the harness

Not rules. Each is a hypothesis the harness confirms or kills.

- Do per-rock caps and regional materials stop wants from stacking into one
  force?
- Does a drifting map force contact, or does turtling win?
- Does "nearest shortfall" surprise the player often enough to matter?
- Does a force staged in the outer band get ambushed there, and is that
  good?
- What band radii and weapon ranges read well in the densest regions?
- Do plating and falloff give enough counters without a matrix?
- Do regional caps make three materials distinct?
- Does a send that must arrive together lose too much to the slowest row?
- Do outcomes hold when the tick rate is doubled?
- How fast must rocks orbit, relative to a match, for neighbours to
  change?
