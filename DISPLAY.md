# Neumannarch — display language

What the player sees, as the target. The only input is the sim's view.
In the belt and on the HUD nothing is a numeral, a label or a panel
except the wheels' counts, the stockpile's stock and net and the clock;
every other fact is a shape, a position, a colour or a line. Screens are
panels: every screen outside a match, and only pause and results inside
one (Screens, below). Every control shows, while hovered, the change to
a want it will make; what the sim does about that change is drawn in the
next tick by the same rules as everything else, never predicted by the
client.

## Ships are the truth

Every ship and structure is drawn where the sim has it, always. A held
force is readable because the sim holds it inside its asteroid's zone
(DESIGN.md, Movement and combat); the display never rearranges anything,
and no glyph stands for a ship anywhere but at the ship.

## The two layers

The belt is drawn in 3D, through the engine: asteroids as meshes at their
bodies, and ships as billboarded textured quads showing the row's glyph,
one draw per ship. It changes only as the sim moves a body, every tick.

The HUD is painted in screen space over the belt camera's projection of
world points: everything the sections below describe, except asteroids and
ships. Nothing on it is a widget or a panel; the painter is only a way to
put pixels on screen. It draws after the belt and is never covered by it,
and it changes with the sim and with the player's pointer, every frame.

## The wheel

Every asteroid that holds a composition, and the selected asteroid whether or
not it holds one, carries one wheel: the whole HUD of an asteroid. Ships and
structures are drawn at their world positions and never on it.

A wheel is a column of sections standing to the right of the asteroid,
centred on the asteroid's height, their inner edges on an arc of a circle
whose centre lies far to the asteroid's left, so the column bows toward the
asteroid's right and the asteroid's left side carries the asteroid's resources
(Resources, below). A section is one upright strip for
one row, of the glyph's height, never a box: the row's glyph at its
left, then its counted lines side by side along the strip, each a small
mark and a numeral in the seat's colour, packed left to right so the
strip holds no blank. A wide count widens its cell and the strip with
it; nothing is cut. A line with a count of zero is not drawn. The
lines, left to right:

- **Present:** a dot mark and the count of the row's units here.
- **Surplus:** a hollow dot and the count of the row's units here or
  arriving above the want. Nothing complete is ever scrapped, so
  lowering a want moves units from the dot to the hollow dot and they
  stay until a shortfall elsewhere wants them.
- **In transit:** an arrow mark and one count of units leaving (in a
  send forming here) or arriving (in a send toward here); the arrow
  points away from the asteroid for leaving and toward it for arriving.
- **Wanted:** a hollow mark and the count still to come: the want not
  covered by present or arriving, the frame building counted among
  them, so the numeral holds while a frame opens and fills. Where a
  frame builds, the mark is a box filling from the bottom to the frame's
  progress, and a frame that spent nothing this second for want of a
  material carries a belt along the box's base in that material's hue:
  metals, volatiles or energy, three fixed hues named in the game crate.
  A dashed mark when no builder is at the asteroid.

Sections stand in a fixed order down the column, structures first and
then units, each by cost. A section is drawn only where the row can be
edited or stands here: the selected asteroid's own sector shows every row, a
row with nothing here as its hollow glyph alone, dimmed, and every other
sector shows only rows with a line to draw. When more than one seat
holds a composition at an asteroid, the column stacks one sector per seat,
seats in seat order from the top, each sector as tall as its own
sections with a gap between sectors. A sector taller than a stated
number of strips wraps into a second column beside the first, and a
third past that, so a crowded asteroid stays within the screen's height; a
column is as wide as its strips can grow, a digit and a signed step more
than they show, so a count rising or a band's step appearing never runs
under the next column. A sector carries a spine: a thin line in the
seat's colour along its inner edge, on the arc.

