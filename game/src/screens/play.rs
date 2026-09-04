use std::collections::BTreeMap;

use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::{FrameCtx, Game};
use neumannarch_protocol::Lobby;
use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;
use neumannarch_sim::{RockId, RowId, SeatId, Session, Vec3};

use crate::controls::{Button, Controls};
use crate::display::camera::BeltCamera;
use crate::display::ease::Clock;
use crate::display::fights::Fights;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::label::titled;
use crate::display::scene::{Client, Hover, Scene, WheelBand};
use crate::display::send::Sending;
use crate::display::viewport::Viewport;
use crate::display::wheels::{Aim, Ease, Motion, Wheels};
use crate::display::{belt, hud};
use crate::net::machine::Machine;
use crate::net::pace::Allowed;
use crate::net::transport::Transport;
use crate::screens::control;
use crate::screens::held::{self, Held};
use crate::screens::panel::{self, Panel};
use crate::screens::panning::Panning;
use crate::screens::pause::{self, Pause};
use crate::screens::{Playable, results};

const OPENING_ZOOM: f64 = 6_000.0;

const REPEAT_DELAY: f32 = 1.0 / 3.0;

const REPEAT_INTERVAL: f32 = 0.1;

const SHIFT_STEP: u32 = 5;

struct Drag {
    from: RockId,
    count: u32,
    adjusted: f32,
}

struct Repeat {
    rock: RockId,
    row: RowId,
    band: WheelBand,
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
        hover: Option<Hover>,
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
    view: View,
    fights: Fights,
    camera: BeltCamera,
    selection: Option<RockId>,
    hovered: Option<RockId>,
    doing: Mode,
    followed: bool,
    panning: Panning,
    motion: Motion,
    clock: Clock,
}

