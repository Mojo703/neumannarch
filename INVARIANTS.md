# Neumannarch — invariants and dependencies

What the code cannot say about itself: the holes it tolerates at runtime,
each with the shape change that would delete it, and why each dependency
was chosen over its alternatives. Everything else about how the code is
shaped is read from the code. A unit that introduces a tolerated failure
adds it here in the same change; a unit that deletes one removes it.

## Checked on every change

- `sim` builds for `wasm32-unknown-unknown` and its tree does not name
  the engine.
- No `std` transcendental, `HashMap`, `HashSet`, `Instant` or `SystemTime`
  in `sim` (`sim/clippy.toml`).
- `State: Hash + PartialEq`, derived, so a new field is hashed without a
  hand edit.
- Replay of a log reproduces the live hash.
- No phase type holds `&mut State`.
- No `unwrap` or `expect` on data that came from a command.

## Tolerated runtime failures

- `Entities::entity` panics on an id the store no longer holds. Every id the
  state keeps is reaped in the same `next` that removes the entity, and the
  effect values a step passes between its phases are read inside that step,
  so no live code can ask; an id that leaves the sim is only ever read back
  by the display, which never asks the store. A borrowed entity in place of
  an id everywhere would delete it, which the effect values cannot carry
  across the borrow of the tick-start snapshot.
- Two ships at exactly one point push each other nowhere, since a push has
  no direction there. Their drift differs by id, so they part within a tick.
  A spacing that could not be zero would delete it, which no placement rule
  can promise once ships are free to move.
- `orbit::universal` caps Newton's iteration at sixty steps; reaching the
  cap means the span was outside the contract. A `Span` type bounded by
  the body's period would delete the cap.
- Two asteroids' zones can pass within one zone's radius of each other
  over a match. The belt lays each asteroid off three smooth fields and a
  drawn point, with nothing holding a pair apart, and the zones are small
  against the belt's own spacing, so a meeting is rare and no rule reads
  it. Excluding a candidate whose orbit comes inside a stated distance of
  a standing one at any tick of the match would delete it, at the price of
  that test over every pair and every tick of generation.
- `BeltCamera::face_the_star` holds the yaw it had while the focus sits
  exactly on the star, where the angle about the star has no value. Only
  the whole-belt frame puts it there, and no pan can, since a pan's
  component toward the star reaches zero at a floor stated off the belt's
  inner edge. A focus that could not stand on the star would delete it, which
  would cost the whole-belt frame the star as its centre.
- The widest zoom frames the belt for a window at least as wide as it is
  tall; a window taller than it is wide clips the belt's sides. The
  camera is built before a frame and never learns the drawing area. A
  camera that took the window with every zoom would delete it.
- A pointer drag keeps the belt under the pointer only while the drag is
  small against the focus's own radius. The yaw is locked to the focus's
  angle about the star, so a pan that carries the focus a noticeable
  share of the way round also turns the view under the pointer. A yaw that did not
  follow the focus would delete it, and would cost the star its one
  screen direction.
- A frame carrying a machine's request arrives at a machine's own
  connection only from a room that has broken the protocol, and is dropped
  as bytes that are not a message are. A wire typed by direction would
  delete the arm; one tagged union cannot express it.
- A room drops a notice or a desync sent by a member, since only the room
  produces them. The same direction-typed wire would delete both arms.
- A room drops a command stamped past the tick its match ends by, and a
  hash reported there, without a word; the cap is what keeps its ledger of
  a match from growing on a tick no match will ever hold. No machine
  playing the match can name such a tick: a match ends at its clock counted
  from the draft's end, the draft ends by the tick `Setup::ends_by` reads
  off the seats and the spans a placement stage and the grace state, and a
  machine issues nothing once the standings say the clock has run. A ledger
  bounded by the last tick the machines themselves have settled, rather
  than by the setup, would delete it, at the price of taking the bound from
  the senders it is there to bound.
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
- `State::apply` reads a want of one as a pick only while the seat still
  holds that row in reserve, since a stage stays unplaced when the
  reserve is spent through an ordinary want and a seat must still be
  able to want one of that row where something stands. A pick command
  distinct from a want of one would delete the rule.
- `Play::previewed` takes a `Rejected` from `State::preview` as an empty
  preview. No hover can raise one: `Wheel::button_at` never answers with a
  button the sim refuses, so no refused want is ever previewed, and a
  drag's edits are clamped to `MAX_WANT` at an asteroid the seat stands
  units at. A `Preview` of a want and a refusal of it in one type, so the
  display could not hold the first without answering the second, would
  delete the arm.
