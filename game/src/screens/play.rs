//! The match: the session every seat is played into, and the pointer's
//! gestures over the belt.

use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::{FrameCtx, Game};
use probe_protocol::Lobby;
use probe_sim::state::Command;
use probe_sim::state::view::View;
use probe_sim::{Band, Place, RowId, SeatId, Session, Vec3};

use crate::controls::{Button, Controls};
use crate::display::camera::BeltCamera;
use crate::display::fights::Fights;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{Client, Hover, Scene, WheelBand};
use crate::display::screen::Screen;
use crate::display::send::Sending;
use crate::display::wheel::Wheel;
use crate::display::{belt, hud};
use crate::net::machine::Machine;
use crate::net::pace::Allowed;
use crate::net::transport::Transport;
use crate::screens::control::{self, Rule};
use crate::screens::held::{self, Held};
use crate::screens::panel::{self, Panel};
use crate::screens::panning::Panning;
use crate::screens::pause::{self, Pause};
use crate::screens::{Playable, results};

/// The eye-to-focus distance a match opens at, in meters: a region of the
/// belt, so the player can pick a rock to start on.
const OPENING_ZOOM: f64 = 6_000.0;

/// How long a held wheel band waits before it repeats, in seconds.
const REPEAT_DELAY: f32 = 1.0 / 3.0;

/// How often a held wheel band repeats after that, in seconds.
const REPEAT_INTERVAL: f32 = 0.1;

/// A left drag from a ring: what it would send, until it is released.
struct Drag {
    from: Place,
    /// Units it moves, off the end of the source run.
    count: u32,
    /// Zoom-axis motion not yet worth a whole unit, in notches.
    adjusted: f32,
}

/// A held wheel band, repeating its edit.
struct Holding {
    place: Place,
    row: RowId,
    band: WheelBand,
    /// Seconds the button has been down.
    held: f32,
    /// Edits issued so far, the first on the press itself.
    edits: u32,
}

/// What the match's own screens ask of the flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    /// Leave the match for the title.
    Leave,
}

/// What the match is doing this frame.
///
/// The belt is read only while it is being played: the pause screen and
/// both holds take every click themselves, so neither can hold a gesture
/// or a preview of one.
enum Doing {
    /// Played, with what the pointer is doing over the belt and what that
    /// shows.
    Playing {
        gesture: Gesture,
        preview: Previewing,
    },
    /// Under the pause screen. A skirmish's sim stops here; a match with
    /// peers plays on, as DISPLAY.md states.
    Paused,
    /// Waiting on a peer, or desynced: the HUD dims and the belt takes no
    /// input.
    Held(Held),
}

/// What the pointer is doing over the belt: one gesture at a time.
#[derive(Default)]
enum Gesture {
    /// Nothing is pressed.
    #[default]
    Still,
    /// A left drag from a ring, until it is released.
    Sending(Drag),
    /// A wheel band held down, repeating its edit.
    Editing(Holding),
}

/// What is under the pointer this frame: the wheel band, the reason that
/// band is disabled, and the ring.
///
/// A band that would change nothing is under the pointer for the reason
/// alone, so it neither presses nor previews.
struct Under {
    slot: Option<(Place, RowId, WheelBand)>,
    refused: Option<String>,
    ring: Option<Place>,
}

/// What the belt shows of the pointer this frame.
#[derive(Default)]
struct Previewing {
    /// The send or the wheel edit the pointer previews.
    hover: Option<Hover>,
    /// Why the wheel band under the pointer is disabled, where one is.
    refused: Option<String>,
}

/// One match in progress: this machine's part of it, the ring its wheel is
/// open on, and what it is doing this frame.
pub struct Play {
    lobby: Lobby,
    machine: Machine,
    view: View,
    fights: Fights,
    camera: BeltCamera,
    selection: Option<Place>,
    doing: Doing,
    /// Whether the focus has followed the player's first placement yet.
    followed: bool,
    panning: Panning,
}