A wheel has two states and no third: full where the pointer or the
selection rests, small elsewhere. Hover and selection are one state
drawn one way, with the same scale, the same detail, the same bands and
the same alpha; selection differs from hover only in that it outlasts
the pointer and holds the camera's focus. While the pointer rests on
another asteroid's wheel, the selected wheel keeps its full size and its
bands and is drawn at the faint alpha of a small wheel, and is whole
again when the pointer leaves; the hovered wheel is full and whole. Two
wheels may be full at once, the hovered one whole and the selected one
faint. A small wheel is drawn at a smaller scale and shows present and
in transit, never wanted, and is drawn faint; a full wheel is drawn
whole, and a faint wheel is painted before a whole one. Neither size
follows the crowd, and a wheel covered by another is no fainter and does
not move. Every change between the two states, by hover or by selection
alike, eases over the fast span, never a jump. Every eased change on
screen uses one of two spans and no other, fast and slow, named
constants of the game crate: fast for what the pointer causes, slow for
what the camera and the screens do. A faded glyph is blended toward the
backdrop, never drawn translucent, so its strokes do not double where
they cross. Which wheel is hovered is decided against the wheels as they
stood before any grew, and a hovered wheel stays hovered until the
pointer leaves its full extent by a margin of a band's width or more, so
growing under the pointer never changes which wheel is hovered, an
overshoot past an edge closes nothing, and nothing jitters. A bare
asteroid under the pointer carries the seat's own wheel as a selected
one does, every row hollow, so what an asteroid could hold shows before
it is clicked.

The wheels' numerals, the stockpile's and the clock's are the only
numerals on the HUD. Ownership is colour, and colour is the team's; a
team's seats share it. Hovering any line shows one short phrase beside
it naming the row and saying what the line is and why: "Frigate here",
"Frigate building", "Frigate short of metals", "No builder for Frigate",
"Frigate wanted", "Frigate arriving from Asteroid 3", "Frigate leaving for
Asteroid 5". Hovering a glyph shows the row's name alone.

## The glyph

Three rules produce every glyph from its row; no glyph is drawn by hand.

- **Shape:** a triangle for a unit, a square for a structure.
- **Marks,** placed on the frame by what earns them, in a sixty-unit
  cell whose shape is the triangle (30,6) (56,52) (4,52) or the square
  from 8 to 52. The mark's outline is white; a hollow mark is stroked, a
  dot is filled. These are the owner's drawings and the game reproduces
  them exactly, scaled to the glyph's size:
  - Build: a plus at (30,38) with arms of 8.5 on a triangle; at the
    centre with arms of 6.5 inside the ring on a square that also stores.
  - Extract: a chevron centred at (30,44) with a half-width of 10 and a
    half-height of 4, pointing down, and above it the weapon's
    material's icon (Icons, below),
    centred at (30,26) and 20 units wide, both wholly inside the
    frame, so an extractor reads as one by its chevron and as which by
    its icon.
  - Capacity: a ring at the centre, radius 13; radius 15 beside a plus.
  - Damage at short range: a dot at (30,34) of radius 8; at (30,29) of
    radius 7 when a belt sits below it.
  - Plating above zero: a belt, a line at y 45 from x 17 to 43.
  - Damage at long range: a bar at x 30 from y 14 to 48. The range
    threshold is a constant in the game crate.
- **Size,** three steps by cost class: a row's cost against two
  thresholds, constants in the game crate.

Fill is the owner's colour; the outline is white.

## The stockpile and the clock

One strip across the top centre, of the wheel strip's height: one box in
the screens' style, a scrim with the screens' line around it, holding
four cells packed the wheel's cell gap apart and no more, three one per
material, metals then volatiles then energy, and a fourth for the clock.
A material's cell, left to right: the material's icon (Icons, below) at
the glyph's height; the stock as a numeral in ink; a bar of one fixed
length, the same for every cell, filled from its left end to the stock
over the capacity in the material's hue, so an empty bar is an empty bar
and no numeral stands on it; after the bar's right end the net as a
signed numeral in ink, "+3" or "-5", income less spend per second over
the last second. Income is what the seat's extractors pulled, before the
capacity clamp; spend is what its frames drained; a cancelled frame's
refund is neither, and the stock's jump on a cancel is its own reading.
Past the fill's tip a fainter segment of the hue extends by ten seconds
of income, and inside the tip a darker segment of the hue marks ten
seconds of spend, so which of the two is longer says which way the stock
moves and how fast. At capacity the income segment runs past the bar's
right end, and that overrun is the loss, at the fill's own alpha, never
fainter; the net numeral sits after the overrun. An empty bar with a
negative net is a stall and draws nothing more, since the frame's belt
at the asteroid names the material. Hovering a cell shows one short
phrase beneath it, the capacity, "of 300"; the two segments already
carry the in and the out. While a wheel's plus band is hovered, each
material's bar marks the row's cost as the darker segment inside the
fill's tip, in place of the spend projection, and a minus band marks the
refund as the fainter segment past the tip, so what a want costs is read
where it is paid and never as a numeral.

The clock's cell is a bar of the same length in ink, full at the start,
its fill shrinking from the right as the match runs, so the filled part
is the time left; inside it at the left the elapsed minutes and seconds
as a numeral. It has no icon, no hue and no hover.

