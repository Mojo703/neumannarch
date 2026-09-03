//! The match: the session every seat is played into, and the pointer's
//! gestures over the belt.

use mirage_engine::egui;
use mirage_engine::math::Vec2;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::{FrameCtx, Game};
use probe_protocol::{Control, Lobby, Message, PlayerId};
use probe_sim::state::Command;
use probe_sim::state::view::View;
use probe_sim::step::fire::Shots;
use probe_sim::{Band, Place, Retention, Rewound, RowId, SeatId, Session, Setup, Stamped, Vec3};

use crate::controls::{Axis, Axis2, Button, Controls};
use crate::display::camera::BeltCamera;
use crate::display::fights::Fights;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{Client, Hover, Scene, WheelBand};
use crate::display::screen::Screen;
use crate::display::send::Sending;
use crate::display::wheel::Wheel;
use crate::display::{belt, hud};
use crate::net::controller::Controller;
use crate::net::local::Local;
use crate::net::transport::Transport;
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};
use crate::screens::pause::Pause;
use crate::screens::{Playable, results};

/// The eye-to-focus distance a match opens at, in meters: a region of the
/// belt, so the player can pick a rock to start on.
const OPENING_ZOOM: f64 = 6_000.0;

/// How long a held wheel band waits before it repeats, in seconds.
const REPEAT_DELAY: f32 = 1.0 / 3.0;

/// How often a held wheel band repeats after that, in seconds.
const REPEAT_INTERVAL: f32 = 0.1;

/// How fast the pan keys move the belt, in points a second.
const KEY_PAN: f32 = 900.0;

/// What one notch of the zoom axis multiplies the eye-to-focus distance by.
const ZOOM_STEP: f64 = 1.25;

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
    row: RowId,
    band: WheelBand,
    /// Seconds the button has been down.
    held: f32,
    /// Edits issued so far, the first on the press itself.
    edits: u32,
}

/// One match in progress: the session, one controller per seat, the
/// transport its peers hear it through, and what the pointer is doing.
pub struct Play {
    lobby: Lobby,
    seat: SeatId,
    session: Session,
    controllers: Vec<Controller>,
    transport: Local,
    view: View,
    fights: Fights,
    camera: BeltCamera,
    selection: Option<Place>,
    hover: Option<Hover>,
    drag: Option<Drag>,
    holding: Option<Holding>,
    paused: bool,
    /// Whether the focus has followed the player's first placement yet.
    followed: bool,
    /// Where the pointer was last frame, in physical pixels.
    pointer: Vec2,
}

impl Play {
    /// The match `setup` names, with one controller per seat `lobby` gives
    /// and the person at this machine playing `me`'s slot.
    pub fn of(lobby: Lobby, setup: Setup, me: PlayerId) -> Play {
        // A skirmish seats its host, and the server unit seats a joiner
        // before it starts the match, so the person always holds a slot.
        let seat = lobby
            .slot_of(me)
            .and_then(|slot| lobby.seat_of(slot))
            .expect("the person at this machine holds a slot");
        let local: Vec<SeatId> = lobby
            .slots()
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.holds(me) || matches!(slot.control, Control::Bot(_)))
            .filter_map(|(at, _)| lobby.seat_of(at))
            .collect();
        // Every seat `seat_of` names is one of the setup's, since a freeze
        // seats exactly the slots that are not closed.
        let session = Session::new(setup, Retention::shipped(), &local)
            .expect("a frozen lobby seats every slot it names");
        let controllers = Controller::of(&lobby, me, session.state().roster());
        let view = View::of(session.state(), seat, &Shots::default());
        let camera = BeltCamera::new(
            Scene::from_view(
                &view,
                session.state().roster(),
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
            seat,
            session,
            controllers,
            transport: Local,
            view,
            fights: Fights::default(),
            camera,
            selection: None,
            hover: None,
            drag: None,
            holding: None,
            paused: false,
            followed: false,
            pointer: Vec2::ZERO,
        }
    }

    /// The seat the person at this machine plays.
    pub fn seat(&self) -> SeatId {
        self.seat
    }

    /// The tick's fogged view of the match, as the player sees it.
    pub fn view(&self) -> &View {
        &self.view
    }

    /// The session at the tick it shows.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The ring the wheel is open on.
    pub fn selection(&self) -> Option<Place> {
        self.selection
    }

    /// What the pointer previews.
    pub fn hover(&self) -> Option<&Hover> {
        self.hover.as_ref()
    }