impl Play {
    /// The match `machine` is playing, set up in `lobby`.
    pub fn of(lobby: Lobby, machine: Machine) -> Play {
        let view = machine.view();
        let camera = BeltCamera::new(
            Scene::from_view(
                &view,
                machine.session().state().roster(),
                Client {
                    selection: None,
                    hover: None,
                    fights: &Fights::default(),
                },
            )
            .centre(),
            OPENING_ZOOM,
        );
        Play {
            lobby,
            machine,
            view,
            fights: Fights::default(),
            camera,
            selection: None,
            doing: Doing::played(),
            followed: false,
            panning: Panning::still(),
        }
    }

    /// The seat the person at this machine plays.
    pub fn seat(&self) -> SeatId {
        self.machine.seat()
    }

    /// The tick's fogged view of the match, as the player sees it.
    pub fn view(&self) -> &View {
        &self.view
    }

    /// The session at the tick it shows.
    pub fn session(&self) -> &Session {
        self.machine.session()
    }

    /// The ring the wheel is open on.
    pub fn selection(&self) -> Option<Place> {
        self.selection
    }

    /// What the pointer previews, which only a match being played has.
    pub fn hover(&self) -> Option<&Hover> {
        match &self.doing {
            Doing::Playing { preview, .. } => preview.hover.as_ref(),
            Doing::Paused | Doing::Held(_) => None,
        }
    }

    /// Whether the pause screen is open.
    pub fn paused(&self) -> bool {
        matches!(self.doing, Doing::Paused)
    }

    /// True once the clock has run out, which is the one end a client can
    /// see: DESIGN.md's fog reveals the standings then and never before, so
    /// a seat cannot know another side was eliminated.
    pub fn over(&self) -> bool {
        self.view.standings.is_some()
    }

    /// The score of the match, over the belt as its last frame drew it.
    pub fn ends(&self) -> results::Results {
        results::Results::of(
            self.lobby.clone(),
            self.scene(),
            self.camera,
            self.machine.session().state(),
        )
    }

    /// The camera the belt is drawn through.
    pub fn camera(&self) -> &BeltCamera {
        &self.camera
    }

    /// Where `rock` is this tick, in meters; `None` for no such rock.
    pub fn rock_pos(&self, rock: probe_sim::RockId) -> Option<Vec3> {
        self.view
            .terrain
            .iter()
            .find(|terrain| terrain.rock == rock)
            .map(|terrain| terrain.orbit.at(self.view.tick, self.view.gravity).pos)
    }

    /// The wheel open on the selected ring, as `screen` projects its rock;
    /// `None` with nothing selected or the rock off screen.
    pub fn wheel(&self, screen: &Screen) -> Option<Wheel> {
        let place = self.selection?;
        let centre = screen.point_of(self.rock_pos(place.rock)?)?;
        Some(Wheel::open(
            place,
            self.machine.seat(),
            self.machine.session().state().roster(),
            centre,
        ))
    }

    /// One tick of the match, through `transport`: this machine's part of
    /// it, and then what the frame draws from.
    ///
    /// A skirmish stops under the pause screen, as DISPLAY.md states, and a
    /// match past its clock advances nothing, since the standings the
    /// results screen shows are already final.
    pub fn tick(&mut self, transport: &mut dyn Transport) {
        if (self.paused() && self.machine.alone()) || self.over() {
            return;
        }
        let ticked = self.machine.tick(transport);
        if ticked.rewound {
            self.fights = Fights::default();
            self.forgets();
        }
        self.holds(ticked.pace);
        self.view = self.machine.view();
        if ticked.pace != Allowed::Advance {
            return;
        }
        self.fights.observe(&self.view);
        self.camera
            .advance(probe_sim::TICK.as_secs_f64(), self.view.gravity);
        self.follow_the_first_placement();
    }

