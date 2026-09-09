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
- `Send::joining` takes the first flight it finds between a source, a
  destination and a seat that has not departed, trusting that at most one
  exists, which holds because a send is solved only where none is found,
  and the one that is found always departs before the window could open a
  second. A store keyed by those three would make it unrepresentable, at
  the price of a second place for a send to live and be reaped; the
  flight the units already carry is the cheaper truth.
- `Schedule::between` gives up after `CORRECTIONS` passes. The margin is
  the predicate and the cap is only its guard: at the shipped
  `BURN_SHARE_OF_SPAN` of one half, a neighbour hop over the shipped belt
  takes three passes or four and never a fifth, and a candidate the cap
  rejected would be one the sim could not have flown within the
  tolerance. A proof that the aim correction converges for every schedule
  inside the margin would delete the cap.
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
- A `Preview` settles as though every send it names departs, so it can
  skip the schedule solve. Where no schedule exists this tick the sim
  will instead leave those units home and open frames, and the hover will
  have said otherwise for one tick. Solving inside the preview would
  delete it, at the price of the solve on every pointer move.
- A bot's frame may stand for one decision at an asteroid where no
  builder of its seat stands or arrives: a unit lost in the step a want
  lands in leaves a shortfall the plan answers at its next decision. A
  plan that saw the step's deaths before its wants landed would delete it,
  which the one-verb view cannot give.

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