impl Play {
    pub fn of(lobby: Lobby, machine: Machine) -> Play {
        let view = machine.view();
        let camera = BeltCamera::new(
            Scene::from_view(
                &view,
                machine.session().state().roster(),
                Client {
                    selection: None,
                    pointed: None,
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
            hovered: None,
            doing: Mode::played(),
            followed: false,
            panning: Panning::still(),
            motion: Motion::default(),
            clock: Clock::default(),
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

    pub fn selection(&self) -> Option<RockId> {
        self.selection
    }

    pub fn hover(&self) -> Option<&Hover> {
        match &self.doing {
            Mode::Playing { hover, .. } => hover.as_ref(),
            Mode::Paused | Mode::Held(_) => None,
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

    pub fn rock_pos(&self, rock: RockId) -> Option<Vec3> {
        self.view
            .terrain
            .iter()
            .find(|terrain| terrain.rock == rock)
            .map(|terrain| terrain.orbit.at(self.view.tick, self.view.gravity).pos)
    }

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
            .advance(neumannarch_sim::TICK.as_secs_f64(), self.view.gravity);
        self.follow_the_first_placement();
    }

    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Picked>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let window = ctx.window_size();

        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let dt = self.clock.frame(ctx.elapsed());
        let mut motion = core::mem::take(&mut self.motion);
        motion.begin(dt);

        let viewport = Viewport::of(&self.camera, window, points_per_pixel);
        let shifted = ctx.down(Button::Shift);
        let aimed = self.wheels(
            &viewport,
            Some(viewport.point_at(ctx.pointer())),
            shifted,
            &mut motion,
        );
        self.read_input(ctx, &viewport, &aimed);
        self.camera.settle(dt);

        let viewport = Viewport::of(&self.camera, window, points_per_pixel);
        let pointer = viewport.point_at(ctx.pointer());
        let scene = self.scene();
        let wheels = Wheels::over(
            &scene,
            self.roster(),
            &viewport,
            &self.aim(Some(pointer), shifted),
            &mut motion,
        );
        self.motion = motion;

        belt::draw(&scene, &viewport, ctx);
        let clicked = ctx.pressed(Button::Select);
        let over = panel::window_of(window, points_per_pixel);
        let phrase = self.phrase(&wheels, pointer);
        let hover = self.hover().copied();
        let doing = &self.doing;
        let mut left = None;
        let mut resumed = false;
        ctx.ui(|ui| {
            hud::paint(&scene, &viewport, ui.painter());
            wheels.paint(ui.painter(), hover.as_ref());
            let panel = Panel::new(ui.painter(), over, pointer, clicked);
            if let Some((beside, phrase)) = phrase {
                let mut controls = control::Controls::over(&panel);
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
                pointed: self.hovered,
                hover: self.hover().copied(),
                fights: &self.fights,
            },
        )
    }

    fn roster(&self) -> &neumannarch_sim::roster::Roster {
        self.machine.session().state().roster()
    }

    pub fn wheels(
        &self,
        viewport: &Viewport,
        pointer: Option<egui::Pos2>,
        shifted: bool,
        ease: &mut impl Ease,
    ) -> Wheels {
        Wheels::over(
            &self.scene(),
            self.roster(),
            viewport,
            &self.aim(pointer, shifted),
            ease,
        )
    }

    fn aim(&self, pointer: Option<egui::Pos2>, shifted: bool) -> Aim {
        Aim {
            viewer: self.machine.seat(),
            pointer,
            hovered: self.hovered,
            step: match shifted {
                true => SHIFT_STEP,
                false => 1,
            },
            wants: self.wants(),
        }
    }

    fn wants(&self) -> BTreeMap<(RockId, RowId), u32> {
        self.view
            .plans
            .iter()
            .map(|plan| ((plan.rock, plan.row), plan.want))
            .collect()
    }

    fn phrase(&self, wheels: &Wheels, pointer: egui::Pos2) -> Option<(egui::Rect, String)> {
        let (at, row, shown) = wheels.spoken_at(pointer)?;
        let beside =
            egui::Rect::from_center_size(at, egui::Vec2::splat(2.0 * crate::display::glyph::HALF));
        let name = titled(self.roster()[row].name);
        let phrase = match shown {
            Some(shown) => shown.entry.phrase(&name),
            None => name,
        };
        Some((beside, phrase))
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
        if let Some(pos) = self.rock_pos(home) {
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
        self.hovered = wheels.hovered().or_else(|| self.rock_at(viewport, at));
        let dt = ctx.dt().as_secs_f32();
        let sending = matches!(gesture, Gesture::Sending(_));
        let notches = self
            .panning
            .drag(ctx, &mut self.camera, viewport.window(), !sending);

        let band = wheels.band_at(at);
        let over = wheels.at(at).or_else(|| self.rock_at(viewport, at));
        if let Gesture::Sending(drag) = &mut gesture {
            drag.adjust(notches);
        }
        if ctx.pressed(Button::Select) {
            gesture = self.pressed(band, over);
        }
        if ctx.released(Button::Select) {
            gesture = self.released(gesture, over);
        }
        if let Gesture::Editing(holding) = &mut gesture
            && holding.repeats(band, dt)
        {
            let (rock, row, band) = (holding.rock, holding.row, holding.band);
            self.edit(rock, row, band);
        }

        let hover = match (&gesture, band) {
            (Gesture::Sending(drag), _) => over.filter(|to| *to != drag.from).map(|to| {
                Hover::Send(Sending {
                    from: drag.from,
                    to,
                    count: drag.count,
                })
            }),
            (_, Some((rock, row, band))) => Some(Hover::Wheel { rock, row, band }),
            (Gesture::Still | Gesture::Editing(_), None) => None,
        };
        self.doing = Mode::Playing { gesture, hover };
    }

    fn pressed(
        &mut self,
        band: Option<(RockId, RowId, WheelBand)>,
        over: Option<RockId>,
    ) -> Gesture {
        match (band, over) {
            (Some((rock, row, band)), _) => {
                self.selection = Some(rock);
                self.edit(rock, row, band);
                Gesture::Editing(Repeat {
                    rock,
                    row,
                    band,
                    held: 0.0,
                    edits: 1,
                })
            }
            (None, Some(from)) => Gesture::Sending(Drag {
                from,
                count: Sending::present(&self.view, from, self.roster()),
                adjusted: 0.0,
            }),
            (None, None) => {
                self.selection = None;
                Gesture::Still
            }
        }
    }

    fn released(&mut self, gesture: Gesture, over: Option<RockId>) -> Gesture {
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
                Some(rock) => self.focuses(rock),
                None => {}
            }
        }
        Gesture::Still
    }

    fn edit(&mut self, rock: RockId, row: RowId, band: WheelBand) {
        let want = self.view.plan_of(rock, row).map_or(0, |plan| plan.want);
        if band.wanted(want) != want {
            self.issue(band.edit(rock, row, want));
        }
    }

    fn rock_at(&self, viewport: &Viewport, at: egui::Pos2) -> Option<RockId> {
        self.view
            .terrain
            .iter()
            .filter_map(|terrain| {
                let centre = viewport.point_of(self.rock_pos(terrain.rock)?)?;
                let away = centre.distance(at);
                (away <= crate::display::wheel::PICK_RADIUS).then_some((away, terrain.rock))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, rock)| rock)
    }

    fn focuses(&mut self, rock: RockId) {
        self.selection = Some(rock);
        if let Some(pos) = self.rock_pos(rock) {
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
    fn repeats(&mut self, slot: Option<(RockId, RowId, WheelBand)>, dt: f32) -> bool {
        if slot != Some((self.rock, self.row, self.band)) {
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
        let from = RockId(0);
        play.doing = Mode::Playing {
            gesture: Gesture::Sending(Drag {
                from,
                count: 1,
                adjusted: 0.0,
            }),
            hover: Some(Hover::Send(Sending {
                from,
                to: RockId(1),
                count: 1,
            })),
        };
        play
    }

    #[test]
    fn a_match_that_begins_to_hold_drops_the_gesture_and_the_preview_the_pointer_had() {
        let mut play = dragging();

        play.holds(Allowed::Held);

        assert!(matches!(play.doing, Mode::Held(Held::Waiting(_))));
        assert!(
            play.hover().is_none(),
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