    /// Whether the pause screen is open.
    pub fn paused(&self) -> bool {
        self.paused
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
            self.seat,
            self.session.state().roster(),
            centre,
        ))
    }

    /// One tick of the match: every controller's commands, then the step.
    ///
    /// A skirmish stops under the pause screen, as DISPLAY.md states, and
    /// a match past its clock advances nothing, since the standings the
    /// results screen shows are already final.
    pub fn tick(&mut self) {
        if self.paused || self.over() {
            return;
        }
        self.speak();
        self.hear();
        self.session.advance();
        self.tell();
        self.view = self.viewed();
        self.fights.observe(&self.view);
        self.camera
            .advance(probe_sim::TICK.as_secs_f64(), self.view.gravity);
        self.follow_the_first_placement();
    }

    /// One frame of the match: the input, the belt, the HUD, and the pause
    /// screen over them.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Step>
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
        let scene = Scene::from_view(
            &self.view,
            self.session.state().roster(),
            Client {
                selection: self.selection,
                hover: self.hover.clone(),
                fights: &self.fights,
            },
        );

        belt::draw(&scene, &screen, ctx);
        let pointer = screen.point_at(ctx.pointer());
        let clicked = ctx.pressed(Button::Select);
        let over = panel::window_of(window, points_per_pixel);
        let paused = self.paused;
        let mut picked = None;
        ctx.ui(|ui| {
            hud::paint(&scene, &screen, wheel.as_ref(), ui.painter());
            if paused {
                let panel = Panel::new(ui.painter(), over, pointer, clicked);
                picked = Pause.frame(&panel);
            }
        });
        match picked {
            Some(Step::Resume) => {
                self.paused = false;
                None
            }
            Some(step) => Some(step),
            None if self.over() => Some(Step::Results(Box::new(results::Results::of(
                self.lobby.clone(),
                scene,
                self.camera,
                self.session.state(),
            )))),
            None => None,
        }
    }

    /// True once the clock has run out, which is the one end a client can
    /// see: DESIGN.md's fog reveals the standings then and never before, so
    /// a seat cannot know another side was eliminated.
    fn over(&self) -> bool {
        self.view.standings.is_some()
    }

    /// The player's fogged view of the tick the session shows.
    fn viewed(&self) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), self.seat, shots)
    }

    /// Every local controller's commands, into this session at once and on
    /// to the peers.
    fn speak(&mut self) {
        let issued: Vec<Stamped> = self
            .controllers
            .iter_mut()
            .flat_map(|controller| controller.issue(&self.session))
            .collect();
        for stamped in issued {
            // A controller stamps the tick the session shows and yields at
            // most that tick's cap, so the batch takes every one.
            self.session
                .insert(stamped)
                .expect("a controller's own command is one this tick takes");
            self.transport.send(stamped);
        }
    }

    /// What this machine now knows: how far each seat it owns is
    /// acknowledged, and its hash at the settled tick, which is the tick
    /// hashes are compared at.
    fn tell(&mut self) {
        let latest = self.session.state().tick();
        let settled = self.session.settled();
        for seat in self
            .controllers
            .iter()
            .filter(|controller| !matches!(controller, Controller::Remote(_)))
            .map(Controller::seat)
            .collect::<Vec<_>>()
        {
            self.transport.acknowledge(seat, latest);
        }
        if let Some(hash) = self.session.hash_at(settled) {
            self.transport.report(settled, hash);
        }
    }

    /// What the peers have said since the last tick.
    fn hear(&mut self) {
        for message in self.transport.received() {
            match message {
                Message::Command(stamped) => {
                    // A peer outside the window has already been cut off by
                    // the pacing rule, which lands with the socket.
                    if let Ok(Rewound::From(_)) = self.session.insert(stamped) {
                        self.fights = Fights::default();
                        self.hover = None;
                    }
                }
                Message::Acknowledge { seat, up_to } => self.session.acknowledge(seat, up_to),
                Message::Hash { .. } | Message::Desync { .. } => {}
                // The room's lobby phase is over once a match is running.
                Message::Join
                | Message::Welcome { .. }
                | Message::Edit(_)
                | Message::Lobby(_)
                | Message::Start(_)
                | Message::Leave => {}
            }
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
            .filter(|seen| seen.seat == self.seat)
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
        let seat = self.seat;
        if let Some(human) = self
            .controllers
            .iter_mut()
            .find(|controller| controller.seat() == seat)
            .and_then(Controller::human)
        {
            human.want(command);
        }
    }

    /// Reads one frame of input: the pause key, the camera, and the
    /// pointer's gestures over `aimed`, the wheel as the player saw it.
    fn read_input<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        seen: &Screen,
        aimed: Option<&Wheel>,
    ) {
        if ctx.pressed(Button::Pause) {
            self.paused = !self.paused;
        }
        let pointer = ctx.pointer();
        let moved = pointer - self.pointer;
        self.pointer = pointer;
        if self.paused {
            return;
        }
        let dt = ctx.dt().as_secs_f32();
        let window = seen.window();

        if ctx.down(Button::Pan) {
            self.camera.pan_by_pixels(moved, window);
        }
        let keys = ctx.axis2(Axis2::Pan);
        if keys != Vec2::ZERO {
            self.camera
                .pan_by_pixels(Vec2::new(-keys.x, keys.y) * KEY_PAN * dt, window);
        }

        let notches = ctx.axis(Axis::Zoom);
        match &mut self.drag {
            // The wheel adjusts how many units a send moves while one is in
            // progress, so it is not zooming then.
            Some(drag) => drag.adjust(notches),
            None if notches != 0.0 => self.camera.zoom(ZOOM_STEP.powf(f64::from(-notches))),
            None => {}
        }

        self.point(ctx, seen, aimed, seen.point_at(pointer), dt);
    }

    /// The pointer's own gestures: the press, the drag, the release, the
    /// held repeat, and the hover preview.
    fn point<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        seen: &Screen,
        aimed: Option<&Wheel>,
        at: egui::Pos2,
        dt: f32,
    ) {
        let slot = aimed.and_then(|wheel| {
            wheel
                .slot_at(at)
                .map(|(row, band)| (wheel.place(), row, band))
        });
        let ring = self.ring_at(seen, at);

        if ctx.pressed(Button::Select) {
            match (slot, ring) {
                (Some((place, row, band)), _) => {
                    self.edit(place, row, band);
                    self.holding = Some(Holding {
                        row,
                        band,
                        held: 0.0,
                        edits: 1,
                    });
                }
                (None, Some(from)) => {
                    self.drag = Some(Drag {
                        from,
                        count: Sending::present(&self.view, from, self.session.state().roster()),
                        adjusted: 0.0,
                    });
                }
                (None, None) => self.selection = None,
            }
        }

        if ctx.released(Button::Select) {
            self.holding = None;
            if let Some(drag) = self.drag.take() {
                self.finish(drag, ring);
            }
        }

        // A repeat follows the band the press landed on: slide off it and
        // the repeat stops rather than editing another row.
        if let (Some(holding), Some((place, row, band))) = (&mut self.holding, slot)
            && (holding.row, holding.band) == (row, band)
        {
            holding.held += dt;
            if holding.due() {
                holding.edits += 1;
                self.edit(place, row, band);
            }
        }

        self.hover = match (&self.drag, slot) {
            (Some(drag), _) => ring.filter(|to| *to != drag.from).map(|to| {
                Hover::Send(Sending {
                    from: drag.from,
                    to,
                    count: drag.count,
                })
            }),
            (None, Some((place, row, band))) => Some(Hover::Wheel { place, row, band }),
            (None, None) => None,
        };
    }

    /// One count edit of `row` at `place`, as the band names it.
    fn edit(&mut self, place: Place, row: RowId, band: WheelBand) {
        let command = band.edit(place, row, self.wanted(place, row));
        self.issue(command);
    }

    /// Ends a drag: a release over another ring is the send, and one over
    /// the ring it started on selects that ring and focuses its rock.
    fn finish(&mut self, drag: Drag, ring: Option<Place>) {
        match ring {
            Some(to) if to != drag.from => {
                let sending = Sending {
                    from: drag.from,
                    to,
                    count: drag.count,
                };
                for command in sending.commands(&self.view, self.session.state().roster()) {
                    self.issue(command);
                }
            }
            Some(place) => {
                self.selection = Some(place);
                if let Some(pos) = self.rock_pos(place.rock) {
                    self.camera.set_focus(pos);
                    self.followed = true;
                }
            }
            None => {}
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
    /// Whether the next repeat is due: the first [`REPEAT_DELAY`] after the
    /// press, and one every [`REPEAT_INTERVAL`] after that.
    fn due(&self) -> bool {
        self.held >= REPEAT_DELAY + REPEAT_INTERVAL * (self.edits - 1) as f32
    }
}
