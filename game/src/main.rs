//! Probe Game on the Mirage engine: one local match, seat zero the
//! player's, over the sim's fogged view.

use mirage_engine::egui;
use mirage_engine::math::{UVec2, Vec2};
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use probe_game::camera::BeltCamera;
use probe_game::fights::Fights;
use probe_game::glyph_quad::GlyphQuad;
use probe_game::scene::{Client, Hover, Scene, WheelBand};
use probe_game::screen::Screen;
use probe_game::send::Sending;
use probe_game::wheel::Wheel;
use probe_game::{belt, hud};
use probe_sim::belt::Belt;
use probe_sim::session::Session;
use probe_sim::state::view::View;
use probe_sim::state::{Command, Issued, State};
use probe_sim::step::fire::Shots;
use probe_sim::{Band, Place, RowId, SeatId, TICKS_PER_SECOND, TeamId, Tick, Vec3};

meshes! { enum Shape { Sphere, GlyphQuad } }

/// The window's size, in logical pixels.
const WINDOW: UVec2 = UVec2::new(1280, 720);

/// The clock a match ends at: fifteen minutes.
const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

/// The seat the player holds. Seat one is an opponent that issues nothing.
const PLAYER: SeatId = SeatId(0);

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

/// How far one turn of the mouse wheel counts as, in notches.
const WHEEL_NOTCH: f32 = 1.0;

/// How far the zoom keys count as per frame, in notches, so a held key
/// zooms smoothly where a wheel jumps.
const ZOOM_KEY_NOTCH: f32 = 0.06;

fn main() {
    run(
        Config::new("Probe Game")
            .with_size(WINDOW.x, WINDOW.y)
            .with_tick_interval(probe_sim::TICK),
        |_| Ok(Play::new()),
    );
}

/// What the player can do. The gamepad bindings DISPLAY.md states are a
/// later unit; every action here is keyboard and mouse.
#[derive(Buttons, Clone, Copy)]
enum Button {
    /// Select a ring, edit a wheel band, or start a send.
    Select,
    /// Drag the belt under the pointer.
    Pan,
    Pause,
}

#[derive(Axes, Clone, Copy)]
enum Axis {
    /// Zoom, or, during a send, how many units go.
    Zoom,
}

#[derive(Axes2, Clone, Copy)]
enum Axis2 {
    Pan,
}

struct Controls;

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

/// One local match: the session, the tick's view of it, and what the
/// pointer is doing to it.
struct Play {
    session: Session,
    view: View,
    fights: Fights,
    camera: BeltCamera,
    selection: Option<Place>,
    hover: Option<Hover>,
    drag: Option<Drag>,
    holding: Option<Holding>,
    /// Commands the input has issued since the last tick.
    pending: Vec<Issued>,
    paused: bool,
    /// Whether the focus has followed the player's first placement yet.
    followed: bool,
    /// Where the pointer was last frame, in physical pixels.
    pointer: Vec2,
}

impl ActionSet for Controls {
    type Axis = Axis;
    type Axis2 = Axis2;
    type Button = Button;
}

impl BindAxis for Axis {
    fn bindings(&self) -> Vec<AxisBinding> {
        match self {
            Axis::Zoom => vec![
                AxisBinding::motion(Motion::Wheel).scale(WHEEL_NOTCH),
                AxisBinding::from(ButtonAxis {
                    negative: Key::Q,
                    positive: Key::E,
                })
                .scale(ZOOM_KEY_NOTCH),
            ],
        }
    }
}

impl BindAxis2 for Axis2 {
    fn bindings(&self) -> Vec<Axis2Binding> {
        match self {
            Axis2::Pan => vec![
                ButtonAxis2 {
                    left: Key::A,
                    right: Key::D,
                    down: Key::S,
                    up: Key::W,
                }
                .into(),
                ButtonAxis2 {
                    left: Key::Left,
                    right: Key::Right,
                    down: Key::Down,
                    up: Key::Up,
                }
                .into(),
            ],
        }
    }
}

impl BindButton for Button {
    fn bindings(&self) -> Vec<ButtonBinding> {
        match self {
            Button::Select => vec![MouseButton::Left.into()],
            Button::Pan => vec![MouseButton::Right.into(), MouseButton::Middle.into()],
            Button::Pause => vec![Key::Escape.into()],
        }
    }
}