## Icons

The three material icons are one SVG sheet in the game crate,
`game/icons/materials.svg`, three groups with the ids metals,
volatiles and energy, each one drawing in its own sixty-unit cell laid
side by side, so the owner edits the three as a whole: metals a hex
nut, a hexagon with a small hole; volatiles a drop; energy a bolt.
Each group is one closed path, no open stroke, no gradient and no
text, read by its id when the
game starts into the same primitives the marks are made of, so it is
stroked hollow or filled solid and tinted by whatever draws it, and it
reads at the smallest glyph size, where thirty units are six pixels.
An icon is drawn wherever a material is named: the Extract mark on an
extractor's glyph, the stockpile's cells and the asteroid bars. They are
the owner's drawings and the game reproduces them exactly, as it does
the marks.

## Resources

Every asteroid carries three bars at its left, the mirror of the wheel at
its right: one per material in the fixed order, stacked, each of one
strip's height, their right ends on the same arc the wheel's sections
stand on, mirrored, its centre far to the asteroid's right, and along that
arc a spine in grey ink, the wheel's spine in no seat's colour. The
icon stands at the bar's right end, nearest the asteroid, and the bar
grows leftward from it. A bar's cap is a band of the material's hue
dimmed toward the backdrop, its length the asteroid's cap for the material
against the largest cap of any material on the belt, so a rich asteroid has
long bands and a poor one short and the cap reads on any display; the
pull over the last second by every extractor there of any seat is the
same hue at full strength laid over the band from its right end, so
fill against band is pull against cap. No outline carries the cap. A
cap of zero draws no bar. An extractor the seat wants at the asteroid
and has not yet standing shows on that material's bar past the pull:
a frame that is building as a fainter segment of the hue, as long as
the extractor's yield would be against the cap, filling with the
frame's progress; a want with no frame yet as a hollow outline of the
same segment; so a player schedules construction against what the
asteroid will give. The bars are drawn at every asteroid always and
take the wheel's two states with it: full where the wheel is full,
small elsewhere, easing between them as the wheel does, drawn at the
wheel's pixel scale at every zoom, so a crowded belt overlaps them.
The small state is the bars alone, no icons, standing tight against
the asteroid as the small wheel stands tight on its other side; the full
state adds the icons and the room they need. Hovering a bar shows "Metals 12
of 20".

## Fights

While shots are exchanged at an asteroid, each engaged seat's spine lights
as a bar: a thick segment of the seat's colour from the top of its
sector, on the same arc, of one fixed length whoever the seat is, so
two seats' bars compare at a glance. The bar is full at the fight's
start and drains downward as that player's total HP at the asteroid falls.
Damage from the last second and a half trails the drain as a white
segment that catches up; white, since a red trail vanishes on a red
seat. A sector with nothing standing stays one bar tall while its bar
shows. Bars disappear ten seconds after the last shot. Ships carry no
health bars: damage is on the hull, and the combatant the player reads
is the force.

## Flights

A ship between asteroids carries its glyph as a billboard, with a line
ahead along the path its schedule will fly, the sim's own prediction
integrated from the ship's body to its arrival, never a straight
line. The line is faint at the ship and full at the destination, and
its dashes roll toward the destination, so its direction reads from a
still frame and from motion alike. Arrival moves the glyph
from the ship into the wheel's present count.

## Editing: the wheel's bands

Build is flow, so there is no queue; the player edits wants. A full
wheel, hovered or selected, looks one way and carries its
interactions; hovering shows them and clicking the asteroid locks them in
place when the pointer leaves. Each of its own sections gains
a plus band and a minus band between its glyph and its counts, plus
above minus, each an action button in the style of every other, an
outlined box bearing its sign in ink always, not on hover; the bands
stand at one place in every strip, so a count appearing never moves the
band under the pointer; while Shift is held the bands bear
"+5" and "-5" and add or remove five. A hovered band brightens and
fills. A band that would change nothing, plus at the cap or minus at
zero, is dimmed and bears no phrase. A click adds or removes that many
wants and selects the asteroid. Holding repeats after a third of a second
and every tenth of a second after that. A full wheel takes input
through its bands and through the drag under Sending, and nothing else;
a small wheel takes none and shows no bands.

Hovering a live band shows its change as a signed step beside the
strip, "+1" or "-5", and changes no count. The click lands the change,
and the next tick draws the sim's response by the rules above: a count
rising, a frame filling, a dashed mark, a ship lifting out toward its
new home. The client predicts nothing.

