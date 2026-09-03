# Probe Game — display language

What the player sees, as the target. The only input is the sim's fogged
view. Nothing is a numeral, a label, or a panel; every fact is a shape, a
position, a colour, or a line in the belt. Inside a match the pause and
results screens are the only panels; outside a match every screen is a
panel in the game's style (Screens, below). Every control shows, while hovered, the change to a want it
will make; what the sim does about that change is drawn in the next tick
by the same rules as everything else, never predicted by the client.

## Ships are the truth

Every ship is drawn where the sim has it, always. A held force is readable
because the sim settles it around its place's anchor (DESIGN.md, Movement
and combat); the display never rearranges anything.

## The two layers

The belt is drawn in 3D, through the engine: rocks as meshes at their
bodies, and ships as billboarded textured quads showing the row's glyph,
one draw per ship. It changes only as the sim moves a body, every tick.

The HUD is painted in screen space over the belt camera's projection of
world points: everything the sections below describe, except rocks and
ships. Nothing on it is a widget or a panel; the painter is only a way to
put pixels on screen. It draws after the belt and is never covered by it,
and it changes with the sim and with the player's pointer, every frame.

## The rings

Every rock carries, on the HUD, a billboarded inner ring at a fixed screen
radius, so it reads the same at every zoom, for its inner band. A second,
outer ring at a larger fixed radius stands for the outer band and is drawn
only when that band holds something or the rock is selected. On a ring,
runs start at twelve o'clock and are laid clockwise; when more than one
seat is present, in seat order. Ownership is colour; position tells seats
apart only where several share a ring, which is uncommon. An empty ring
draws nothing.

## Glyph runs

One glyph per unit present, on the HUD, grouped by row, rows ordered by
cost descending from the run's start. Glyphs of a row are laid consecutively
along the ring.
When a run would exceed its share of the ring, its glyphs overlap and stack
like cards; magnitude stays arc length.

- A unit wanted but absent continues the run as a hollow glyph.
- A frame in progress is a hollow glyph filling from the bottom to its
  progress fraction.
- A shortfall with no builder in range shows a dashed hollow glyph.

## The glyph

Three rules produce every glyph from its row; no glyph is drawn by hand.

- **Frame:** a triangle for a unit, a square for a structure.
- **Marks,** one per weapon, inside the frame: a dot for damage with range
  within sight, a bar for damage with range beyond sight, a plus for build,
  a chevron for extract.
- **Size,** three steps by cost class; the thresholds are constants in the
  game crate.

Fill is the owner's colour; the outline is white.

## Fights

While shots are exchanged in a band, each engaged seat's run on that ring
gains an arc just inside it, on the HUD. The arc is full at the fight's start and
drains clockwise as that player's total HP at the rock falls. Damage from
the last second and a half trails the drain as a red segment that catches
up. Arcs disappear ten seconds after the last shot. Ships carry no health
bars: damage is on the hull, and the combatant the player reads is the
force.

## Flights

A ship between rocks carries its glyph as a billboard, on the belt, with a
line ahead, on the HUD, to its destination rock. Arrival moves the glyph
from the ship onto the ring.

## Editing: the roster wheel

Build is flow, so there is no queue; the player edits wants. Selecting a
ring, inner or outer, opens a wheel, on the HUD, outside both rings at a
fixed screen radius, editing that band: one slot per roster row, drawn as
the row's glyph by the same three rules. Structures fill the left half and units the
right, each half ordered by cost from the top down; the outer band's wheel
has no structures. The selected ring brightens.

A slot is a sector of the wheel's annulus with the glyph at mid radius. Its
outer band is plus and its inner band is minus; neither carries a symbol,
since a plus mark inside a glyph already means a build weapon. A hovered
band brightens. A click adds or removes one want. Holding repeats after a
third of a second. There is no other input on the wheel, and the inner run
is display only.

Hovering plus shows one hollow glyph at half alpha at the end of that row's
run. Hovering minus dims the last glyph of that row: a hollow one means the
click cancels a frame, a solid one means it sends a ship away. The click
lands the change, and the next tick draws the sim's response by the rules
above: a line from the rock a surplus ship is coming from, a glyph filling,
a dashed glyph, a ship lifting out toward its new home. The client predicts
nothing.

Gamepad: the stick points at a slot, one face button is plus and another is
minus, holding a button shows its preview and releasing commits, and a held
button repeats as with the mouse.

A wheel holds at most twelve slots. Past that, the first level is two
slots, a square and a triangle, and choosing one opens that category's
wheel in its place. There is no deeper level.

Sending: drag from a ring to another, on the same rock or another. The
source run dims the glyphs that would go and the destination run shows them
hollow; the mouse wheel adjusts how many; release issues the two count
edits, and the flight is the sim's response. Sending a staged force in is
the drag from a rock's outer ring to its inner ring.

## Camera

A sixty-degree tilt from above, pan and zoom, no rotation. The camera is
attached to a focus point that moves at the local orbital velocity, so the
player's region stays on screen while the belt turns. The focus starts at
the belt's centre until the player's first placement, then at that rock;
clicking a rock's ring makes it the focus.

## Fog

Enemy ships and their glyphs are drawn on the belt only inside sight. A
radar contact is a dot with a short velocity streak and no glyph, on the
HUD. Rocks are always drawn, on the belt.

## Screens

Outside a match the game is a sequence of full-window screens, drawn as
panels in one style: the HUD's palette, thin lines, no window chrome, the
roster's glyphs from the same rasteriser as the belt's ships. No screen
floats over another; each replaces the last.

- **Title.** Skirmish, Host, Join, Settings, Quit, in a column. Join
  takes an address.
- **Lobby.** One screen for skirmish and multiplayer. Its centre is the
  belt the match will be played on, rendered from the seed by the same
  belt and HUD code as the match, every rock's ring drawn, regions tinted
  by their caps; it redraws the instant the seed changes. Down the left,
  one row per seat: its colour, who holds it (the player's name, a bot's
  personality, open, or closed, as DESIGN.md's lobby rule names them),
  its team, and its readiness. A row the
  viewer owns is editable where DESIGN.md's lobby rule allows; the rest
  is not. Down the right, the host's shape: seed with a regenerate action,
  clock, team layout. Across the bottom: Ready for a guest, Start for the
  host, enabled by the same rule, and Leave. A guest sees the host's edits
  as they land.
- **Loading.** The belt from the lobby, still, until every machine has
  built the match and agreed the first hash.
- **Play.** The match, as every section above describes. Escape opens the
  pause screen over it: Resume, Surrender, Leave. Play continues under it
  in multiplayer and stops under it in a skirmish. Play has one held
  state: in multiplayer, when a peer has fallen behind by the stated
  span, the match holds, the HUD dims, and the waiting seats' colours are
  shown; it resumes by itself.
- **Results.** At the clock or elimination: the final belt under the
  standings, rocks held and army value per team in the match's own
  glyphs and colours with the winner named, and Rematch, which returns to
  the lobby with its shape kept, or Leave to the title.

## Judging

Legibility is judged on screenshots rendered through the engine's offscreen
Session over fixed scenes: a region with several rocks, mixed forces, a
fight, and a flight; a fight at one rock; the whole belt. A judge that has
not seen the code answers, from the image alone: who holds each rock, which
side is winning, what is in flight and where to. Every misread is a defect.

## Later layers

Economy, orbit ellipses, time scrubbing, cost shown by appearance, alerts,
presets. Each is added over this base only after the base reads correctly.