    /// One frame of the match: the input, the belt, the HUD, and the pause
    /// or held screen over them, and what the player picked in those.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Picked>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let window = ctx.window_size();
        // The painter's own measure is only reachable through the UI layer,
        // and both layers size their glyphs by it.
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());

        let screen = Screen::of(&self.camera, window, points_per_pixel);
        let aimed = self.wheel(&screen);
        self.read_input(ctx, &screen, aimed.as_ref());

        // Picking read the projection the player last saw; the camera this
        // frame's input moved is what the frame then draws through.
        let screen = Screen::of(&self.camera, window, points_per_pixel);
        let wheel = self.wheel(&screen);
        let scene = self.scene();

        belt::draw(&scene, &screen, ctx);
        let pointer = screen.point_at(ctx.pointer());
        let clicked = ctx.pressed(Button::Select);
        let over = panel::window_of(window, points_per_pixel);
        let sentence = self.sentence(&scene, &screen, pointer);
        let doing = &self.doing;
        let mut left = None;
        let mut resumed = false;
        ctx.ui(|ui| {
            hud::paint(&scene, &screen, wheel.as_ref(), ui.painter());
            let panel = Panel::new(ui.painter(), over, pointer, clicked);
            if let Some((beside, sentence)) = sentence {
                let mut controls = control::Controls::over(&panel);
                controls.note(beside, sentence);
                controls.finish();
            }
            match doing {
                Doing::Playing { .. } => {}
                Doing::Paused => match Pause.frame(&panel) {
                    Some(pause::Picked::Resume) => resumed = true,
                    Some(pause::Picked::Leave) => left = Some(Picked::Leave),
                    None => {}
                },
                Doing::Held(held) => {
                    if held.frame(&panel) == Some(held::Picked::Leave) {
                        left = Some(Picked::Leave);
                    }
                }
            }
        });
        if resumed {
            self.doing = Doing::played();
        }
        left
    }

    /// The belt as this tick's view lays it, with what the pointer is doing
    /// over it.
    fn scene(&self) -> Scene {
        Scene::from_view(
            &self.view,
            self.machine.session().state().roster(),
            Client {
                selection: self.selection,
                hover: self.hover().cloned(),
                fights: &self.fights,
            },
        )
    }

    /// The one sentence the pointer is over, and what to stand it beside:
    /// a disabled wheel band's reason, else the state of the run glyph
    /// under the pointer.
    fn sentence(
        &self,
        scene: &Scene,
        screen: &Screen,
        pointer: egui::Pos2,
    ) -> Option<(egui::Rect, String)> {
        let beside = |at: egui::Pos2| {
            egui::Rect::from_center_size(at, egui::Vec2::splat(2.0 * crate::display::glyph::HALF))
        };
        if let Doing::Playing { preview, .. } = &self.doing
            && let Some(refused) = &preview.refused
        {
            return Some((beside(pointer), refused.clone()));
        }
        let (at, mark) = hud::glyph_at(scene, screen, pointer)?;
        let roster = self.machine.session().state().roster();
        Some((beside(at), mark.reason.sentence(roster)))
    }

    /// Holds the match where `pace` says to, and plays it again where a
    /// hold has passed. The pause screen is the player's own, so a hold
    /// waits behind it and is read again on the tick after they resume.
    fn holds(&mut self, pace: Allowed) {
        if self.paused() {
            return;
        }
        match self.holding(pace) {
            Some(held) => self.doing = Doing::Held(held),
            None if matches!(self.doing, Doing::Held(_)) => self.doing = Doing::played(),
            None => {}
        }
    }

    /// Drops what the client remembers of what it saw, which a rewind may
    /// have made stale.
    fn forgets(&mut self) {
        if let Doing::Playing { preview, .. } = &mut self.doing {
            preview.hover = None;
        }
    }

    /// Why the match is holding under `pace`, where it is.
    fn holding(&self, pace: Allowed) -> Option<Held> {
        match (self.machine.desynced(), pace) {
            (Some(tick), _) => Some(Held::Desynced(tick)),
            (None, Allowed::Held) => Some(Held::Waiting(self.machine.waiting())),
            (None, Allowed::Advance | Allowed::Slowed) => None,
        }
    }

    /// Where the player's own first entity stands, once it exists: the
    /// focus follows it, as DISPLAY.md's camera states.
    fn follow_the_first_placement(&mut self) {
        if self.followed {
            return;
        }
        let Some(place) = self
            .view
            .seen
            .iter()
            .filter(|seen| seen.seat == self.machine.seat())
            .find_map(|seen| seen.home)
        else {
            return;
        };
        if let Some(pos) = self.rock_pos(place.rock) {
            self.camera.set_focus(pos);
            self.followed = true;
        }
    }

    /// The ring under `at`, in points: within the inner ring's radius of a
    /// rock's centre is its inner band, within the outer ring's is its
    /// outer band, and the nearest rock wins.
    fn ring_at(&self, screen: &Screen, at: egui::Pos2) -> Option<Place> {
        self.view
            .terrain
            .iter()
            .filter_map(|terrain| {
                let centre = screen.point_of(self.rock_pos(terrain.rock)?)?;
                let away = centre.distance(at);
                let band = if away <= hud::ring_radius(Band::Inner) {
                    Band::Inner
                } else if away <= hud::ring_radius(Band::Outer) {
                    Band::Outer
                } else {
                    return None;
                };
                Some((
                    away,
                    Place {
                        rock: terrain.rock,
                        band,
                    },
                ))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, place)| place)
    }

    /// What the seat wants of `row` at `place` now.
    fn wanted(&self, place: Place, row: RowId) -> u32 {
        self.view
            .compositions
            .iter()
            .filter(|composition| composition.place == place)
            .flat_map(|composition| &composition.rows)
            .find(|wanted| wanted.row == row)
            .map_or(0, |wanted| wanted.want)
    }

    /// Asks the person's own controller for `command`, which the next tick
    /// stamps and issues.
    fn issue(&mut self, command: Command) {
        if let Some(human) = self.machine.human() {
            human.want(command);
        }
    }

    /// Reads one frame of input: the pause key, and, while the match is
    /// being played, the camera and the pointer's gestures over `aimed`,
    /// the wheel as the player saw it.
    ///
    /// A pause and a hold take every click themselves, so the belt reads
    /// nothing under either and the gesture the pointer was in the middle
    /// of is dropped with the frame it began.
    fn read_input<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        seen: &Screen,
        aimed: Option<&Wheel>,
    ) {
        if ctx.pressed(Button::Pause)
            && let Some(doing) = self.doing.pausing()
        {
            self.doing = doing;
        }
        let Doing::Playing { gesture, .. } = &mut self.doing else {
            return;
        };
        let gesture = core::mem::take(gesture);
        let at = seen.point_at(ctx.pointer());
        let dt = ctx.dt().as_secs_f32();
        // The wheel adjusts how many units a send moves while one is in
        // progress, so it is not zooming then.
        let sending = matches!(gesture, Gesture::Sending(_));
        let notches = self
            .panning
            .drag(ctx, &mut self.camera, seen.window(), !sending);
        let under = self.under(seen, aimed, at);
        let (gesture, preview) = self.pointed(ctx, under, gesture, dt, notches);
        self.doing = Doing::Playing { gesture, preview };
    }

    /// What is under the pointer at `at`, of the wheel `aimed` as the
    /// player saw it and the rings `seen` projects.
    fn under(&self, seen: &Screen, aimed: Option<&Wheel>, at: egui::Pos2) -> Under {
        let aimed_at = aimed.and_then(|wheel| {
            wheel
                .slot_at(at)
                .map(|(row, band)| (wheel.place(), row, band))
        });
        let refused = aimed_at
            .map(|(place, row, band)| self.band_rule(place, row, band))
            .and_then(|rule| rule.why().map(str::to_string));
        Under {
            slot: aimed_at.filter(|_| refused.is_none()),
            refused,
            ring: self.ring_at(seen, at),
        }
    }

    /// The pointer's own gestures over the belt: the press, the drag, the
    /// release, the held repeat, and what they preview.
    fn pointed<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        under: Under,
        gesture: Gesture,
        dt: f32,
        notches: f32,
    ) -> (Gesture, Previewing) {
        let Under {
            slot,
            refused,
            ring,
        } = under;
        let mut gesture = gesture;
        if let Gesture::Sending(drag) = &mut gesture {
            drag.adjust(notches);
        }
        if ctx.pressed(Button::Select) {
            gesture = self.pressed(slot, ring);
        }
        if ctx.released(Button::Select) {
            gesture = self.released(gesture, ring);
        }
        if let Gesture::Editing(holding) = &mut gesture
            && holding.repeats(slot, dt)
        {
            let (place, row, band) = (holding.place, holding.row, holding.band);
            self.edit(place, row, band);
        }

        let hover = match (&gesture, slot) {
            (Gesture::Sending(drag), _) => ring.filter(|to| *to != drag.from).map(|to| {
                Hover::Send(Sending {
                    from: drag.from,
                    to,
                    count: drag.count,
                })
            }),
            (_, Some((place, row, band))) => Some(Hover::Wheel { place, row, band }),
            (Gesture::Still | Gesture::Editing(_), None) => None,
        };
        (gesture, Previewing { hover, refused })
    }

    /// What a press over `slot`, the wheel band under the pointer, or
    /// `ring`, the ring under it, begins: a wheel edit that repeats while
    /// it is held, a send off that ring, or the closing of the wheel.
    fn pressed(&mut self, slot: Option<(Place, RowId, WheelBand)>, ring: Option<Place>) -> Gesture {
        match (slot, ring) {
            (Some((place, row, band)), _) => {
                self.edit(place, row, band);
                Gesture::Editing(Holding {
                    place,
                    row,
                    band,
                    held: 0.0,
                    edits: 1,
                })
            }
            (None, Some(from)) => Gesture::Sending(Drag {
                from,
                count: Sending::present(&self.view, from, self.machine.session().state().roster()),
                adjusted: 0.0,
            }),
            (None, None) => {
                self.selection = None;
                Gesture::Still
            }
        }
    }

    /// What a release ends: a drag over another ring is the send, and one
    /// over the ring it started on selects that ring and focuses its rock.
    fn released(&mut self, gesture: Gesture, ring: Option<Place>) -> Gesture {
        if let Gesture::Sending(drag) = gesture {
            match ring {
                Some(to) if to != drag.from => {
                    let sending = Sending {
                        from: drag.from,
                        to,
                        count: drag.count,
                    };
                    let roster = self.machine.session().state().roster();
                    for command in sending.commands(&self.view, roster) {
                        self.issue(command);
                    }
                }
                Some(place) => self.focuses(place),
                None => {}
            }
        }
        Gesture::Still
    }

    /// Selects `place`'s ring and puts the focus on its rock.
    fn focuses(&mut self, place: Place) {
        self.selection = Some(place);
        if let Some(pos) = self.rock_pos(place.rock) {
            self.camera.set_focus(pos);
            self.followed = true;
        }
    }

    /// One count edit of `row` at `place`, as the band names it.
    fn edit(&mut self, place: Place, row: RowId, band: WheelBand) {
        let command = band.edit(place, row, self.wanted(place, row));
        self.issue(command);
    }

    /// Whether a click on `band` changes what `place` wants of `row`, and
    /// the sentence it shows while it does not: the wheel's plus stops at
    /// the cap a want command carries and its minus at none.
    fn band_rule(&self, place: Place, row: RowId, band: WheelBand) -> Rule {
        let wanted = self.wanted(place, row);
        match band {
            WheelBand::Plus => Rule::only_if(
                wanted < probe_sim::state::MAX_WANT,
                "This is the most you can want here",
            ),
            WheelBand::Minus => Rule::only_if(wanted > 0, "You want none here"),
        }
    }
}

