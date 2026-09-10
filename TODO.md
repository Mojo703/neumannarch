# Neumannarch — project state (overseer-facing)

Present and future tense only: what is in flight, what is queued, the
rules every brief carries, the engine gaps verified open, and what is
settled. Nothing dated stays: a landed or resolved item is deleted in
the session it lands, never annotated, and a ruling that changes the
game goes into DESIGN.md or DISPLAY.md, never here. The commit message
is the record. Agents see this file only through their briefs.

## In flight

Uncommitted and verified green by the overseer's own gate, replay and
rollback (2026-09-09): the match clock is a duration. `Setup::clock` was
a `Tick` while meaning a span of running match time, so the room could
compare it against an absolute tick and did, in two places, dropping
every relayed command and ignoring every hash report over the final
stretch of every multiplayer match. It is a `Time` now, so both
comparisons are type errors rather than plausible lines; `Tick::since`
and `Tick::after` are the only named crossings between a moment and a
duration, and the two puns elsewhere in `sim` are gone with them. The
room bounds against `Setup::ends_by()`, the draft's certain end after
the clock, and a sim test plays a match out at one, two, three and four
seats and asserts the final tick equals it, so the bound cannot drift
from the draft's stage count in silence. Shown failing first at the
room, at tick 15,600 of a 7,200-tick clock, for the command and for the
hash report separately. `Forwarding::commanded` no longer folds the
ledger's refusal into the ownership check, so the two refusals are
tellable apart and the test discriminates rather than merely passes.
The hash did not move. Budgets: code +64/-50, tests +120/-18, docs +10.

The next unit, ruled 2026-09-09: the four holes in the tests below,
widened to carry two `game` tests that assert the opposite of their
names, at `display/scene.rs` — a surplus test that passes for a client
doing `present - want` because present is zero in its scene, and a cost
test that asserts exactly what client arithmetic gives, leaving
DISPLAY.md's own discriminator, that what the reserve or a surplus fills
is free and the bar marks nothing, untested. After it, the relayed
command below.

Two holes open, neither fixed:
- `game/src/net/machine.rs` expects its own controller's command to be
  taken, and the bot clears the thirty-two command cap with no margin,
  since `Plan::commands` ends in `take(MAX_COMMANDS_PER_TICK)` and emits
  exactly thirty-two. A second command source on one seat at one tick,
  a draft pick beside a funded plan, panics the game process. This is
  the likeliest explanation for the crashes the owner saw and blamed on
  an agent. The fix is for the controller to hand over a type that
  cannot hold more than a tick's commands, so the refusal has no arm to
  reach. The cap is enforced in three places that do not know about each
  other, which is the root it shares with the hole below.
- `Machine::apply` drops a refused relayed command silently, so a
  machine that learns a command late loses it for good and desyncs at
  the next hash. Only `Refused::Late` is reachable from a peer, and only
  on one boundary: a command tick a multiple of sixteen, a receiver
  pinned at the pacing ceiling, and a delivery gap of 133 ms or more
  between a peer's acknowledgement and the command after it. Three of
  the sim's four refusals are unreachable only because of facts held in
  `game/src/net/pace.rs` and `server/src/records.rs`, crates the sim
  cannot see; that inversion is the shape defect under it. The fix is
  for the session's oldest tick to be the settled one rather than a
  rewind budget used as a validity bound, which leaves only a tick a
  broken sender could name, and that is a desync to surface.

Holes in the tests, measured 2026-09-09, the unit after the clock:
- Falloff has never been exercised. Every test row is built with a
  falloff of zero and the raider's is never checked, so the term can be
  deleted from `step/fire.rs` with the whole gate still green.
- A hit is never checked against the target's plating. The crate's one
  damage assertion fires at a storage, whose plating is zero. One test
  with the falloff above.
- Repair has no test anywhere in the workspace: `Construction::repair`,
  `Progress::repairs`, `State::heal` and `Entities::heal`.
- The win rule is untested. `Standings::leaders` decides every match and
  its only caller is the harness; `Team::value` is asserted nowhere.

Rulings of 2026-09-09, so they are not re-raised:
- Three and four sides divide the circle with a dead arc of 25 degrees
  between sectors, which lifts the worst enemy station pair at four
  teams from 0.707 m to the 1.414 m ceiling and leaves two and three
  teams bit-identical. The cap on a line's spread is written as an
  absolute length where it is a fraction of the sector; at four teams
  the sector half-angle is 45 degrees and the two coincide exactly. The
  binding constraint is the raider's 1 m stand-off, so no rule keeping a
  row at its own stand-off can beat 1.414 at four teams.
- Two rows of one team can hold the same station, whenever their damage
  ranges differ by a multiple of four metres, as the frigate's and the
  lancer's do. Left as it stands (owner: it has not affected a game), so
  it owes an INVARIANTS.md entry with the shape change that would delete
  it, and the sentence claiming long-range rows stand behind short-range
  rows is false for four of the frigate's six ranks and wants
  correcting. Both land with the dead arc.
- Factions are built on top of the entity refactor, not before it, and
  the refactor's const stats are not reopened for them.
- The refactor's type is `EntityPattern` as ruled.
- One lane: a unit lands and commits before the next begins, and no
  second working tree is opened to run two at once.
- `agents` stays undocumented until rating settles the bot, so its
  thirty-eight tests pinning rules stated nowhere else are deliberate.
- Every cut-out in a glyph is filled in pure black: the role pictogram,
  the tier notches, the tier-three corners and the tier-two inner
  border. A glyph is one opaque shape, so a pile of ships cannot show
  one glyph through another and invent a third. A DISPLAY.md change.
- A hollow glyph is a dim solid one, so DISPLAY.md loses the three
  sentences calling it the frame's outline alone, `Fill` collapses to
  one arm, and the wheel's halved alpha becomes the stated way a hollow
  row reads. The deletion, not the implementation. Lands with the
  cut-outs above.
- The scenes a judge reads are one seeded two-bot skirmish at three
  named ticks, not hand-typed `Scene` literals: `display/local.rs` is
  promoted out from behind `#[cfg(test)]`, `Watched` and
  `skirmish_where_you_go_first` are deleted, and `Scene` becomes opaque
  so nothing outside the crate can build a state the sim would not
  produce. About 400 lines out of `look.rs`.

Read at 820a036 and written to the session scratchpad, each a durable
list rather than a warm agent: the overhaul's impact map over 682 lines
in 75 files, with nothing in the repo to re-baseline; a roster tree
study from OpenRA, Zero-K, Warzone 2100 and Beyond All Reason read as
real files; the circle arithmetic with its scripts; the relayed-command
comparison; a contraction audit of 16 findings, about 230 lines falling
out mechanically; an audit of the sim's 252 tests against the sentences
they pin; and a survey of the three crates whose public surfaces hide
dead code from the gate.

Everything below this line is the history of units already committed;
prune it in the next session that has the budget.

