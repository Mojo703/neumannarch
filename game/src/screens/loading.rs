//! Loading: the lobby's belt, still, until every machine has built the
//! match and agreed its first hash.

use mirage_engine::egui::{Align2, Pos2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::{Lobby, PlayerId};
use probe_sim::Setup;
use probe_sim::Tick;
use probe_sim::belt::Belt;

use crate::display::camera::BeltCamera;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::Scene;
use crate::display::screen::Screen;
use crate::display::{belt, hud};
use crate::net::machine::{Machine, Unplayable};
use crate::net::transport::Transport;
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};
use crate::screens::play::Play;
use crate::screens::{Playable, lobby};

/// The lobby's match built and held still while every machine builds the
/// same initial state and agrees its hash.
///
/// A skirmish has no peer to agree with, so it hands over on its first
/// frame.
pub struct Loading {
    lobby: Lobby,
    /// The match, until the frame that hands it to the play screen.
    machine: Option<Machine>,
    scene: Scene,
    camera: BeltCamera,
}

impl Loading {
    /// Paints the still belt and, once every machine agrees, hands the
    /// match over.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Step>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let size = ctx.window_size();
        let screen = Screen::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &screen, ctx);

        let window = panel::window_of(size, points_per_pixel);
        let scene = &self.scene;
        ctx.ui(|ui| {
            hud::paint(scene, &screen, None, ui.painter());
            let panel = Panel::new(ui.painter(), window, Pos2::ZERO, false);
            panel.text(
                "Building the match",
                Pos2::new(window.center().x, window.bottom() - panel::MARGIN),
                Align2::CENTER_CENTER,
                panel::INK,
                panel::BODY_SIZE,
            );
        });

        let agreed = self.machine.as_ref().is_some_and(Machine::agreed);
        let machine = agreed.then(|| self.machine.take()).flatten()?;
        Some(Step::Play(Box::new(Play::of(self.lobby.clone(), machine))))
    }

    /// The match `setup` names, over the belt `lobby`'s seed lays, as the
    /// person holding `me`'s slot plays it.
    pub fn of(
        lobby: Lobby,
        setup: Setup,
        me: PlayerId,
        transport: &mut dyn Transport,
    ) -> Result<Loading, Unplayable> {
        let machine = Machine::of(&lobby, setup, me, transport)?;
        let scene = Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Tick::ZERO);
        let camera = BeltCamera::new(scene.centre(), lobby::PREVIEW_ZOOM);
        Ok(Loading {
            lobby,
            machine: Some(machine),
            scene,
            camera,
        })
    }

    /// Takes in what the other machines say while the match is built.
    pub fn tick(&mut self, transport: &mut dyn Transport) {
        if let Some(machine) = &mut self.machine {
            machine.listen(transport);
        }
    }
}