impl Doing {
    /// A match being played, with the pointer over nothing.
    fn played() -> Doing {
        Doing::Playing {
            gesture: Gesture::Still,
            preview: Previewing::default(),
        }
    }

    /// What Escape does: it opens the pause screen over a match being
    /// played and closes it again. `None` while the match is held, which
    /// offers only its own control.
    fn pausing(&self) -> Option<Doing> {
        match self {
            Doing::Playing { .. } => Some(Doing::Paused),
            Doing::Paused => Some(Doing::played()),
            Doing::Held(_) => None,
        }
    }
}

impl Drag {
    /// Takes `notches` of the zoom axis as units added to or taken off the
    /// send, whole units at a time.
    fn adjust(&mut self, notches: f32) {
        self.adjusted += notches;
        while self.adjusted >= 1.0 {
            self.adjusted -= 1.0;
            self.count += 1;
        }
        while self.adjusted <= -1.0 {
            self.adjusted += 1.0;
            self.count = self.count.saturating_sub(1);
        }
    }
}

impl Holding {
    /// Whether another edit is due `dt` seconds on, with the pointer still
    /// over `slot`: the first repeat [`REPEAT_DELAY`] after the press and
    /// one every [`REPEAT_INTERVAL`] after that.
    ///
    /// A repeat follows the band the press landed on: slide off it and the
    /// repeat stops rather than editing another row.
    fn repeats(&mut self, slot: Option<(Place, RowId, WheelBand)>, dt: f32) -> bool {
        if slot != Some((self.place, self.row, self.band)) {
            return false;
        }
        self.held += dt;
        let due = self.held >= REPEAT_DELAY + REPEAT_INTERVAL * (self.edits - 1) as f32;
        self.edits += u32::from(due);
        due
    }
}

