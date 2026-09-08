use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::{FrameCtx, Game};
use neumannarch_protocol::Lobby;
use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, Preview};
use neumannarch_sim::{AsteroidId, SeatId, Session, Vec3};

use crate::controls::{Button, Controls};
use crate::display::camera::BeltCamera;
use crate::display::ease::{self, Clock, Span};
use crate::display::fights::Fights;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{ButtonAt, Client, Scene, WheelGesture};
use crate::display::send::Sending;
use crate::display::stockpile_bar::StockpileBar;
use crate::display::viewport::Viewport;
use crate::display::wheels::{Aim, Ease, Motion, Wheels};
use crate::display::{belt, hud};
use crate::net::machine::Machine;
use crate::net::pace::Allowed;
use crate::net::transport::Transport;
use crate::screens::control;
use crate::screens::held::{self, Held};
use crate::screens::lobby::seat_names;
use crate::screens::order::Order;
use crate::screens::panel::Panel;
use crate::screens::panning::Panning;
use crate::screens::pause::{self, Pause};
use crate::screens::{Playable, results};

const REPEAT_DELAY: f32 = 1.0 / 3.0;

const REPEAT_INTERVAL: f32 = 0.1;

const SHIFT_STEP: u32 = 5;

struct Drag {
    from: AsteroidId,
    count: u32,
    adjusted: f32,
}

struct Repeat {
    at: ButtonAt,
    held: f32,
    edits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Leave,
}

enum Mode {
    Playing {
        gesture: Gesture,
        hover: Option<WheelGesture>,
    },
    Paused,
    Held(Held),
}

#[derive(Default)]
enum Gesture {
    #[default]
    Still,
    Sending(Drag),
    Editing(Repeat),
}

pub struct Play {
    lobby: Lobby,
    machine: Machine,
    names: Vec<String>,
    view: View,
    fights: Fights,
    camera: BeltCamera,
    selection: Option<AsteroidId>,
    hovered: Option<AsteroidId>,
    shifted: bool,
    doing: Mode,
    followed: bool,
    panning: Panning,
    motion: Motion,
    clock: Clock,
    order_alpha: f64,
}

impl Play {
    pub fn of(lobby: Lobby, machine: Machine) -> Play {
        let view = machine.view();
        let opening = Scene::from_view(
            &view,
            machine.session().state().roster(),
            Client {
                selection: None,
                asked: Vec::new(),
                gesture: None,
                fights: &Fights::default(),
            },
        );
        let camera = BeltCamera::framing(opening.belt_inner_radius, opening.belt_outer_radius);
        Play {
            lobby,
            names: seat_names(
                machine.seating(),
                machine.player(),
                machine.session().setup().seed(),
            ),
            machine,
            view,
            fights: Fights::default(),
            camera,
            selection: None,
            hovered: None,
            shifted: false,
            doing: Mode::played(),
            followed: false,
            panning: Panning::still(),
            motion: Motion::default(),
            clock: Clock::default(),
            order_alpha: 1.0,
        }
    }

