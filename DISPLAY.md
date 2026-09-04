# Neumannarch — display language

What the player sees, as the target. The only input is the sim's
view. In the belt and on the HUD nothing is a numeral, a label or a
panel except the wheels' counts and the stockpile's bars; every other
fact is a shape, a position, a colour or a line. Screens are panels:
every screen outside a match, and only pause and results inside one
(Screens, below). Every control shows, while
hovered, the change to a want it will make; what the sim does about that change is drawn in the next tick
by the same rules as everything else, never predicted by the client.

## Ships are the truth

Every ship and structure is drawn where the sim has it, always. A held
force is readable because the sim holds it inside its rock's zone
(DESIGN.md, Movement and combat); the display never rearranges anything,
and no glyph stands for a ship anywhere but at the ship.

## The two layers

The belt is drawn in 3D, through the engine: rocks as meshes at their
bodies, and ships as billboarded textured quads showing the row's glyph,
one draw per ship. It changes only as the sim moves a body, every tick.

The HUD is painted in screen space over the belt camera's projection of
world points: everything the sections below describe, except rocks and
ships. Nothing on it is a widget or a panel; the painter is only a way to
put pixels on screen. It draws after the belt and is never covered by it,
and it changes with the sim and with the player's pointer, every frame.

## The wheel

Every rock that holds a composition, and the selected rock whether or
not it holds one, carries one wheel: the whole HUD of a rock. Ships and
structures are drawn at their world positions and never on it.

A wheel is a column of sections standing to the right of the rock,
centred on the rock's height, their inner edges on an arc of a circle
whose centre lies far to the rock's left, so the column bows toward the
rock's right and the rock's left side stays clear for what a later
section of this document puts there. A section is one upright strip for
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
  points away from the rock for leaving and toward it for arriving.
- **Wanted:** a hollow mark and the count still to come: the want not
  covered by present or arriving, the frame building counted among
  them, so the numeral holds while a frame opens and fills. Where a
  frame builds, the mark is a box filling from the bottom to the frame's
  progress, and a frame that spent nothing this second for want of a
  material carries a belt along the box's base in that material's hue:
  metals, volatiles or energy, three fixed hues named in the game crate.
  A dashed mark when no builder is at the rock.

Sections stand in a fixed order down the column, structures first and
then units, each by cost. A section is drawn only where the row can be
edited or stands here: the selected rock's own sector shows every row,
a row with nothing here as its hollow glyph alone, dimmed, and every
other sector shows only rows with a line to draw. When more than one
seat holds a composition at a rock, the column stacks one sector per
seat, seats in seat order from the top, each sector as tall as its own
sections with a gap between sectors. A sector taller than a stated number of strips wraps into a
second column beside the first, and a third past that, so a crowded
rock stays within the screen's height; a column is as wide as its
strips can grow, a digit and a signed step more than they show, so a
count rising or a band's step appearing never runs under the next
column. A sector carries a spine: a thin
line in the seat's colour along its inner edge, on the arc.

A wheel has two states and no third: full where the pointer or the
selection rests, small elsewhere. Hover and selection are one state
drawn one way, with the same scale, the same detail, the same bands and
the same alpha; selection differs from hover only in that it outlasts
the pointer and holds the camera's focus. While the pointer rests on another
rock's wheel, the selected wheel keeps its full size and its bands and
is drawn at the faint alpha of a small wheel, and is whole again when
the pointer leaves; the hovered wheel is full and whole. Two wheels
may be full at once, the hovered one whole and the selected one faint. A small wheel is drawn at a
smaller scale and shows present and in transit, never wanted, and is
drawn faint; a full wheel is drawn whole, and a faint wheel is painted
before a whole one. Neither size follows the crowd, and a wheel covered
by another is no fainter and does not move. Every change between the
two states, by hover or by selection alike, eases over one short
span, a constant of the display, never a jump. A faded glyph is blended toward the
backdrop, never drawn translucent, so its strokes do not double where
they cross. Which wheel is hovered is decided
against the wheels as they stood before any grew, and a hovered wheel
stays hovered until the pointer leaves its full extent by a margin of
a band's width or more, so growing under the pointer never changes
which wheel is hovered, an overshoot past an edge closes nothing, and
nothing jitters. A bare rock under the pointer carries the seat's own wheel as
a selected one does, every row hollow, so what a rock could hold shows
before it is clicked.