The bot unit, built and verified 2026-09-08, awaiting the owner's
review of the diff and the commit. The owner read the code and
accepted it, accepted the non-test budget miss (2416 against 1817,
289 of it the match runner and guarantee watchers shared by tests and
`harness verify`), and ruled the three deviations stand: a frame with
no builder may last one decision, garrisons grow only where a builder
stands, and a near-full stockpile also wants one more yard at home
since build is flow at the builders' rate. The guarantees run in
`check.sh` as `harness verify 6` in release (seven seconds) rather
than as tests (130 seconds). Renamed before review: brimming to
stock_near_capacity, cramped to short_of_room, sink to
spend_overflow, Trial to PlayedMatch. Fixed after the owner's play
found thirty shipyards at one asteroid: the overflow yard is wanted
at the staging asteroid, one frame at a time, never at home every
second; the overflow warship is the row whose cost is most in the
fullest material, at the building asteroid with no frame open; and
the stockpile guarantee is stated as the defect measured, a full
stockpile with spend under nine tenths of the builders' rate, since
a besieged seat working every builder flat out is not hoarding. The
mirror ends with four yards against two and armies of five hundred
and two hundred, which the thousand-units-per-asteroid cost below
now bears on. Found by the owner's play: the
bots' names no longer say how they play, since expansion is
failure-driven for both and a turtle claims as much as an expander
(turtle against expand ends 8 to 7 asteroids); the personalities
differ by claims, masons, yards and the army numbers alone. A balance
pass item. Also from the owner's play: a bot builds shipyards at one
asteroid while its warships are wanted at another, since yards go to
home and the developed asteroids while the fill feeds whichever
asteroid's force is furthest below its want, often a claim whose one
constructor builds at a fifth of a yard's rate; the fill should weigh
the build rate standing at the asteroid, or the yards should follow
the forces. The same balance pass, with two more from the owner's
questions (2026-09-08): the attack is a trickle by construction,
since once committed every decision re-homes every spare armed unit
to the target, so units go forward one at a time as they finish; the
copied bots gather to a threshold and send the whole force, and the
fix is a wave, a send only when the force at staging beyond its
garrison meets the threshold, nothing forwarded between waves (owner:
left for the balance pass). And the turtle personality is to be
considered for removal (owner, 2026-09-08), since it is only a
slower expander now and nothing in it says hold what you have; one
personality is the deletion. Two more from the owner's play
(2026-09-09), both for the same balance pass and neither urgent: late
in a match a bot stops spending and sits at capacity on every material,
building almost nothing and never attacking, and the fight the owner
expects never comes. Measured over expand against expand at the full
clock: both seats peg all three materials from about ten minutes to the
end, seat 0 spending 36 a second against builders that could spend 687,
with 25 armed units among 549 entities. The harness already holds the
guarantee that catches this, and it fires at the full clock, naming
552s to 612s; `check.sh` runs `harness verify 6` for speed and the
behaviour starts after that window closes, so the gate has never seen
it. Turning the gate up to the full clock makes it red until the bot is
fixed, so the two go together. And a bot opens frames for many
rows at once where it should open them one at a time, since build
effort splits evenly across the frames at an asteroid and an extractor
yields nothing until it completes, so N frames in parallel finish an
extractor in N times the span one alone would take and the income
lost is real. Serial beats parallel for any row whose yield starts at
completion, which is the whole economy; the same shape is present late
game and matters less there. Found by the agent, a sim item: `Composition::builder` is
a yes or no while the bot needs the count of builders standing, and
the flag disagrees with the row counts for a builder whose send is
forming; the count belongs on the composition beside the flag.

The unit was one Opus agent on the owner's ruling to copy the
mechanisms of five open-source bots read from their source
(CircuitAI, Petra, OpenRA, M27AI, Wesnoth; clones in the session
scratchpad, the research report in the session). Measured before it:
no bot ever fielded a warship, since the army value split by share
and divided by cost truncated every row to zero; twelve frames sat
open at asteroids where no builder stood because a claim closed on
the first structure and the constructor left; both seats sat at
stockpile capacity from five minutes. The design: the army
fills the most-behind row one whole unit a decision; spending holds
back only inside a band of 1.5 to 1.1 of the enemy's armed value and
a material above 0.8 of capacity always buys the cheapest armed row;
a want stands only where a builder stands or is arriving and is
swept otherwise, the claim closing when nothing more is wanted there,
open frames capped at two per builder; expansion is triggered by an
extractor that could not be placed for lack of spare cap, no count
per personality; forces per asteroid (owner, 2026-09-08: there is
defence, and with no lanes a single army is wrong), every held
asteroid wanting the enemy armed value at or toward it times the
defence ratio floored at a garrison, the staging asteroid adding the
offensive force, the fill feeding the most-behind asteroid's
most-behind row; attack re-homes the staging force only, joining an
attack already committed, the ratio falling to zero at two thirds of
the clock; the plan as the target composition per asteroid. The
demolition was reviewed 2026-09-08: the guarantees fail on the
shipped bot, the first in the opening second; the match runner and
the guarantee checks become library items shared by the tests and a
`harness verify` command since a fifteen-minute match takes ninety
seconds in debug. Three
guarantees pinned first as failing tests: every frame has a builder
standing or arriving; two bots field an army by a third of the clock
and exchange shots by half; no stock sits at capacity for a minute
while an armed row is affordable. The harness takes four names. The
agent stops at red for the demolition review.

Landed and committed 2026-09-08 (the bot commit the owner allowed in
advance): the bot rebuild, four managers and one funding pass, the
registry of shipped bots, the contracts test per bot, the tests from
the real start, the decision cap from the sim, the reaping fix. Fight
numbers measured against the old holding rule for the combat unit's
constants: fights at 2 to 4 asteroids in fifteen minutes; armed units
of one seat at one asteroid max 23, median 5; of one row max 11,
median 2; always two seats; zone 30 m, ranges 3, 6, 14 m. Chosen from
them: stage offset 15 m outward, station spacing 2 m, stage width 20
m along the radial (a rank of ten). Balance left open: expand grows to
fifty asteroids and arms about seventy units; the wave fires at an
undefended target with whatever stands; the turtle is crushed. The
plan's own cap on commands is left to its test, not INVARIANTS.md.
Found by it at HEAD and fixed in the tree by the overseer: the store
listed the dead in store order and the ready weapons searched that
list as if in id order, so some dead units kept their weapons and
fire looked one up seven minutes into an expand-against-turtle match
and panicked. The dead are listed in id order now, and a test pins
that reaping takes every dead unit's weapons whatever order they
stand in; it fails on the old code. Goes into the bot unit's commit.

For the owner's morning ruling, from the combat unit's build: the
fight's `Stage` shares its name with the draft's `Stage` (DESIGN.md
uses the word for both); one wants renaming. The stage's offset became
a rule after the build showed a fixed 15 m put every row's station out
of range of the asteroid (a lancer's station 22 m from an asteroid it
hits at 14), stalling sieges and failing the turtle's contract: it is
half the roster's longest damage range outward, 7 m shipped. The look
tool's fight scene is a hand-placed fixture, not a stepped state, so
it cannot show a pass or a rank; driving it from a stepped state, or
putting the stage on the view so the display draws stations and
lines, is a display unit. Balance: the wave fires at an undefended
target with whatever stands.