    pub fn seat(&self) -> SeatId {
        self.machine.seat()
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn session(&self) -> &Session {
        self.machine.session()
    }

    pub fn selection(&self) -> Option<AsteroidId> {
        self.selection
    }

    pub fn gesture(&self) -> Option<&WheelGesture> {
        match &self.doing {
            Mode::Playing { hover, .. } => hover.as_ref(),
            Mode::Paused | Mode::Held(_) => None,
        }
    }

    fn previewed_button(&self, at: ButtonAt) -> WheelGesture {
        let edit = at.edit(self.view.want_of(at.posting));
        WheelGesture::Button(at, self.previewed(&[edit]))
    }

    fn previewed_send(&self, sending: Sending) -> WheelGesture {
        let edits = sending.commands(&self.view, self.roster());
        WheelGesture::Send(sending, self.previewed(&edits))
    }

    fn previewed(&self, wants: &[Command]) -> Preview {
        self.machine
            .session()
            .state()
            .preview(self.machine.seat(), wants)
            .unwrap_or_default()
    }

    fn refreshes_preview(&mut self) {
        let refreshed = match self.gesture() {
            Some(WheelGesture::Button(at, _)) => Some(self.previewed_button(*at)),
            Some(WheelGesture::Send(sending, _)) => Some(self.previewed_send(*sending)),
            None => None,
        };
        if let Mode::Playing { hover, .. } = &mut self.doing {
            *hover = refreshed;
        }
    }

    fn step(&self) -> u32 {
        match self.shifted {
            true => SHIFT_STEP,
            false => 1,
        }
    }

    pub fn paused(&self) -> bool {
        matches!(self.doing, Mode::Paused)
    }

    pub fn over(&self) -> bool {
        self.view.standings.over()
    }

    pub fn ends(&self) -> results::Results {
        results::Results::of(
            self.lobby.clone(),
            self.scene(),
            self.camera,
            self.machine.session().state(),
        )
    }

    pub fn camera(&self) -> &BeltCamera {
        &self.camera
    }

    pub fn asteroid_pos(&self, asteroid: AsteroidId) -> Option<Vec3> {
        self.view
            .terrain
            .iter()
            .find(|terrain| terrain.asteroid == asteroid)
            .map(|terrain| terrain.orbit.at(self.view.time, self.view.gravity).pos)
    }

    pub fn tick(&mut self, transport: &mut dyn Transport) {
        if (self.paused() && self.machine.alone()) || self.over() {
            return;
        }
        let was = self.view.time;
        let ticked = self.machine.tick(transport);
        if ticked.rewound {
            self.fights = Fights::default();
            self.forgets();
        }
        self.holds(ticked.pace);
        self.view = self.machine.view();
        self.refreshes_preview();
        if ticked.pace != Allowed::Advance {
            return;
        }
        self.fights.observe(&self.view);
        self.camera
            .advance(self.view.time.since(was).seconds(), self.view.gravity);
        self.follow_the_first_placement();
    }

    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Picked>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let window = ctx.window_size();
        let points_per_pixel = 1.0 / ctx.pixels_per_point();
        let dt = self.clock.frame(ctx.elapsed());
        self.motion.begin(dt);
        self.order_alpha = ease::toward(self.order_alpha, self.order_target(), dt, Span::Slow);

        let viewport = Viewport::of(&self.camera, window, points_per_pixel);
        self.shifted = ctx.down(Button::Shift);
        let aimed = self.easing_wheels(&viewport, viewport.point_at(ctx.pointer()));
        self.read_input(ctx, &viewport, &aimed);
        self.camera.settle(dt);

        let viewport = Viewport::of(&self.camera, window, points_per_pixel);
        let pointer = viewport.point_at(ctx.pointer());
        let scene = self.scene();
        let over = viewport.bounds();
        let bar = scene
            .stockpile_bar
            .map(|view| StockpileBar::across(over, view));
        let wheels = self.easing_wheels_over(&scene, &viewport, pointer);

        belt::draw(&scene, &viewport, ctx);
        let clicked = ctx.pressed(Button::Select);
        let phrase = self.phrase(&wheels, pointer);
        let order = self.order(over);
        let gesture = self.gesture().cloned();
        let doing = &self.doing;
        let mut left = None;
        let mut resumed = false;
        ctx.ui(|ui| {
            hud::paint(&scene, &viewport, ui.painter());
            wheels.paint(ui.painter(), gesture.as_ref());
            if let Some(bar) = &bar {
                bar.paint(ui.painter(), Some(pointer));
            }
            if let Some(order) = &order {
                order.paint(ui.painter());
            }
            let panel = Panel::new(ui.painter(), over, pointer, clicked);
            if let Some((beside, phrase)) = phrase {
                let mut controls = control::Controls::over(&panel);
                if let Some(bar) = &bar {
                    controls.avoid(bar.frame());
                }
                controls.note(beside, phrase);
                controls.finish();
            }
            match doing {
                Mode::Playing { .. } => {}
                Mode::Paused => match Pause.frame(&panel) {
                    Some(pause::Picked::Resume) => resumed = true,
                    Some(pause::Picked::Leave) => left = Some(Picked::Leave),
                    None => {}
                },
                Mode::Held(held) => {
                    if held.frame(&panel) == Some(held::Picked::Leave) {
                        left = Some(Picked::Leave);
                    }
                }
            }
        });
        if resumed {
            self.doing = Mode::played();
        }
        left
    }