Gamepad: the stick points at a section, one face button is plus and
another is minus, a shoulder button is Shift, holding a button shows its
preview and releasing commits, and a held button repeats as with the
mouse.

A wheel shows at most twelve sections. A larger roster collapses to two,
a square and a triangle, and choosing one opens that category's own
wheel in its place, which does not collapse again.

Sending: drag from one asteroid's wheel to another's. While the drag is
held, the source wheel dims the lines that would go and the destination
wheel shows them as an arriving line at half alpha, a hover preview like
the plus band's; the mouse wheel adjusts how many; release issues the
two count edits, and the flight is the sim's response.

## Camera

A sixty-degree tilt from above, pan and zoom, no rotation. Every move
of the camera eases over a short span, pan, zoom and the jump to a new
focus alike, never a cut. The camera is
attached to a focus point that moves at the local orbital velocity, so the
player's region stays on screen while the belt turns. The focus starts at
the belt's centre until the player's first placement, then at that asteroid;
clicking an asteroid's wheel makes it the focus. A pan, a zoom and the jump
to a new focus each ease over the slow span, never a cut.

## Ranges

Every asteroid's zone is drawn as one faint circle at the zone's radius in
the belt, always, in one ink. Every armed ship at an asteroid carries one
faint circle at its longest weapon range in its owner's colour; a ship
in flight carries none, since it is not a shooter. Both are painted on
the HUD over the belt camera's projection, thin, at low alpha, and are
never brighter than a wheel. Nothing else on the HUD states a distance.

## Words on screen

Every string the player reads is a short phrase, never a sentence: no
full stop, no semicolon, no dash. A comma in a string is a sign the
screen says too much and is brought to the owner before it is drawn.
The screen never tells the player what to do: no hint, no prompt, no
tutorial phrase, ever. A string that opens with an imperative verb is
a defect.
The screen never tells the player what to do: no hint, no prompt, no
tutorial phrase, ever. A control shows what it is; the belt shows what
is; nothing invites.

## Controls

Every control on every screen, the wheel included, is one of three kinds,
and each kind looks and behaves one way everywhere.

- **An action** is a button that does one thing on click: Start, Ready,
  Leave, Kick, Rematch, Random Seed, Quit, a wheel band.
- **A choice** is a dropdown showing its current value; a click opens the
  list and a click picks. A team, a seat's holder, the clock. Nothing
  cycles on click.
- **A value** is a field showing its current value, typed into. The seed,
  the join address.

A control and its label are one thing: the value is inside the control,
never beside it, and an action that belongs to a value sits inside that
value's field at its right edge, as Random Seed does inside the seed.

A screen's main action, Start or Ready on the lobby, Join on the
title, shows its reason beside it always when disabled, not only on
hover, so the way forward is never hidden. Every row of a settings
column carries its label at the left and its control at the right.
A guest's control follows the same rule as the host's. If the host's
edit lands first, the screen shows the room's result and nothing else:
there is no message for an action that did not take, since the state on
screen is the truth.

A control whose feature does not exist in this version is not drawn.
Disabled with a reason is only for a control that exists and cannot act
now.

Enabled means it will work. A control is enabled only when the rule
behind it, evaluated on this machine against the state shown, says the
action succeeds; otherwise it is disabled, and hovering it shows the
reason as a short phrase beside it: "Waiting for Team 2", "Host only",
"Cannot host here", "Cannot quit here". The rules are the same
values the sim and the lobby apply, so a control is never enabled and
then refused.

The join field shows its own outcome while staying editable:
"Connecting", "No room", "Room full", "Version differs". It also shows
why a return to the title happened, keeping the address: "Room closed",
"Host left", "Removed". No other screen has an error, a notice or a log.

## Screens

Outside a match the game is a sequence of full-window screens, drawn as
panels in one style: the HUD's palette, thin lines, no window chrome, the
roster's glyphs from the same rasteriser as the belt's ships. No screen
floats over another; each replaces the last. Every control on them
follows Controls, above.

- **Title.** Skirmish, Host, Join, Quit, in a column, each an action;
  Join is a value field for the address with Join as the action inside
  it. Host is enabled only while the title holds a listener it bound on
  opening, so a port in use disables it. Quit closes the game on the
  desktop and is not drawn in the browser. Settings is drawn once a
  settings screen exists.