Built overnight, uncommitted, for the owner's morning review: the
combat unit. The sim is green end to end (208 tests, gate clean); the
tree is 164 lines smaller than HEAD; the tick is level with HEAD.
Rulings the overseer made on the owner's delegation: the stage's
offset is half the roster's longest weapon range (7 m shipped), a
rule not a number; a row runs outward from the stage along the
lateral, away from the star, so no station crosses the asteroid;
spacing 2 m and width 20 m from our fights' numbers. Two bot
guarantees fail under the new fight (a want at staging outliving its
builder for two decisions; expand's income falling because the fight
now destroys extractors); routed to the bot agent to fix in agents/
only, the growth guarantee restated as never idling rather than never
falling. Both fixed by the bot agent: the builderless frame was the
offence manager re-proposing a wave every second while its units died
at the target, now a wave goes only once the last has landed, which
makes "nothing forwarded between waves" true; the growth watcher
counts extractors destroyed and breaches only on a minute's fall with
none lost and a free asteroid in reach. The whole gate is green on
the overseer's own run, all four guarantees; the diff against HEAD
is 25 files, 568 in, 656 out. Traces on the new fight: expand takes
21 asteroids against a turtle where it took 53 and the turtle fields
a real army (6935 value); a four-seat match drives one turtle off
every asteroid. Balance, not defect: offence still arms one unit at
one staging asteroid while economy spends on twenty. The fight
screenshot is a hand-placed fixture and cannot show a pass or a
rank; a display unit drives it from a stepped state. Awaiting the
owner's review and word to commit.
Landed and committed 2026-09-08 (e4e8cf5): the transfer unit. The two-impulse coast is replaced by a rendezvous
rule: a flier thrusts at the movement limit along the difference
between its relative velocity and the velocity it wants, straight at
the destination at the speed the limit can stop from over the
distance left less a 1 m/s margin, exactly the difference where a
tick would overshoot; it arrives within tolerance and never orbits
(the switching rule first ruled chattered on its own switching
surface and sent fliers past their target; the owner ruled the
correction into DESIGN.md). Measured: 2 km in 9.97 s against a least
of 9.88, the belt's widest 47 km in 53.9 s against 48.6, the extra
being the destination's own travel; no tick above 1.9 ms over fifteen
minutes, the 50 ms send spikes gone, the late tick 0.68 ms. The sim
lost 288 non-test lines and 360 of tests, INVARIANTS.md three entries.
Friction to carry: `World::steers` in the fixture copies the step's
movement half and drifted silently this unit (a movement pass both
call would delete it); `Berth` reaches the surface by two paths,
`state::Berth` and `state::view::Berth`. After the owner's play
(2026-09-08): the flight line runs to the destination where it stands
now, not where it will be, so the line and the asteroid move together;
the arrival estimate, which nothing then read, is deleted from the
sim, the view and both documents; and a flier aims at the rim of its
destination's zone on its own side, one zone radius from the body
toward the flier, so an arriving force meets its destination at the
edge facing where it came from and is not surrounded on arrival
(DESIGN.md Sends carries the sentence). Gate green after each. Deletes the
schedule, the send search, the Lambert solver, fulfilment's
three-stage split, the forming window and the leaving state, three
INVARIANTS entries and the unbuilt flight-line sentence, about a
thousand lines (the owner hoped for two; the preview and the send drag
stay). Why: the far solve cost 41 ms on the tick a send began, 81
candidates flown through four correction passes that could not
converge for spans where the burns' finite length put the ship
hundreds of metres off the impulsive arc; gravity at the belt is
0.17 m/s² against a limit of 80, so a transfer is straight-line
kinematics and the coast only doubles its time. The owner chose the
rule over an aim fix (C) and an analytic schedule (B). DESIGN.md Sends
is rewritten in the brief's words; the display estimates arrival by
the same formula.

Next after the transfer unit lands, ruled 2026-09-08: the bot
rebuilt as managers that propose and one funding pass that decides,
on the owner's word that the bots sit doing almost nothing and that a
ratio knob would hide the defect. The defect: the plan runs its rules
in a fixed order with priority implicit in the order, extractors
sized to the builders' demand and yards to a personality count, so
income meets spend and the bot stops; the band and the overflow spend
were bolted on to force spending past that fixed point. The shape
(Petra's two phases, OpenRA's unbounded appetite, CircuitAI's growth
loop): four managers read the survey and propose, never issue, each
proposal a posting, a count above what stands, a priority and a
reason. Defence proposes a garrison at every held asteroid an enemy
force stands at or approaches, sized by it. Economy proposes an
extractor at every held asteroid with spare cap in a material the mix
uses, gated by payback, and one more yard wherever builders are short
against income, both uncapped. Expansion proposes a claim, a
constructor want at the best free asteroid, whenever spare cap at
held asteroids falls under the mix's demand, up to the claims in
flight. Offence proposes one more armed unit at staging by the fill
rule, always. One funding pass sorts by priority and funds each
proposal whole or not at all against stock plus income over a short
horizon less what open frames still need; a funded proposal is the
want, an unfunded one falls to what stands. The stock is always spent
to the bottom, so the band, the overflow spend, the yard count, the
want/keep/promised/affordable bookkeeping and the builderless sweep
are deleted. The personality is the priority order and three numbers:
expand funds economy, expansion, defence, offence with a small
garrison floor; turtle funds defence, economy, offence, never
expansion, with a large garrison. Priority moves with state: defence
above economy under threat; offence below economy when own armed
value exceeds the enemy's by the band's ratio, the two numbers kept.
Attack stays CircuitAI's, in one wave. A fourth guarantee: a lone
expand bot's income rises until the free asteroids within reach are
taken, and a turtle's holds; the three existing guarantees stay.