- `FightStage::of` takes its lateral by normalising the asteroid's
  position and its axis by normalising what is left of the asteroid's
  velocity once that lateral is taken out of it, and answers the zero
  vector where either has no direction; `State::spawn_body` takes the
  radial the same way. An asteroid's orbit is a bounded ellipse about a
  central mass, so the position is never zero and the velocity never runs
  along it, and no fight can reach the fallback: it would collapse a
  stage onto the asteroid and leave its plane no normal to lift it off
  the body or to spread a line across. An orbit that answered directions
  rather than vectors, so a body
  carried a radial and a tangent that exist by construction, would delete
  both.
- `Asteroid::toward_shell` answers the zero vector for a point at exactly
  the asteroid's centre, where there is no direction out, so the return
  term only damps a ship standing there. The term pushes out from every
  other point inside the floor, a unit spawns at the floor or beyond it
  and the stage's own plane stands off the body, so nothing steers to
  that point. A push read from the asteroid's own radial, which every
  orbit has, rather than from the ship's offset, which a ship at the
  centre has not, would delete it, at the price of a push that leans one
  way at every other point inside the floor.
- Two units that do no damage can circle one plane from one starting
  phase. A circle's plane and phase are drawn from the unit's
  identifier through the digest, and nothing holds two draws apart, so a
  pair whose digests collide holds one station; separation parts them
  there as it parts any pair that meets. A plane taken from a fixed
  sequence per asteroid, the nth unit standing at the nth plane, would
  delete it, at the price of a unit that changed its circle whenever
  another arrived or died.
- `Threats::best` answers with no target where the shooter's team and
  plating name no ranking of the roll. A ranking is built for every row
  that does damage standing at the asteroid, and only a unit standing
  there that does damage ever asks, so the arm cannot be reached. A
  ranking read by the asking entity rather than by a pair of values
  would delete it, at one ranking per unit rather than one per pair.
- A bot's frame stands for one decision at an asteroid where no builder
  of its seat stands or arrives. The funding pass justifies a want once
  a decision, a second apart, while `Fulfilment` opens frames every tick
  off the tick-start snapshot, so a builder killed or sent away between
  two decisions leaves the want standing and a frame opens behind it.
  The frame spends nothing, since no builder reaches it, and at the next
  decision `Survey::frame_no_builder_fills` answers yes, `justified`
  reads the want at zero and the frame is cancelled. A bot that surveyed
  and funded every tick would delete it, at a survey and a funding pass
  per seat per tick rather than one a second.
- A machine expects its own controller's command to be one the tick it
  stamps can still take, and the bot meets the thirty-two command cap
  exactly, since its plan ends in a take of that many. A second source of
  commands on one seat at one tick, a draft pick beside a funded plan,
  refuses the thirty-third and panics the process. A controller handing
  over a type that cannot hold more than one tick's commands would delete
  it, since the refusal would have no arm left to reach.
- A relayed command a machine refuses as late or as early is dropped
  without a word, so that machine's history lacks it for good and its
  next hash disagrees, which ends the match. Nothing reads the refusal. A
  relay that could carry only a command the receiving tick can still
  take would delete it.

## Dependencies

| crate | scope | why |
|---|---|---|
| `libm` | `sim`, `agents` | transcendentals identical on every target; `std`'s are the platform's; an agent's arithmetic must replay identically too |
| `mirage-engine` | `game` | the engine, by path; `look`'s bin additionally needs its `offscreen` feature |
| `image` | `game`, behind the `look` feature | writing PNGs; already in the engine's tree |
| `serde` (derive) | `sim`, `protocol`, `agents` by way of them | the derives every wire value takes; adds no arithmetic, so determinism is untouched |
| `ciborium` | `protocol` | one wire encoding for native and the browser: CBOR, `no_std`-capable, self-describing so a record file survives a field addition, and it needs no io in the crate. It beat `postcard`, whose wire is smaller, on reachability (`postcard` is not fetchable in this environment) and on self-description; revisit `postcard` if wire size ever matters |
| `tokio-tungstenite` (with `tungstenite`) | `server`; `game` on native | one WebSocket implementation for both ends of the wire, so the handshake and the framing are the same code. Chosen over hand-rolling a socket, which the environment's blocked downloads would otherwise have forced; `ewebsock`, which would have unified the two client paths behind one API, is not fetchable here |
| `tokio` | `server`; `game` on native | the runtime `tokio-tungstenite` needs: one socket's reads and writes selected over without a timeout or a poll. Chosen over `axum`, which adds `hyper`, routing and a tower stack for a room that needs a listener and a handshake and nothing else, and which stays the right answer when the binary grows a room list; `smol` and `async-std` are not fetchable here |
| `futures-util` | `server`; `game` on native | the `Stream` and `Sink` halves of a WebSocket |
| `web-sys` (`WebSocket`), `js-sys`, `wasm-bindgen` | `game` on wasm32 | the browser's own socket, at the engine's own versions. One link per target rather than one crate over both: nothing fetchable here abstracts a native and a browser socket together, and the two are small enough that a shared abstraction would be longer than either |

Adding one requires a row here, with the reason it was chosen over its
alternatives.