The wheels' numerals and the stockpile's are the only numerals on the
HUD. Ownership is colour, and colour is the team's; a team's seats share
it. Hovering any line shows one short phrase beside it naming the row
and saying what the line is and why: "Frigate here", "Frigate
building", "Frigate short of metals", "No builder for Frigate",
"Frigate wanted", "Frigate arriving from Rock 3", "Frigate leaving for
Rock 5". Hovering a glyph shows the row's name alone.

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
  - Extract: a chevron at (30,32) with a half-width of 15, pointing down.
  - Capacity: a ring at the centre, radius 13; radius 15 beside a plus.
  - Damage at short range: a dot at (30,34) of radius 8; at (30,29) of
    radius 7 when a belt sits below it.
  - Plating above zero: a belt, a line at y 45 from x 17 to 43.
  - Damage at long range: a bar at x 30 from y 14 to 48. The range
    threshold is a constant in the game crate.
- **Size,** three steps by cost class: a row's cost against two
  thresholds, constants in the game crate.

Fill is the owner's colour; the outline is white.

## The stockpile

The match's one numeral exception on the HUD, shaped as Beyond All
Reason shapes its resource bar. Across the top centre, one cell per
material, metals then volatiles then energy. A cell is: the material's
icon in its hue at the left; a bar filled to the stock over the capacity;
two numerals stacked at the bar's right, the stock above the capacity;
and after them the income and the spend per second as two signed
numerals, "+3" and "-5", income above spend. A bar at capacity shows the
loss as a faint overflow running off its right end. A wheel slot whose
row the stock cannot fund one of dims, and its hover names the short
material.

## Fights

While shots are exchanged at a rock, each engaged seat's spine lights
as a bar: a thick segment of the seat's colour from the top of its
sector, on the same arc, of one fixed length whoever the seat is, so
two seats' bars compare at a glance. The bar is full at the fight's
start and drains downward as that player's total HP at the rock falls.
Damage from the last second and a half trails the drain as a white
segment that catches up; white, since a red trail vanishes on a red
seat. A sector with nothing standing stays one bar tall while its bar
shows. Bars disappear ten seconds after the last shot. Ships carry no
health bars: damage is on the hull, and the combatant the player reads
is the force.

## Flights

A ship between rocks carries its glyph as a billboard, with a line ahead
to its destination rock. The line is faint at the ship and full at the
destination, and its dashes roll toward the destination, so its direction
reads from a still frame and from motion alike. Arrival moves the glyph
from the ship into the wheel's present count.

## Editing: the wheel's bands

Build is flow, so there is no queue; the player edits wants. A full
wheel, hovered or selected, looks one way and carries its
interactions; hovering shows them and clicking the rock locks them in
place when the pointer leaves. Each of its own sections gains
a plus band and a minus band between its glyph and its counts, plus
above minus, each an action button in the style of every other, an
outlined box bearing its sign in ink always, not on hover; the bands
stand at one place in every strip, so a count appearing never moves the
band under the pointer; while Shift is held the bands bear
"+5" and "-5" and add or remove five. A hovered band brightens and
fills. A band that would change nothing, plus at the cap or minus at
zero, is dimmed and bears no phrase. A click adds or removes that many
wants and selects the rock. Holding repeats after a third of a second
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

Sending: drag from one rock's wheel to another's. While the drag is
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
the belt's centre until the player's first placement, then at that rock;
clicking a rock's wheel makes it the focus. A pan, a zoom and the jump
to a new focus each ease over the same span as a wheel's growth, never
a cut.

## Ranges

Every rock's zone is drawn as one faint circle at the zone's radius in
the belt, always, in the rock's own tint from its caps. Every armed ship at a rock carries one faint circle at its longest
weapon range in its owner's colour; a ship in flight carries none, since
it is not a shooter. Both are painted on the HUD over the belt camera's
projection, thin, at low alpha, and are never brighter than a wheel.
Nothing else on the HUD states a distance.

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
  zoom whose zone circles stand apart, each tinted by its rock's
  caps; it redraws the instant the seed changes, and it pans and zooms
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
- **Results.** At the clock: the final belt, held still, under a panel
  titled Results: one row per team in the match's colours, its rocks held
  as a count of rock glyphs and its army value, the winning row marked;
  then Rematch, which returns to the lobby with its shape kept, and Leave
  to the title. The word standings appears nowhere on screen.

## Judging

Legibility is judged on screenshots rendered through the engine's offscreen
Session over fixed scenes: a region with several rocks, mixed forces, a
fight, and a flight; a fight at one rock; the whole belt. A judge that has
not seen the code answers, from the image alone: who holds each rock, which
side is winning, what is in flight and where to. Every misread is a defect.

## Later layers

Economy, orbit ellipses, time scrubbing, cost shown by appearance, alerts,
presets. Each is added over this base only after the base reads correctly.