Landed and committed 2026-09-08 (a76429a): the store unit. Measured on a fifteen-minute two-bot
match in release: the tick grows about quadratically with entities,
0.13 ms at 134, 1.7 at 480, 4.2 at 781 against 8.3 available, and
the engine catches up at most eight ticks a frame, so with a frame of
six to eight milliseconds (0.4 ms empty, 5.8 at 400 ships, 7.6 at the
whole belt, offscreen) the clock falls behind. The game's own work
around the step is under two percent. The flame graph (session
scratchpad, late-game-flame.svg): the entity tree's own key search
and comparison a quarter; `standing_at` walking the whole store per
call a quarter, called per unit by the chase and per asteroid and
material by extraction; the chase's threat ranking per unit 30
percent; the field sums 18; the sweep 17; fire 12. The unit: a dense
entity store in asteroid-then-id order with ids minted by the store,
so who stands at an asteroid is a slice and a lookup cannot fail;
the chase ranked once per asteroid per plating; the sweep deleted in
favour of the slices; asteroid bodies solved once per tick; one hash
re-baseline; every downstream reader (view, agents, game) propagated;
timed before and after. Opus deletes, Fable writes it back (owner).
Cuts ruled in (owner, 2026-09-08): no separation in flight; the
global sweep deleted. Kepler stays: one solve per steered unit and
per structure lookup is 0.7 percent of the profile. Ruled out for
now: fields by moments. Encapsulation and organisation, ruled with it
(owner, 2026-09-08): the store's vector, order and id index are
private to one type, `Entities`, whose surface is the slice standing
at an asteroid, the fliers, one entity by id, spawn and reap; every
phase reads a per-asteroid roll built once a tick (the slice, the
asteroid's body, the units by seat, the threat ranking per plating)
and cannot reach what the roll does not carry; the state's queries
shrink to what leaves the sim and the Index impls go; one concern per
file, phase files hold rules only; the sim's line count ends at or
below where it started. Layout ruled (owner, 2026-09-08): columns
now, one vector per field ordered by standing place, seat, then id,
the entity a view over them, flights in a sparse side table, a
per-tick asteroid body column; the demolition landed on Opus at full
red (472 lines out, 33 in) with a critique the rebuild answers:
home-keyed readers served by an in-transit index of entities whose
home is not where they stand; `entity(id)` infallible and the id
field private; the fields' x-sort over a borrowed index with bodies
from the roll; `surplus_at` and `Send::forming` made explicit in id
order; `Ready` reshaped so reaping and lookup are not scans; the
damage `Assigned` renamed. The rebuild landed on Opus (owner,
2026-09-08: a Fable agent read to 200k context before writing and
was stopped; briefs now carry a reading rule, errors and diff first,
a file only when about to change it): the tick at 781 entities is
0.85 ms against 4.2, gate green, replay and rollback agree. Rulings
on it (owner, 2026-09-08): the sim's non-test budget miss of 332
lines accepted for the speed and the private store; shots carrying
their own exchanges from the tick they fired accepted, since that
was the one id that outlived its entity across the surface; the
`Default` on `EntityId`, added so a game test could mint one, is
replaced before commit; and the send solve, 25 to 54 ms on the tick
a send is solved, is investigated to its root with the target under
one millisecond, before the field sums (19 percent), separation (17)
and propagation (10) that remain. Awaiting the owner's word to
commit. After
it: the roster's rows as a closed enum (owner, 2026-09-08), so every
roster lookup is infallible and the `Option` every reader of a row
carries is deleted; its own unit across every crate. The field sums
are quadratic by design and wait for a DESIGN.md ruling.

Held for after the bot fights: tiers, ruled 2026-09-08 to be designed
in parallel and to land only once the bot fights. The mechanism as
proposed: a builder builds a structure up to one tier above its own
and a unit up to its own, read off the tier every row already
carries; a frame opens only where a standing builder can build the
row, else the want stands dashed as a want with no builder does; the
reserve and surplus ignore tiers; no upgrade verb and no level on an
asteroid. Costs the gate in fulfilment, the wheel's dashed phrase,
the bot's roles per tier, and a re-stated roster (a tier-two yard and
constructor; the lancer's tier).

The next units after it, in the owner's order, each a design
conversation on the owner's questions before a brief:

1. Grouping icons at far zoom, the conversation opened 2026-09-08:
   the owner ranks what a pile must read as who, what, where, how
   much, with small position offsets allowed and some rows always
   visible; measured that a force is a 60 m disc on a 50 km belt, so
   above fight zoom every force is one glyph, and that the spacing
   (0.5 m) is under the drawn side (1 m) so bodies overlap at world
   scale too. Three shapes offered, gather-by-row recommended; the
   owner noted the torn-down tally wheel and BAR's no-overlap footprint
   and asked whether grouping is needed at all. No ruling yet; the
   belt's size may change first.
2. The camera's focus as an orbit, the sim's own type: clicking an
   asteroid sets the focus to that asteroid's orbit exactly, a pan or a
   zoom into empty space makes a circular orbit through the panned
   point, the hand-rolled turn in `advance` and its rate cap are
   deleted, the soft floor on the pan stays (agreed 2026-09-08).
3. The spectator: a host who holds no seat watches read-only and can
   change which seat, through a panel like the draft's.
4. Bodies as 3D models with the glyph as a screen icon over them, and
   a placeholder mesh per row through an enum the catalog matches on.
5. Ship speed and combat feel through the harness; then hotkeys.

Numbers of the display unit, each a first value to tune by play:
asteroid floor 6 pt, entity side 1 m floored at 26 pt, resting marks
whole at a zone of 2 pt and gone at a quarter, fight bars whole at 4
and gone at 0.2, the yield mark's floor 5 pt and its thickness three
tenths of its stand-off floored at 12 pt, slivers under 2 pt not
drawn, the wheel's stand-off the zone plus the bars' gap floored at
the bodies' floor, the structure ring a spacing off the surface at an
eighth turn a rung. Left for the contraction audit: the soft pan sits
on a hard clamp; `Orbit::tilted_ellipse` takes seven bare numbers;
`Vec3::dot`, `Vec3::normalized` and `Belt::SPACING_METERS` went
`pub` for the display; `Stage.placed` records where a pick landed
beside the body that stands there; `Scene` copies five belt constants
through `View`; `EntityView` copies the row's reach and glyph per
entity. Ambient and the starfield are engine features the owner adds
in parallel.

Open from the time split: extraction, construction and fulfilment act
per tick with a fixed second's worth of work rather than integrating
the match time that passed, so the step still returns early during the
draft. The complete shape is every phase taking the step's match-time
span, zero in the draft, and no early return. Contraction audit.

Open from the draft unit:
- `harness draft` on the seeded belt with the rebuilt bot
  (2026-09-08): the first picker won 5 of 8, none drawn, and both
  sides held two asteroids at ninety seconds in all eight; the pick
  is now the richest free asteroid on every seed. The first-picker
  edge is a balance reading, not a defect.
- The owner's arrival scene, the probes flying into the system during
  the draft: display only, not yet designed.

## The construction overhaul, ruled 2026-09-09, not yet begun

Designed with the owner across one session and ruled item by item. It
begins only once the combat work is committed, since its first unit is
atomic across 41 files.

Why: with more rows the build menu clutters; the player has too few
real choices; and nothing makes scaling wide cost anything against
scaling up. The third is measured, not suspected. Expand against expand
at the full clock ends with 44 asteroids, 437 extractors and 25 armed
units, so a bot that optimises the win rule never fights.

The rules, as ruled:
- An asteroid gates what may be built there. A want for a row the
  asteroid does not allow is refused by name.
- A row states what it requires, what it excludes, and one row it
  replaces. Replacing implies requiring. Requires means every named
  row, not any one of them: a conjunction makes a branch fragile, and
  fragility is what makes a structure worth killing and worth
  defending. Transitive existence through the replaces chain covers the
  ordinary upgrade so a retrofit does not un-build the fleet.
- Nothing distinguishes a structure from a ship. A requirement names a
  row and any entity of that row standing there satisfies it, so a
  tier two constructor ship unlocks better defences by being required,
  and a capability can walk: send it away and the asteroid falls back.
- An asteroid supports one shipyard, whoever built it first, so a cheap
  shipyard is a denial play and taking a rock means killing it.
- Labs are the horizontal axis and are not capped. Tiers are vertical.
  All the pressure against building everything everywhere rests on
  shipyard exclusivity and retrofit cost; if the late game drifts to
  one super-rock, that is the dial.
- A destroyed structure drops its asteroid back down the tree, and
  every frame of the rows it granted cancels and refunds that tick.
- A tier two replaces its tier one by retrofit, paying the new cost in
  full with nothing returned, tuned expensive. It needs no new verb:
  wanting the tier two where the tier one stands opens a retrofit
  frame, and the tier one keeps standing and granting until it
  completes.
- A variant is an ordinary row carrying a mark as it carries a tier. A
  lab may remove a row and add another. Built things never change.
- The row becomes `EntityPattern`, one enum variant per kind of entity,
  with every stat a const method whose body is a match. No wildcard arm
  at the root of any of those matches: the wildcard turns adding a
  variant from a compile error into a silent default, which is the
  failure this codebase most needs the compiler to catch. Or-patterns
  listing variants replace it.
- `Weapon` becomes `Effect`, since build and extract are not weapons.
- The wire encodes the enum as a `u8` through `serde(into, try_from)`,
  following `protocol/src/record.rs`. Without it serde writes the
  variant name and a want grows from three bytes to eighteen. The
  `try_from` is what deletes `Rejected::NoSuchRow`: an unknown row
  stops being a command the sim rejects and becomes a frame the wire
  refuses.
- Factions are left open (owner, 2026-09-09), to be decided when
  factions are built.

Measured blast radius, read from the tree at e3dc9a9: 41 files mention
`RowId`, 107 sites in sim, 58 in agents, 32 in game, 4 in protocol,
plus 110 reads through the roster or state index. `.weapons` is read in
8 places, so the `Effect` rename is small. The roster sits inside the
hashed `State` and `Draft::of` reads it during construction, so taking
it out re-baselines every hash the repo pins.

The units, in order, each ending green but the second:
1. `Weapon` to `Effect`. Eight sites.
2. The enum, atomic. Deletes `RowId`, `Roster`, the nine row
   constants, `get`, `len`, `Index<RowId>`, `units_by`, `NoSuchRow`,
   and the roster's place in `State` and its hash. Every one of the 41
   files breaks the moment the row stops being indexable, so it cannot
   be staged gradually.
3. The three relations and `satisfies`, with every relation empty, plus
   the gate in `apply` and the retrofit at frame completion. Ends green
   with behaviour unchanged, because an empty tree refuses nothing.
4. Fill the tree. Roster design, not code, and not started.
5. The wheel becomes asteroid-dependent; the glyph gains the mark; the
   asteroid-state glyph lands.
6. The bot learns the tree.

Open inside it: what the construction turret is, since builder and
constructor are both taken; and which visual channel carries the mark,
since the glyph already spends its silhouette on the role and its base
on the tier.

## Measured this session, each needing a unit or a ruling

- Four teams still interleave. Worst pair of enemy stations over the
  armed rows: 2.000 m at two teams, 1.732 at three after the line cap,
  0.707 at four. Separation only pushes inside 0.5, so at four teams
  two lines sit inside each other with nothing parting them. No cap of
  that shape reaches further, because two adjacent lines meet at a
  corner and the stations either side of it sit half a station spacing
  apart whatever the line's length. Fixing it means changing how sides
  divide the circle, which is the owner's.
- A stationed frigate cannot reach the structures on its own asteroid.
  Its station stands about 7.6 m off the body and it reaches 6, so it
  connects only while running, and spends 4.8 s of a 10 s cycle out of
  range. Short rows can besiege only in passing.
- The gate cannot see two of its own guarantees. `harness verify 6`
  ends at 360s; the hoard guarantee fires at 552s and the
  builderless-frame one at 675s at the full clock. Both pass at six
  minutes with the behaviour present. Turning the clock up makes the
  gate red until the late-game stall is fixed, so the two go together.
- The pass now bottoms out at the enemy's reload, 30 ticks for a raider
  and 60 for a frigate. Chatter is gone, no two-tick pass anywhere
  against 94 percent before, but the median carried distance in a duel
  is 0.55 to 0.76 m, near the 0.5 m at which a station counts as
  reached. So the approach reads as a charge and the sustained fight as
  jitter about the station. The lever for a slower pass is a weapon's
  reload, not a new constant.

Four nits from the blind review of the pass rule, recorded rather than
folded into the unit that found them, each for whichever unit next
touches its file:
- `the_rule_reads_only_the_tick_it_is_given` runs Holding twice with an
  empty `Shots` both times, so the one input this change added is
  absent from the test that pins the phase to its snapshot. Give it a
  runner with a prey and a `Shots` carrying that prey's hit.
- `a_short_range_unit_turns_for_its_station_no_sooner_than_its_prey_reloads`
  stands one frigate against one. With two enemies of unequal rates the
  runner can re-target between turns, since threat is damage over hit
  points and a damaged enemy rises, so no single row's reload bounds
  the gap. The name claims more than the world it runs in.
- `HeldUnit::place` looks the station up, then `passing` looks it up
  again through the roll. Hand the first one down and the `None` case
  becomes unrepresentable in one place instead of two.
- `passing` opens with `runs_passes().then_some(())?`, a value computed
  and thrown away where a plain early return says the same thing in the
  register the rest of the module uses.

## Open, each for a ruling or a unit

Ruled and queued (owner, 2026-09-09), the unit after the combat fixes:
unarmed rows circle the rock. Found by the owner in play and measured:
an unarmed unit arrives at the rim and never moves in, because the
return term is exactly zero anywhere between the asteroid's floor and
the zone, and stations go to armed rows only, so a constructor keeps
wander and velocity damping alone. It lands 30.01 m off and sits at
29.60 m twenty-two seconds later. The fix is a station, not a new
steering term: every unarmed unit takes a place on a circle about the
asteroid's body, at the floor plus one station spacing, on a plane
whose normal is drawn from the unit's own identifier, so two builders
never share a plane and a builder keeps its lean wherever it goes. The
circle turns at a quarter of the speed that row's manoeuvring limit can
hold at that radius. One ring per unit, not one per rock: two builders
never share a plane, so nothing has to space them along an arc and
separation covers the crossings. A builder still reaches the whole zone
to build; what changes is that it comes in from the rim and can be
killed.

Ruled and queued as the unit after that (owner, 2026-09-09):
the shield holds its ground. A passing unit runs and does not break
while an enemy stands within that enemy's own weapon range of an
unarmed structure of the passing unit's side. Without it a defender
that breaks off empties the enemy's range of armed targets and hands
over its own refinery for the seconds it is away, since an unarmed
structure has no threat and is shot only when nothing armed is in
range. Build it as one fact per asteroid per team per tick, computed in
the pass the roll already makes over its standing entities and read as
a flag by every passing unit, never as a check per unit; it costs
nothing at an asteroid holding one seat. The owner asked to be told if
it costs more than noise: the tick at the last commit runs 0.30 ms at
p50 with 1289 entities against a budget of 8.33 ms, so there is room.
Queued behind it, a roster item: turrets, an armed structure. A turret
carries threat, so it is a normal target and it frees a garrison from
having to stand still to shield what it guards.

From the owner's play (2026-09-08), each for a ruling before a unit:
- The mouse wheel over a wheel's line sets that row's want, as the
  buttons do. A DISPLAY.md Editing change; kept beside the hotkeys
  item.
- The draw order (owner, 2026-09-08): the world by depth, then on the
  screen layer the zone and range circles, the yield marks, the flight
  lines and the fight bars, then the bars and the wheels with the hover
  phrase, then the panels; fight bars and flight lines stay under a
  wheel. The yield marks paint over the bodies today. Bodies will one
  day be 3D models, with the glyph then a screen-layer icon over the
  model as strategic icons are; a placeholder mesh per row, a cube,
  through an enum the mesh catalog matches on, is a unit of its own.
- A host who holds no seat and plays only bots is shown the first
  bot's seat as if it were their own: its stockpile bar, its wheels with
  buttons that issue nothing, its draft turn (found 2026-09-08). Ruled:
  such a player is a spectator, read-only, able to change which seat
  they watch, through a panel like the draft's. A unit of its own.
- The wheel's wrapped columns stand about 130 px apart in the look
  tool's wheel and draft scenes, reading as three groups rather than
  one wheel. A small display item.
- The naming pass over the sim crate (owner, 2026-09-06), the owner
  judging it from the diff: read-only Haiku surveyors in parallel, one
  per module group (state, step, orbit and roster, history and the
  rest), each returning a table of symbol, file, one sentence of what
  it holds, and the proposed name under the naming rule; the overseer
  filters, the owner rules on the list, one Sonnet applier renames
  workspace-wide under the gate, one commit; the other crates follow
  if the sim's proposals read well, else the survey moves to Sonnet.
  Deferred behind the visual and gameplay units (owner, 2026-09-07).
- No look scene hovers a plus or minus button over the stockpile bar,
  so the cost and refund segments are pinned by test and unseen; a
  look scene for each is a small display item, after the owner's play
  has judged them.
- Tuple keys outside the preview unit's files, about thirty maps: an
  asteroid with a seat as a bare pair in `step/fire.rs`,
  `step/extraction.rs` (`Income`), `display/fights.rs`; a row at a
  post as a pair or triple in `agents/plan.rs` (`targets`,
  `standing`); and tuple returns in `display/glyph.rs`,
  `personality.rs`, `harness.rs`. One Sonnet sweep
  under the tuples rule, deferred behind the visual and gameplay
  units.
- Second derivations in the agents, found 2026-09-06: `survey.rs`
  tallies the seat's holdings from `view.present` where the view
  already carries `compositions` from `State::holdings`; with the four
  rank sorts under 1e. A Sonnet unit after the preview.
- Ship glyphs stairstep: the quads use an alpha-test cutout MSAA cannot
  soften. Ruled: a coverage-sampled glyph rasteriser, edge coverage in
  the sheet's alpha, no material change. A small display unit.
- A thousand units at one asteroid cost 8.3 to 8.5 ms a tick against the
  8.333 ms budget (release, 21-asteroid belt); rank the roll once per asteroid
  and plating. The state is cloned whole every tick and 240 clones are
  kept (`Retention::shipped`, every tick over a two-second window), so
  every field added to the state is copied 120 times a second; the
  cadence constant already exists and a rewind of the whole window
  costs 26 ms at 100 entities.
- Time (owner's worry, 2026-09-06): `TICKS_PER_SECOND` is 120 and the
  game paces the sim at that rate of wall time with no speed control;
  verified 2026-09-06, no time scale exists in the game. Every "per
  second" in the sim is a sim second; if a speed control is added,
  rates stay sim seconds and only the pacing changes. Into the
  contraction audit: the 49 non-test uses of `TICKS_PER_SECOND` in the
  sim against one `Tick` vocabulary.
- Panics by raw index (owner, 2026-09-06): `Index` impls on `State`
  and a handful of `as usize` lookups panic on an id the store did not
  mint. Structural fix for the contraction audit or the columns unit
  (plan 5): stores whose ids are minted only by the store, so an id in
  hand is valid by construction and no lookup can fail.
- Dead `pub` items rustc cannot see: a `pub` item in a library crate is
  never flagged unused, so a crate's surface hides dead code from the
  gate. The sim is done: every item outside `lib.rs`'s re-exports is
  `pub(crate)` and rustc's lint now finds the rest. Remaining, by
  count of `pub` items: protocol 64, agents 38, game 375. Into the
  contraction audit (1e); `check.sh` gains the scan until then.
- No panics anywhere in the game or the sim (owner, 2026-09-06,
  distant): every panic path becomes error propagation and handling;
  with the id-minting stores above and the raw-index item. A crate-wide
  audit, after the belt.
- Camera controls are poor (owner, 2026-09-06); future work, no unit
  yet. Kept beside the hotkeys item since both are input.
- Hotkeys: the wheel's bands take the pointer only, and strategy players
  require hotkey play; the wheel's shape makes a binding per row and per
  step hard to add. Kept in mind for every wheel change; no unit yet.
- Whether a flying unit can be re-sent, its schedule solved from its own
  body mid-flight, when its destination's want falls, or whether a send
  is a commitment. A design ruling.
- Standings in a match: a page over the match toggled by a key with
  per-team standings, shaped as Beyond All Reason's stats page. A design
  conversation on its content before DISPLAY.md gains it.
- Test worlds start from the real game start (owner, 2026-09-08): the
  owner does not want tests building states the game never has. The
  agents' fixture is being moved to `Setup::new` and `State::start` in
  the bot unit; the sim's `World::ring`, `World::seated` and the
  hand-set stocks and caps its 200 tests build on are the same defect
  at a larger scale, a unit of its own after the fight rules land.
- The gate's verify now plays a fifteen-minute match for the growth
  guarantee and takes 34 seconds, up from seven; if it grows again,
  the growth guarantee moves out of the gate to a pre-commit command.
- Bot refinement by rating (owner, 2026-09-08): after the bot's shape,
  the fight rules and the harness's balance questions have landed, not
  before, since a rating measures bots on one set of rules and folds
  the rules' balance into bot strength. First a round robin against a
  frozen reference over many seeds with the personality's numbers as
  the variables and a confidence interval; Elo once there is a
  population of personalities and variants. A fifteen-minute match is
  about ten seconds in release, so a few hundred matches is an hour.
- Fable's wheel proposals, not built, for the owner: asteroid names in
  phrases; total HP on the fight bar's hover; a live send line from
  wheel to pointer; the hint phrase advancing.
- The holding rule, from the owner's play (2026-09-08): a constructor
  with five raiders on it runs out of the zone under its caution term,
  the raiders' chase pulls only toward an enemy inside the zone, and
  the return term holds the constructor at the zone's edge, so neither
  reaches the other. The owner's three changes to put as mechanism in
  the combat-feel unit: a unit steers toward a target direction rather
  than by a sum of forces, so a chase closes; a slower row does not
  chase a faster one; a defending force holds near its structures. A
  DESIGN.md Movement and combat change before the unit.
- Held for the owner's play: asteroids sub-pixel at region zoom; the fight
  arc refilling on reinforcement; repair at 15 HP/s beating a frigate's
  12 DPS; elimination before the clock; a fresh-eyes judgement after
  every display change; ships too slow and combat slow to start and
  random-feeling in outcome, both weighed by the harness.

## Docs ahead of code

One line per sentence of DESIGN.md or DISPLAY.md the
code does not yet do, naming the unit that lands it. A brief quotes its
lines from here; landing deletes them; the overseer reads this section
against the code at every session start.

- DISPLAY Resources: an extractor wanted or building at an asteroid shows
  its coming yield on the asteroid's bar; not drawn. The extractor-coming
  unit.
- DISPLAY Flights: the line ahead of a ship follows its schedule's
  path; the code draws a straight line. The extractor-coming unit.
- DISPLAY Resources: the small state is bars alone, tight to the
  asteroid; the code draws icons at both states. The extractor-coming
  unit.
- DISPLAY Lobby: no Seat column; the lobby still draws one with the
  seat's number in a square of its colour (screens/lobby.rs). Plan 6.
- DISPLAY Lobby: the team choice shows a square of the team's colour in
  the closed control and the open list. Not built. Plan 6.
- DISPLAY Rings and Lobby: colour is the team's everywhere; every
  painter colours by seat through `seat_color32`. Plan 6.
- DISPLAY Title: Settings is not drawn; Quit closes the game on the
  desktop through `ctx.close` and is not drawn in the browser. Both
  drawn disabled with a reason (screens/title.rs). Plan 6.
- DISPLAY Play: Surrender is not drawn; drawn disabled (screens/pause.rs).
  Plan 6.
- DESIGN World, Entity and Sends: one movement limit; the sim solves one
  schedule per row from a per-row acceleration. Plan 1e or the belt.
- DESIGN Start: a seat whose draft stages lapse may place its reserve
  from the free asteroids at any later tick; the code never places it
  afterwards (found by the bot unit's tests, 2026-09-08; unreachable
  while a bot holds its own seat). A draft unit or the columns unit.
- DESIGN Movement and combat, Chase: a unit chases an enemy inside the
  zone; the code's chase targets any enemy homed at the asteroid
  wherever it stands, and both the chase and the return term saturate
  at the same speed past 7.5 m, so a chase with a heavier weight than
  return pulls a unit out of the zone without bound and a caution-driven
  unit runs until the enemy's field vanishes 15 m off (found by the
  owner's play, 2026-09-08). The combat-feel unit, with the owner's
  three changes listed under Open.

## Plan, in order

Priority (owner, 2026-09-07): visual and gameplay changes first, the
belt since its asteroid and ship counts set every other number, then
the readings the owner needs to judge by play, then the contraction,
then measurement before any store rewrite, then the screens, then the
programme.

1. The belt and the star (item 3), then ship speed and combat feel
   through the harness, then the asteroid bars and flight line unit
   (Docs ahead of code), then camera and hotkeys, before the
   contraction audit.
1e. Contraction audit, read-only then units: the codebase is much
   larger than it has any right to be for this amount of game. Survey
   for types whose fields copy another type's, parallel indexes over
   what the state answers, per-tick bags of borrowed values. Each
   finding becomes a deletion in the unit that next touches its file,
   or its own Sonnet unit with a ceiling. Known: `display::scene::
   Client` passes two of Scene's own fields through a constructor from
   four call sites; four identical rank-by-score-then-id sorts in
   agents/plan.rs and roles.rs.
2. Vectors, Sonnet: the connection's inboxes and both websockets'
   queues gain a stated cap on frame count, past which the connection
   closes; the kept records as a deque; the agent memory's asteroid sets as
   sorted slices; about 75 never-mutated fields and returns become
   `Box<[T]>` and `Box<str>` at their constructor sites (protocol 6,
   server 9, sim 19, agents 12, game 15). Net lines below zero.
3. The belt and the star, Opus: map generation from seed with regional
   caps; one to two hundred asteroids in an annulus from the seed with
   regional cap triples; neighbour spacing near two kilometres, the
   extent growing with the count; the star in the middle of the system
   as the central mass drawn, and a starfield skybox, both engine
   questions first since the engine deleted its sky; the lobby's seed
   changes the belt and its preview is the seed's belt at whole-belt
   zoom. The zone radius tens of metres, from the largest force an
   asteroid holds at the holding rule's spacing. Settles the asteroid
   count, the belt's spread against the schedule search bound, and the
   ship count a match reaches, which items 4 and 5 answer to.
4. The harness for scale and truth, Sonnet: a scale check playing a full
   bot match at two and five thousand ships on a release build printing
   milliseconds per tick; a per-tick invariant mode checking while a
   match plays (no thrust above a limit, every flier on an unended
   schedule, wants against holdings after fulfilment, a re-stepped
   tick's hash against the kept one, stock within capacity, both
   machines' settled hashes) naming the tick and the rule on a
   violation, extended with bot-behaviour guarantees (a bot builds an
   army and attacks when it beats the defence); the win-loss matrix
   against an expected table as an ignored test or a harness command
   run before a sim commit and after a balance change, the gate kept
   fast. With it the test audit: every sim test mapped to the DESIGN.md
   sentence it pins, read-only report first, then the tests with no
   sentence deleted.
5. The entity columns, Opus, on item 4's number: the store transposed to
   one column per field over a dense position with a sorted id index,
   an entity a view over the columns, iteration in id order, `Vision`
   built once per step, manoeuvring folded into propagation,
   `PerSeat<T>`; ready moments onto the entity; one hash re-baseline.
6. Screens on egui's own layout and widgets under one `Style`, driven
   headlessly through `Session::offer_ui`; deletes `control.rs`,
   `field.rs`, every `Places` and width constant; the lobby loses its
   Seat column and the team choice carries the team's colour square;
   colour is the team's everywhere; a control for an unbuilt feature is
   not drawn; Quit through `ctx.close` on the desktop and not drawn in
   the browser; `set_tick_interval` for pacing instead of dropping
   steps; button text centred. Opus. Clears five ledger lines.
7. Held for a ruling after item 4's numbers: the solve leaving the sim
   as a stamped schedule command from the owning machine, validated in
   apply by one integration against the tolerance and the limit; then a
   planner memo by place pair and quantised phase. The overseer's view:
   right if a played match shows the solve in the tick's budget.
8. The structural programme, each unit updating INVARIANTS.md: the
   small collapses (View::want; Room folded into Socket; one
   belt-drawing preamble); a frame on its
   post; one asteroid type from sim to pixel; one mark state replacing Fill
   and Reason; Layout yielding placed marks and one Dial; mark geometry
   as primitives painted once and rasterised once; one Log type; the
   agent's knowledge as one row per asteroid; the wheel built once on
   selection; the Maneuver phase spelt manoeuvring; the units question:
   evaluate one existing dimensional-analysis crate with const-generic
   dimensions, adopt only if already in the cargo cache and its bounds
   stay out of the rules. Opus for the sim stores and the agents; Sonnet
   for the rest; a line ceiling per brief; `check.sh` gains a
   duplication check.
9. From play: a selected asteroid owns the focus each tick until a pan
   releases it (DISPLAY Camera, main loop).
10. From the harness: seat 0's edge isolated and removed; territory that
    varies with composition; combat before the last third of a match;
    the tick-rate-doubling check; timeouts instead of iteration caps in
    the slow tests.
11. Later: fog in the display; gamepad; the twelve-slot wheel; hulls as
    meshes with a level-of-detail rule, the stencil icon, and a DESIGN
    line that a faction skews the hull's dialect and never the glyph;
    Haiku playtesting agents; a room list over the server; nicknames on
    the wire; factions as skews over one roster.

## Operational

- The engine is a path dependency at `../../mirage-engine`; its wgpu 29
  and egui 0.35 pin is its own concern. Its verification recipes are in
  `docs/verifying.md` there.
- The browser: `game/tools/serve-web.sh` builds the game for wasm32,
  binds it with the wasm-bindgen CLI pinned by the lockfile, serves
  `dist/` with the engine's page; a WebGPU browser is required. No
  browser run has been made yet; the first is the owner's.
- Linear is not set up and is ignored.
- Crate downloads are blocked in the agents' environment, so a new
  dependency must already be in the cargo cache. In it: resvg and usvg
  0.45.1, tiny-skia (in the lockfile), kurbo.
- One implementation agent at a time. Every brief carries the rules
  below.
- Every file edit through the Edit and Write tools, never a shell
  script, sed or heredoc; scripted edits produce mistakes the owner has
  to correct. No relaxation for sweeps.
- No comments of any kind. A comment is replaced by a type or an
  invariant upheld at compile time wherever one can carry the fact. One
  test per guarantee a module makes to its callers, named as the
  sentence of that guarantee; no test per branch, field, helper or
  identity; a test no plausible wrong implementation fails is deleted.
  Tests share one fixture module per crate.
- Naming: a function is named by what it returns or does in the game's
  words, never `of`, `get`, `handle`, `process`, `run`, `update`,
  `helper`, `util` or a suffix; a type is a noun the design documents
  use; a test is named as the guarantee sentence, never with `test`,
  `works`, `should`, `check`. A confused agent is a naming defect. A
  name has as many words as a design-document reader needs to know
  what it holds without opening it, usually two, never capped; a
  design noun stands alone; a coined single word is a defect, for
  fields and methods too (owner, 2026-09-06).
- Tuples (owner, 2026-09-06): a tuple of three or more members is
  always a defect, and any tuple that keys a map, is a field, or
  crosses a function boundary becomes a struct with methods and its
  fields private where it can; a tuple inside one expression, such as
  a sort key, is fine. An asteroid with a seat is `Post`; a row at a
  post is `Posting`.
- Placement: a computation lives on the type that is its subject and
  there is one computation of each fact; before writing a derivation an
  agent searches for the type that owns the fact and extends it; a
  second derivation anywhere is a defect.
- Store rule for every sim brief: no rule holds an entity across a tick
  or indexes the entity store by anything but an id, so plan item 5's
  columns land without touching a rule.
- Three line budgets reported separately, code, tests and docs; the
  code budget below zero and the test budget below zero unless a new
  guarantee has no old test to replace. No document describes the
  code's shape (owner, 2026-09-08: ARCHITECTURE.md culled, since a
  description of code goes stale and agents read code); a tolerated
  runtime failure goes into INVARIANTS.md with its shape change, and a
  new dependency into its table with the reason.
- Words on screen: every user-facing string is a short phrase, no full
  stop, semicolon or dash; a comma is escalated to the owner before it
  is drawn (DISPLAY.md "Words on screen").
- Match setup, bots included, is the start menu's job; the playable
  takes no command-line arguments.
- Visuals: a Fable agent under the owner's direct steering through the
  overseer chat; a fresh-eyes judge on screenshots after.
- Easing: two spans and no other, `ease::Span { Fast, Slow }`, 20 ms
  for what the pointer causes and 100 ms for the camera and the
  screens (DISPLAY The wheel); no other easing constant is added.
- The engine moves under the owner's parallel work; a game unit
  expects its signatures to change mid-unit and reports each break as
  engine friction rather than working around it.

## Engine queue, approved 2026-09-07

Engine units the owner approved, dispatched from this session under
the engine's own CLAUDE.md, one Opus agent at a time, in this order,
after the belt lands (owner: the engine is a path dependency, so no
engine edit while a game unit builds). No request file in the engine
repo; the design lives here and in the briefs; each unit writes its
ARCHITECTURE.md section. The owner commits in the engine.

The engine is at `../../mirage-engine`, which is
`/home/matthewg/Documents/Projects/mirage-engine`. Two read-only Opus
proposals (2026-09-07) designed every item below against the engine's
accepted patterns; the owner ruled on their doubts. Units in this
order, the first five additive, the last red-state:

0. Gate words, one reviewed change before any doc is written: the doc
   vocabulary gains font, glyph, arrow, grab, crosshair, punctuation,
   numpad, zoom, motion, sky, skies, equirectangular; the unit gate
   gains point, points.
1. Renames: `TickCtx::dt` to `tick_interval` (owner: matches
   `set_tick_interval` letter for letter), `FrameCtx::dt` to
   `since_last_frame`, and the crate-private fields with them.
2. `keys!` gains Minus, Equal, BracketLeft, BracketRight, Semicolon,
   Quote, Backquote, Backslash, Comma, Period, Slash, Home, End, PageUp,
   PageDown, Insert, Delete, CapsLock, Numpad0..9, NumpadAdd, Subtract,
   Multiply, Divide, Decimal, Enter, NumLock, with display names; new
   rows at the end so capture precedence holds.
3. Camera, pure geometry beside `pixel_of`:
   `shifted_so(point, lands_at, size) -> Option<Camera>`, translated
   never turned, `None` by `pixel_of`'s own contract; and
   `zoomed_about(point: Vec3, factor, size) -> Option<Camera>`, the
   world point the caller names keeping its pixel while the eye moves
   toward it, the field of view untouched (owner: the API assumes no
   plane; the game names the ground point its own ray test found).
   The game's depth-scaled drag pan and its zoom guess are deleted.
4. `FrameCtx::text_layout(text, Font) -> TextLayout` under the `ui`
   feature, in logical points: `Font::proportional(size)`,
   `Font::monospace(size)`, `Family { Proportional, Monospace }`
   public with `Font::family()`; `TextLayout` holds egui's galley and a
   context clone privately so `size`, `width`, `height` now and glyph
   boxes, `wrapped_at(width)` and a baseline later live on one value.
5. `FrameCtx::set_cursor(Cursor)` per frame, reset to Arrow each frame;
   `Cursor` closed as listed; resolved once into egui's platform output
   so one writer reaches winit, egui's own icon winning where the UI
   holds the pointer; winit writes the browser canvas style itself;
   headless `Session::cursor()` reads the resolved one.
6. Sky and ambient: `#[derive(Skies)]` in mirage-engine-derive (no
   macro), `BuildSky: Catalog` with `build(&self, &Assets) -> SkyData`,
   `NoSkies` uninhabited, `type Skies` on `Game` written by every
   implementor (neumannarch's `NoSkies` lands in the same change);
   `SkyData` a closed sum, `Equirect` now (width twice the height,
   else boot-fatal naming the sky) and parametric kinds such as a
   gradient later as variants; `Assets::sky(name)` reads an equirect
   `.png`; built skies live in a renderer table erased of the
   vocabulary; `ctx.set_sky(G::Skies)` per frame, last write kept,
   drawn as one full-screen triangle inside the forward pass after the
   opaque and cutout batches at far depth with an equal test, direction
   from the inverse projection with translation dropped, repeat across
   and clamp down, in HDR under bloom and exposure; no sky draws the
   documented background; an orthographic lens shows one direction's
   texels, a listed invariant. `ctx.set_ambient(Color)` per frame,
   channels held at zero and above, today's constant the default; the
   background constant moves to the sky module. Example `sky-turn`.
   About 900 code, 450 tests, 130 docs.
7. One headless input stream, red-state (delete `offer_ui`,
   `Overlay::offer(egui::Event)` and their two tests first): a
   crate-private event enum (Switched, Pointed, Moved(Motion, f32),
   Typed) fed by `press`, `release`, `set_pointer`, `motion(Motion,
   f32)` and `type_text(&str)` through one private `Session::feed` that
   reaches the action tables and egui both, modifiers read off the live
   devices; the windowed path keeps egui-winit's translation, a named
   two-producer seam carried by tests. `Session::new(..)?
   .with_frame_interval(Duration)` and `set_frame_interval` for the dt
   each `step` reports, zero by default so every existing session reads
   as today. Unblocks plan item 6. Rewrites the headless paragraph and
   `docs/verifying.md`.

## Engine gaps, verified open 2026-09-06

Each answered by a numbered item of the queue above; the game never
works around a gap.

- No text measure without a painter: cell widths are guessed at 0.6 em
  per digit; a measure on `FrameCtx` would delete the guess.
- The offscreen `Session` cannot drive the scroll wheel: `Motion::Wheel`
  is filled only by a winit event, so wheel zoom and the send drag's
  wheel count are verified through a key binding instead.
- The offscreen `Session::step` reports a frame `dt` of zero, so easing
  by frame delta freezes headlessly; the game eases by elapsed game
  time. A headless frame carrying a duration would let the drive test
  time-based display behaviour.
- `FrameCtx` exposes no cursor icon.
- `Key` has no punctuation keys, so zoom is bound to Q and E.
- `FrameCtx::dt` on the tick and the frame contexts share a name for a
  fixed and a variable step.
- egui strokes have no round caps without a second overlapping shape,
  which forces a blend-toward-backdrop fade instead of alpha.
- The overlay's MSAA sample count and egui's tessellation options are
  not exposed; whether the headless target resolves MSAA is
  undocumented.
- Offscreen input never reaches the UI layer: `press` and `set_pointer`
  are read by the game and `offer_ui` by egui, never both, so a screen
  built from egui widgets cannot be verified headlessly; plan item 6
  depends on this.
- A drag pan under perspective was inexact; the game now pans by two
  pixels' rays meeting the focus's plane through `Camera::ray_through`
  and `Ray::hit_plane`, exact at every zoom. Queue item 3's
  `shifted_so` would still delete the game's plane arithmetic.
- Found 2026-09-08 by the belt unit: `Sphere::catalog` is documented
  as `0..=3` while `Mesh` says every value is built once and kept, so
  a game cannot tell whether an uncatalogued value is a first-frame
  hitch or an error; `Light::point` documents no falloff, so a range
  is found only by rendering; `ray::Plane` is not re-exported from
  the crate root while `Ray` is, and `mesh::Plane` holds the root's
  name, and `Ray` exposes neither its origin nor its direction;
  `Camera` cannot say what distance frames a sphere or a box and
  never learns the drawing area, so the game derives the widest zoom
  by hand from the tilt and the field of view.

## Settled, do not re-raise

- No command budget as a rule of the game; caps against hostile input
  are an engineering matter inside the relay and `apply`.
- Ships are never drawn in a layout the sim does not have.
- No per-ship health bars.
- No points, no anchors, no objectives, no intrinsic compositions.
- Fixed-point numerics; per-row speed limits.
- A commander or any vital row; fixed seat quadrants; a staging place
  per seat; asteroid gravity or patched conics.
- Attachment or reference frames of any kind: every body moves under
  one law; an anchor is an orbit, not a frame.
- Slots or any fixed formation lattice: formation is emergent from the
  holding rule, which is one replaceable module.
- Client-side prediction of the sim's response to a want change: the
  preview is the sim's answer or nothing.
- No line or ribbon primitive in the engine: the HUD is painted in
  screen space through egui's painter over `Camera::pixel_of`.
- Hand-rolled dimensional newtypes (owner: madness).
- Nothing hidden by zoom means the world is always drawn: bodies shrink
  to a pixel floor and never vanish; a resting HUD mark fades as a pure
  function of zoom and never pops (owner, 2026-09-08).
- No small wheel: a wheel exists only at the hovered and the selected
  asteroid (owner, 2026-09-08).
- Structures have no body of their own in the sim and the display
  rings them about the asteroid; no orbit, offset or frame for a
  structure, ever (owner, 2026-09-08).
- A draft pick places the reserve structure at once (owner, 2026-09-08).
