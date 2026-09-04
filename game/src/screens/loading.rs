//! Loading: the lobby's belt, still, until every machine has built the
//! match and agreed its first hash.

use mirage_engine::egui::{Align2, Pos2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::{Crew, Lobby, Started};
use probe_sim::Tick;
use probe_sim::belt::Belt;

use crate::display::camera::BeltCamera;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::Scene;
use crate::display::screen::Screen;
use crate::display::{belt, hud};
use crate::net::machine::Machine;
use crate::net::transport::Transport;
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
    machine: Machine,
    scene: Scene,
    camera: BeltCamera,
}

impl Loading {
    /// Whether every machine has built the match and agreed its first hash,
    /// which is when the match is played.
    pub fn agreed(&self) -> bool {
        self.machine.agreed()
    }

    /// The match it built, which the play screen takes once every machine
    /// has agreed it.
    pub fn plays(self) -> Play {
        Play::of(self.lobby, self.machine)
    }

    /// Paints the still belt.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
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
    }

    /// The match `started` names, over the belt `lobby`'s seed lays, as the
    /// machine `crew` is the seats of plays it.
    pub fn of(
        lobby: Lobby,
        started: Started,
        crew: &Crew,
        transport: &mut dyn Transport,
    ) -> Loading {
        let machine = Machine::of(started, crew, transport);
        let scene = Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Tick::ZERO);
        let camera = BeltCamera::new(scene.centre(), lobby::PREVIEW_ZOOM);
        Loading {
            lobby,
            machine,
            scene,
            camera,
        }
    }

    /// Takes in what the other machines say while the match is built.
    pub fn tick(&mut self, transport: &mut dyn Transport) {
        self.machine.listen(transport);
    }
}