impl Game for Play {
    type Actions = Controls;
    type Meshes = Shape;
    type Sound = NoSound;
    type Sources = NoSources;
    type Styles = ();

    /// Advances the match by the commands the input issued, then reads the
    /// tick's view. A paused match, and one past its clock, advances
    /// nothing, so the HUD stops with it.
    fn tick(&mut self, _ctx: &mut TickCtx<'_, Self>) {
        if self.paused || self.over() {
            return;
        }
        let issued = core::mem::take(&mut self.pending);
        self.session.advance(issued);
        self.view = View::of(self.session.state(), PLAYER, self.session.shots());
        self.fights.observe(&self.view);
        self.camera
            .advance(probe_sim::TICK.as_secs_f64(), self.view.gravity);
        self.follow_the_first_placement();
    }

    fn frame(&mut self, ctx: &mut FrameCtx<'_, Self>) {
        let window = ctx.window_size();
        // The painter's own measure is only reachable through the UI layer,
        // and both layers size their glyphs by it.
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());

        // Picking reads the projection the player last saw; the camera this
        // frame's input moves is what the frame then draws through.
        let seen = Screen::of(&self.camera, window, points_per_pixel);
        let aimed = self.wheel(&seen);
        self.read_input(ctx, &seen, aimed.as_ref());

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
        let menu = self.menu();
        let mut resumed = false;
        ctx.ui(|ui| {
            hud::paint(&scene, &screen, wheel.as_ref(), ui.painter());
            if let Some(menu) = &menu {
                resumed = menu.show(ui);
            }
        });
        if resumed {
            self.paused = false;
        }
    }
}

impl Play {
    fn new() -> Play {
        let state = State::start(
            CLOCK,
            Belt::GRAVITY,
            Belt::fixed(Belt::GRAVITY),
            &[TeamId(0), TeamId(1)],
        );
        let view = View::of(&state, PLAYER, &Shots::default());
        let camera = BeltCamera::new(belt_centre(&view), OPENING_ZOOM);
        Play {
            session: Session::new(state),
            view,
            fights: Fights::default(),
            camera,
            selection: None,
            hover: None,
            drag: None,
            holding: None,
            pending: Vec::new(),
            paused: false,
            followed: false,
            pointer: Vec2::ZERO,
        }
    }

    /// True once the clock has run out, when the standings are the one
    /// panel DISPLAY.md allows.
    fn over(&self) -> bool {
        self.view.standings.over()
    }

    /// The panel the match shows, if any: the pause menu, or the standings
    /// once the clock has run out.
    fn menu(&self) -> Option<Menu> {
        match (self.over(), self.paused) {
            (true, _) => Some(Menu::Standings(
                self.view
                    .standings
                    .teams()
                    .iter()
                    .map(|team| format!("team {} — {} rocks", team.team.0, team.rocks))
                    .collect(),
            )),
            (false, true) => Some(Menu::Paused),
            (false, false) => None,
        }
    }

    /// The wheel open on the selected ring, where its rock is on screen.
    fn wheel(&self, screen: &Screen) -> Option<Wheel> {
        let place = self.selection?;
        let centre = screen.point_of(self.rock_pos(place.rock)?)?;
        Some(Wheel::open(
            place,
            PLAYER,
            self.session.state().roster(),
            centre,
        ))
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
            .filter(|seen| seen.seat == PLAYER)
            .find_map(|seen| seen.home)
        else {
            return;
        };
        if let Some(pos) = self.rock_pos(place.rock) {
            self.camera.set_focus(pos);
            self.followed = true;
        }
    }