#[cfg(test)]
mod tests {
    use probe_protocol::{Lobby, PlayerId};
    use probe_sim::{Band, RockId};

    use super::*;
    use crate::net::local::Local;

    /// A skirmish being played, with the pointer in the middle of a send.
    fn dragging() -> Play {
        let lobby = Lobby::skirmish(PlayerId::HOST);
        let started = lobby.freeze().expect("a skirmish is a match");
        let crew = started
            .seating()
            .run_by(PlayerId::HOST)
            .expect("the host holds a seat");
        let mut play = Play::of(lobby, Machine::of(started, &crew, &mut Local));
        let from = Place {
            rock: RockId(0),
            band: Band::Inner,
        };
        play.doing = Doing::Playing {
            gesture: Gesture::Sending(Drag {
                from,
                count: 1,
                adjusted: 0.0,
            }),
            preview: Previewing {
                hover: Some(Hover::Send(Sending {
                    from,
                    to: Place {
                        rock: RockId(1),
                        band: Band::Inner,
                    },
                    count: 1,
                })),
                refused: None,
            },
        };
        play
    }

    #[test]
    fn a_match_that_begins_to_hold_drops_the_gesture_and_the_preview_the_pointer_had() {
        let mut play = dragging();

        play.holds(Allowed::Held);

        assert!(matches!(play.doing, Doing::Held(Held::Waiting(_))));
        assert!(
            play.hover().is_none(),
            "a held match previews nothing over the belt"
        );
    }

    #[test]
    fn the_pause_screen_does_not_open_over_a_held_match() {
        let held = Doing::Held(Held::Waiting(Vec::new()));

        assert!(
            held.pausing().is_none(),
            "a held match offers only its own control"
        );
        assert!(matches!(Doing::played().pausing(), Some(Doing::Paused)));
        assert!(matches!(
            Doing::Paused.pausing(),
            Some(Doing::Playing { .. })
        ));
    }

    #[test]
    fn a_hold_that_passes_plays_the_match_again_with_the_pointer_over_nothing() {
        let mut play = dragging();
        play.holds(Allowed::Held);

        play.holds(Allowed::Advance);

        assert!(matches!(
            play.doing,
            Doing::Playing {
                gesture: Gesture::Still,
                ..
            }
        ));
    }
}