- **Lobby.** One screen for skirmish and multiplayer. The belt the match
  will be played on fills the screen behind everything else, rendered
  from the seed by the same belt and HUD code as the match, at the widest
  zoom whose zone circles stand apart, each asteroid wearing its resource
  bars; it redraws the instant the seed changes, and it pans and zooms
  under the same controls as the match. Over it, at the left, the seats
  as a table of four rows, one per seat the match can hold, under column
  labels Holder, Team, Ready. Holder is a choice: You, Open, Closed, each
  bot personality by name, or, for a seat a guest holds, that guest's
  name, not chosen but shown with Kick as an action beside it. Team is a
  choice of the four teams, each shown as a square of the team's colour
  beside its number, in the closed control and in the open list alike;
  colour is the team's everywhere the game draws ownership. Ready is a
  mark. A closed seat's row stays
  in the table with its holder reading Closed and its team and ready
  cells empty. Down the right, the match's settings as labelled rows:
  Seed, a value with Random Seed inside it; Clock, a choice of one, five,
  fifteen or thirty minutes. Across the bottom: Ready for a guest, Start
  for the host, and Leave, with a disabled Start's reason beside it. A
  guest sees the host's controls disabled with their reason, and the
  host's edits as they land. Kick returns the guest to the title. Every
  label is a word in title case, never an identifier.
- **Loading.** The belt from the lobby, still, until every machine has
  built the match and agreed the first hash.
- **Play.** The match, as every section above describes. Escape opens the
  pause screen over it: Resume and Leave, and Surrender once DESIGN.md
  has a rule for it. Play continues under it
  in multiplayer and stops under it in a skirmish. Play has two held
  states, both in multiplayer only. Waiting: when a peer has fallen
  behind by the stated span, the match holds, the HUD dims, and the
  waiting seats' colours are shown; it resumes by itself. Desynced: when
  the room reports a settled tick whose hashes differ, the match holds,
  the HUD dims, the word Desynced and the tick are shown, and Leave
  returns to the title; it never resumes.
- **The draft.** While the draft runs (DESIGN.md, Start), Play carries
  one panel, the third inside a match, titled Draft: the order, drawn
  as an initiative list at the screen's left edge over the belt, never
  over the strip, in the screens' style, packed as tight as its rows
  read. One row per stage in the order the stages run, the first round
  then the second: the glyph of the structure that stage places,
  filled in the seat's colour once placed and hollow in the seat's
  colour before, so the glyph carries the seat; the seat's name (You,
  the bot's name, a guest's name); and at the right a bar of one fixed
  length: full before the stage begins, draining over the stage's span
  while it runs, empty once it ran out unplaced and until its seat
  places, and replaced by the asteroid's name, "Asteroid 3", once placed. After
  the last stage ends, one last row, titled Clock, drains the grace.
  The running stage's row is whole and every other row is faint, as a
  small wheel is. The strip's clock cell reads 0:00 with its bar full.
  A bot's name is drawn from a short list the bot's personality owns,
  chosen by the seed and the seat, so a match's bots read as people
  and two bots of one personality read apart; the lobby's Holder
  choice still names the personality. An asteroid a draft placement stands
  on shows the placed row on its small wheel as a wanted line in the
  seat's colour, the one wanted line a small wheel ever shows, so a
  taken asteroid reads as taken from the belt. Placing goes through the
  wheel: a bare asteroid under the pointer shows the seat's own hollow
  wheel. Every band is live during the draft, since a want accepted
  then stands until the clock runs, except a reserve band before the
  seat's first stage, disabled with "Not yet", and a reserve band at a
  asteroid the draft has taken, disabled with "Asteroid taken", which are the
  sim's own refusals and nothing more. Nothing is refused silently.
  When the last placement lands or the grace runs out, the panel goes
  over the slow span, and the clock's numeral starts.
- **Results.** At the clock: the final belt, held still, under a panel
  titled Results: one row per team in the match's colours, its asteroids held
  as a count of asteroid glyphs and its army value, the winning row marked;
  then Rematch, which returns to the lobby with its shape kept, and Leave
  to the title. The word standings appears nowhere on screen.

## Judging

Legibility is judged on screenshots rendered through the engine's offscreen
Session over fixed scenes: a region with several asteroids, mixed forces, a
fight, and a flight; a fight at one asteroid; the whole belt. A judge that has
not seen the code answers, from the image alone: who holds each asteroid, which
side is winning, what is in flight and where to. Every misread is a defect.

## Later layers

Economy, orbit ellipses, time scrubbing, cost shown by appearance, alerts,
presets. Each is added over this base only after the base reads correctly.