    fn scene(&self) -> Scene {
        Scene::from_view(
            &self.view,
            self.roster(),
            Client {
                selection: self.selection,
                asked: self
                    .hovered
                    .into_iter()
                    .chain(self.motion.shrinking())
                    .collect(),
                gesture: self.gesture().cloned(),
                fights: &self.fights,
            },
        )
    }

    fn roster(&self) -> &Roster {
        self.machine.session().state().roster()
    }

    fn order_target(&self) -> f64 {
        match self.view.draft.ended() {
            None => 1.0,
            Some(_) => 0.0,
        }
    }

    pub fn order(&self, window: egui::Rect) -> Option<Order> {
        (self.order_alpha > 0.0).then(|| {
            Order::over(
                window,
                &self.view.draft,
                self.view.tick,
                self.roster(),
                &self.names,
                self.order_alpha as f32,
            )
        })
    }

    pub fn wheels(
        &self,
        viewport: &Viewport,
        pointer: Option<egui::Pos2>,
        ease: &mut impl Ease,
    ) -> Wheels {
        Wheels::over(
            &self.scene(),
            self.roster(),
            viewport,
            &self.aim(pointer),
            ease,
        )
    }

    fn easing_wheels(&mut self, viewport: &Viewport, pointer: egui::Pos2) -> Wheels {
        let scene = self.scene();
        self.easing_wheels_over(&scene, viewport, pointer)
    }

    fn easing_wheels_over(
        &mut self,
        scene: &Scene,
        viewport: &Viewport,
        pointer: egui::Pos2,
    ) -> Wheels {
        let mut motion = core::mem::take(&mut self.motion);
        let wheels = Wheels::over(
            scene,
            self.roster(),
            viewport,
            &self.aim(Some(pointer)),
            &mut motion,
        );
        self.motion = motion;
        wheels
    }