    /// Where `rock` is this tick, in meters.
    fn rock_pos(&self, rock: probe_sim::RockId) -> Option<Vec3> {
        self.view
            .terrain
            .iter()
            .find(|terrain| terrain.rock == rock)
            .map(|terrain| terrain.orbit.at(self.view.tick, self.view.gravity).pos)
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

    /// Issues `command` as the player's, for the next tick to apply.
    fn issue(&mut self, command: Command) {
        self.pending.push(Issued {
            seat: PLAYER,
            command,
        });
    }

    /// Reads one frame of input: the pause key, the camera, and the
    /// pointer's gestures over `aimed`, the wheel as the player saw it.
    fn read_input(&mut self, ctx: &mut FrameCtx<'_, Self>, seen: &Screen, aimed: Option<&Wheel>) {
        if ctx.pressed(Button::Pause) {
            self.paused = !self.paused;
        }
        let pointer = ctx.pointer();
        let moved = pointer - self.pointer;
        self.pointer = pointer;
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
    fn point(
        &mut self,
        ctx: &mut FrameCtx<'_, Self>,
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
    /// Whether the next repeat is due: the first a third of a second after
    /// the press, and one every [`REPEAT_INTERVAL`] after that.
    fn due(&self) -> bool {
        self.held >= REPEAT_DELAY + REPEAT_INTERVAL * (self.edits - 1) as f32
    }
}

/// The one panel DISPLAY.md allows: the pause menu, and the standings once
/// the clock has run out.
enum Menu {
    Paused,
    /// One line per team, in team order.
    Standings(Vec<String>),
}

impl Menu {
    /// Shows the panel and answers whether the player resumed.
    fn show(&self, ui: &mut egui::Ui) -> bool {
        let mut resumed = false;
        egui::Window::new("Probe Game")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| match self {
                Menu::Paused => resumed = ui.button("Resume").clicked(),
                Menu::Standings(lines) => {
                    for line in lines {
                        ui.label(line);
                    }
                }
            });
        resumed
    }
}

/// The middle of the belt's rocks, in meters: where the camera looks until
/// the player's first placement.
///
/// A belt with no rock has no middle, and the origin is where the central
/// mass is; only a map with nothing on it reaches that.
fn belt_centre(view: &View) -> Vec3 {
    let rocks = view.terrain.len().max(1) as f64;
    view.terrain
        .iter()
        .map(|terrain| terrain.orbit.at(view.tick, view.gravity).pos)
        .fold(Vec3::ZERO, |sum, pos| sum + pos)
        * (1.0 / rocks)
}

/// The playable driven headlessly: input through the engine's offscreen
/// `Session`, and the screenshots the display is judged on.
///
/// Behind the `look` feature, which turns the engine's offscreen target on:
/// `xvfb-run -a cargo test -p probe-game --features look --bin probe-game`.
#[cfg(all(test, feature = "look"))]
mod tests {
    use std::path::PathBuf;

    use mirage_engine::headless::Session as Offscreen;
    use probe_game::scene::Fill;
    use probe_sim::RockId;
    use probe_sim::roster::SHIPYARD;

    use super::*;

    /// The offscreen target's size, in physical pixels.
    const TARGET: UVec2 = UVec2::new(1280, 720);

    /// The rock the drive plays on: near the belt's centre, where the
    /// camera opens.
    const ROCK: RockId = RockId(10);

    /// A headless run of the playable.
    fn play() -> Offscreen<Play> {
        Offscreen::new(
            Config::new("probe-play").with_tick_interval(probe_sim::TICK),
            TARGET,
            |_| Ok(Play::new()),
        )
        .expect("the offscreen session starts")
    }

    /// The frame's projection, as the game builds it: the offscreen layer
    /// paints one point per pixel.
    fn screen(session: &Offscreen<Play>) -> Screen {
        Screen::of(&session.game().camera, TARGET, 1.0)
    }

    /// Where `ROCK` draws, in points.
    fn rock_at(session: &Offscreen<Play>) -> egui::Pos2 {
        let play = session.game();
        screen(session)
            .point_of(play.rock_pos(ROCK).expect("the rock is on the map"))
            .expect("the rock is in front of the eye")
    }

    /// The point of `wheel` that names `row`'s plus band, found through the
    /// wheel's own hit test. The selected rock is the focus, so the wheel is
    /// centred on the target's own centre.
    fn plus_band(wheel: &Wheel, row: RowId) -> egui::Pos2 {
        let centre = egui::pos2(TARGET.x as f32 / 2.0, TARGET.y as f32 / 2.0);
        (0..3_600)
            .map(|step| {
                let angle = core::f32::consts::TAU * step as f32 / 3_600.0;
                let (sin, cos) = angle.sin_cos();
                let out = probe_game::wheel::RADIUS + probe_game::wheel::BAND_WIDTH * 0.5;
                egui::pos2(centre.x + out * sin, centre.y - out * cos)
            })
            .find(|at| wheel.slot_at(*at) == Some((row, WheelBand::Plus)))
            .expect("the row has a slot")
    }

    /// One click of the left button where the pointer stands.
    fn click(session: &mut Offscreen<Play>) {
        session.press(MouseButton::Left);
        session.step();
        session.release(MouseButton::Left);
        session.step();
    }

    /// `ticks` sim ticks.
    fn advance(session: &mut Offscreen<Play>, ticks: u32) {
        for _ in 0..ticks {
            session.tick();
        }
    }

    /// Writes the target's pixels to `game/look/<name>.png`.
    fn save(session: &Offscreen<Play>, name: &str) -> PathBuf {
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("look");
        std::fs::create_dir_all(&out).expect("game/look is writable");
        let path = out.join(format!("{name}.png"));
        let pixels = session.pixels().expect("the target reads back");
        image::RgbaImage::from_raw(TARGET.x, TARGET.y, pixels)
            .expect("the pixels match the target's size")
            .save(&path)
            .expect("the PNG writes");
        path
    }

    #[test]
    fn a_click_on_a_ring_and_one_on_the_wheel_place_a_shipyard() {
        let mut session = play();
        session.step();
        save(&session, "play_start");
        assert!(
            session.game().view.seen.is_empty(),
            "a match starts with nothing on the map"
        );

        session.set_pointer(pixel(rock_at(&session)));
        click(&mut session);
        let place = session.game().selection.expect("the ring was selected");
        assert_eq!(place.rock, ROCK);

        let wheel = session
            .game()
            .wheel(&screen(&session))
            .expect("the wheel is open on the selection");
        session.set_pointer(pixel(plus_band(&wheel, SHIPYARD)));
        session.step();
        assert!(
            matches!(session.game().hover, Some(Hover::Wheel { row, .. }) if row == SHIPYARD),
            "the hovered band previews its own row"
        );
        click(&mut session);
        advance(&mut session, 2);

        let view = &session.game().view;
        let shipyard = view
            .seen
            .iter()
            .find(|seen| seen.row == SHIPYARD)
            .expect("the reserve placed the shipyard");
        assert_eq!(shipyard.seat, PLAYER);
        assert_eq!(shipyard.home, Some(place));

        let scene = Scene::from_view(
            view,
            session.game().session.state().roster(),
            Client {
                selection: session.game().selection,
                hover: session.game().hover.clone(),
                fights: &session.game().fights,
            },
        );
        let ring = scene
            .rings
            .iter()
            .find(|ring| ring.place == place)
            .expect("its ring draws");
        assert!(
            ring.runs.iter().any(|run| run
                .marks
                .iter()
                .any(|mark| mark.fill == Fill::Solid && !mark.dim)),
            "the shipyard's glyph is on the ring"
        );

        session.step();
        save(&session, "play_placed");
    }

    #[test]
    fn a_right_drag_moves_the_belt_under_the_pointer() {
        let mut session = play();
        session.step();
        let from = egui::pos2(400.0, 300.0);
        let dragged = egui::vec2(-120.0, 60.0);
        session.set_pointer(pixel(from));
        session.step();
        let was = rock_at(&session);

        session.press(MouseButton::Right);
        session.set_pointer(pixel(from + dragged));
        session.step();

        // The pan is scaled at the focus's depth, and the drag itself moves
        // the plane to a slightly different depth, so a drag across a wide
        // view lands a fraction of itself off.
        let now = rock_at(&session);
        assert!(
            (now - was - dragged).length() <= 0.06 * dragged.length(),
            "the rock moved from {was} to {now}, not by {dragged}"
        );
    }

    /// A painted point as the pointer's own physical pixel, which the
    /// offscreen layer draws one of per point.
    fn pixel(at: egui::Pos2) -> Vec2 {
        Vec2::new(at.x, at.y)
    }
}