    fn aim(&self, pointer: Option<egui::Pos2>) -> Aim<'_> {
        Aim {
            viewer: self.machine.seat(),
            pointer,
            hovered: self.hovered,
            step: self.step(),
            view: &self.view,
            state: self.machine.session().state(),
        }
    }

    fn phrase(&self, wheels: &Wheels, pointer: egui::Pos2) -> Option<(egui::Rect, String)> {
        let (at, spoken) = wheels.spoken_at(pointer)?;
        let beside =
            egui::Rect::from_center_size(at, egui::Vec2::splat(2.0 * crate::display::glyph::HALF));
        Some((beside, spoken.phrase(self.roster())))
    }

    fn holds(&mut self, pace: Allowed) {
        if self.paused() {
            return;
        }
        match self.holding(pace) {
            Some(held) => self.doing = Mode::Held(held),
            None if matches!(self.doing, Mode::Held(_)) => self.doing = Mode::played(),
            None => {}
        }
    }

    fn forgets(&mut self) {
        if let Mode::Playing { hover, .. } = &mut self.doing {
            *hover = None;
        }
    }

    fn holding(&self, pace: Allowed) -> Option<Held> {
        match (self.machine.desynced(), pace) {
            (Some(tick), _) => Some(Held::Desynced(tick)),
            (None, Allowed::Held) => Some(Held::Waiting(self.machine.waiting())),
            (None, Allowed::Advance | Allowed::Slowed) => None,
        }
    }

    fn follow_the_first_placement(&mut self) {
        if self.followed {
            return;
        }
        let Some(home) = self
            .view
            .present
            .iter()
            .find(|present| present.seat == self.machine.seat())
            .map(|present| present.home)
        else {
            return;
        };
        if let Some(pos) = self.asteroid_pos(home) {
            self.camera.set_focus(pos);
            self.followed = true;
        }
    }

    fn issue(&mut self, command: Command) {
        if let Some(human) = self.machine.human() {
            human.want(command);
        }
    }

    fn read_input<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        viewport: &Viewport,
        wheels: &Wheels,
    ) {
        if ctx.pressed(Button::Pause)
            && let Some(doing) = self.doing.pausing()
        {
            self.doing = doing;
        }
        let Mode::Playing { gesture, .. } = &mut self.doing else {
            return;
        };
        let mut gesture = core::mem::take(gesture);
        let at = viewport.point_at(ctx.pointer());
        self.hovered = wheels.hovered().or_else(|| self.asteroid_at(viewport, at));
        let dt = ctx.dt().as_secs_f32();
        let sending = matches!(gesture, Gesture::Sending(_));
        let notches = self
            .panning
            .drag(ctx, &mut self.camera, viewport.window(), !sending);

        let button = wheels.button_at(at);
        let over = wheels.at(at).or_else(|| self.asteroid_at(viewport, at));
        if let Gesture::Sending(drag) = &mut gesture {
            drag.adjust(notches);
        }
        if ctx.pressed(Button::Select) {
            gesture = self.pressed(button, over);
        }
        if ctx.released(Button::Select) {
            gesture = self.released(gesture, over);
        }
        if let Gesture::Editing(holding) = &mut gesture
            && holding.repeats(button, dt)
        {
            let at = holding.at;
            self.edit(at);
        }

        let hover = match (&gesture, button) {
            (Gesture::Sending(drag), _) => over
                .filter(|to| *to != drag.from)
                .map(|to| Sending {
                    from: drag.from,
                    to,
                    count: drag.count,
                })
                .map(|sending| self.sending_hover(sending)),
            (_, Some(at)) => Some(self.button_hover(at)),
            (Gesture::Still | Gesture::Editing(_), None) => None,
        };
        self.doing = Mode::Playing { gesture, hover };
    }

    fn button_hover(&self, at: ButtonAt) -> WheelGesture {
        match self.gesture() {
            Some(held @ WheelGesture::Button(over, _)) if *over == at => held.clone(),
            _ => self.previewed_button(at),
        }
    }

    fn sending_hover(&self, sending: Sending) -> WheelGesture {
        match self.gesture() {
            Some(held @ WheelGesture::Send(over, _)) if *over == sending => held.clone(),
            _ => self.previewed_send(sending),
        }
    }

    fn pressed(&mut self, button: Option<ButtonAt>, over: Option<AsteroidId>) -> Gesture {
        match (button, over) {
            (Some(at), _) => {
                self.selection = Some(at.posting.asteroid());
                self.edit(at);
                Gesture::Editing(Repeat {
                    at,
                    held: 0.0,
                    edits: 1,
                })
            }
            (None, Some(from)) => match Drag::off(&self.view, from, self.roster()) {
                Some(drag) => Gesture::Sending(drag),
                None => {
                    self.focuses(from);
                    Gesture::Still
                }
            },
            (None, None) => {
                self.selection = None;
                Gesture::Still
            }
        }
    }

    fn released(&mut self, gesture: Gesture, over: Option<AsteroidId>) -> Gesture {
        if let Gesture::Sending(drag) = gesture {
            match over {
                Some(to) if to != drag.from => {
                    let sending = Sending {
                        from: drag.from,
                        to,
                        count: drag.count,
                    };
                    for command in sending.commands(&self.view, self.roster()) {
                        self.issue(command);
                    }
                }
                Some(asteroid) => self.focuses(asteroid),
                None => {}
            }
        }
        Gesture::Still
    }

    fn edit(&mut self, at: ButtonAt) {
        let want = self.view.want_of(at.posting);
        if at.button.wanted(want) != want {
            self.issue(at.edit(want));
        }
    }

    fn asteroid_at(&self, viewport: &Viewport, at: egui::Pos2) -> Option<AsteroidId> {
        self.view
            .terrain
            .iter()
            .filter_map(|terrain| {
                let centre = viewport.point_of(self.asteroid_pos(terrain.asteroid)?)?;
                let away = centre.distance(at);
                (away <= crate::display::wheel::PICK_RADIUS).then_some((away, terrain.asteroid))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, asteroid)| asteroid)
    }

    fn focuses(&mut self, asteroid: AsteroidId) {
        self.selection = Some(asteroid);
        if let Some(pos) = self.asteroid_pos(asteroid) {
            self.camera.set_focus(pos);
            self.followed = true;
        }
    }
}

impl Mode {
    fn played() -> Mode {
        Mode::Playing {
            gesture: Gesture::Still,
            hover: None,
        }
    }

    fn pausing(&self) -> Option<Mode> {
        match self {
            Mode::Playing { .. } => Some(Mode::Paused),
            Mode::Paused => Some(Mode::played()),
            Mode::Held(_) => None,
        }
    }
}

impl Drag {
    fn off(view: &View, from: AsteroidId, roster: &Roster) -> Option<Drag> {
        let count = Sending::present(view, from, roster);
        (count > 0).then_some(Drag {
            from,
            count,
            adjusted: 0.0,
        })
    }

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

impl Repeat {
    fn repeats(&mut self, under: Option<ButtonAt>, dt: f32) -> bool {
        if under != Some(self.at) {
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
    use neumannarch_protocol::{Lobby, PlayerId};

    use super::*;
    use crate::net::local::Local;

    fn dragging() -> Play {
        let lobby = Lobby::skirmish(PlayerId::HOST);
        let started = lobby.freeze().expect("a skirmish is a match");
        let crew = started
            .seating()
            .run_by(PlayerId::HOST)
            .expect("the host holds a seat");
        let mut play = Play::of(lobby, Machine::of(started, &crew, &mut Local));
        let from = AsteroidId(0);
        let sending = Sending {
            from,
            to: AsteroidId(1),
            count: 1,
        };
        let hover = Some(play.previewed_send(sending));
        play.doing = Mode::Playing {
            gesture: Gesture::Sending(Drag {
                from,
                count: 1,
                adjusted: 0.0,
            }),
            hover,
        };
        play
    }

    #[test]
    fn a_press_on_an_asteroid_holding_no_unit_of_the_seat_starts_no_drag() {
        let lobby = Lobby::skirmish(PlayerId::HOST);
        let started = lobby.freeze().expect("a skirmish is a match");
        let crew = started
            .seating()
            .run_by(PlayerId::HOST)
            .expect("the host holds a seat");
        let mut play = Play::of(lobby, Machine::of(started, &crew, &mut Local));
        let bare = AsteroidId(3);

        let gesture = play.pressed(None, Some(bare));

        assert!(
            matches!(gesture, Gesture::Still),
            "a bare asteroid has nothing to send"
        );
        assert_eq!(play.selection(), Some(bare), "the press selects it");
    }

    #[test]
    fn a_match_that_begins_to_hold_drops_the_gesture_and_the_preview_the_pointer_had() {
        let mut play = dragging();

        play.holds(Allowed::Held);

        assert!(matches!(play.doing, Mode::Held(Held::Waiting(_))));
        assert!(
            play.gesture().is_none(),
            "a held match previews nothing over the belt"
        );
    }

    #[test]
    fn the_pause_screen_does_not_open_over_a_held_match() {
        let held = Mode::Held(Held::Waiting(Vec::new()));

        assert!(
            held.pausing().is_none(),
            "a held match offers only its own control"
        );
        assert!(matches!(Mode::played().pausing(), Some(Mode::Paused)));
        assert!(matches!(Mode::Paused.pausing(), Some(Mode::Playing { .. })));
    }

    #[test]
    fn a_hold_that_passes_plays_the_match_again_with_the_pointer_over_nothing() {
        let mut play = dragging();
        play.holds(Allowed::Held);

        play.holds(Allowed::Advance);

        assert!(matches!(
            play.doing,
            Mode::Playing {
                gesture: Gesture::Still,
                ..
            }
        ));
    }
}
