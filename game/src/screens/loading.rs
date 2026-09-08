use mirage_engine::egui::{Align2, Pos2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use neumannarch_protocol::{Crew, Lobby, Started};
use neumannarch_sim::Time;
use neumannarch_sim::belt::Belt;

use crate::display::bars::Bars;
use crate::display::camera::BeltCamera;
use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::Scene;
use crate::display::viewport::Viewport;
use crate::display::{belt, hud};
use crate::net::machine::Machine;
use crate::net::transport::Transport;
use crate::screens::Playable;
use crate::screens::panel::{self, Panel};
use crate::screens::play::Play;

pub(crate) const BUILDING: &str = "Building the match";

pub struct Loading {
    lobby: Lobby,
    machine: Machine,
    scene: Scene,
    camera: BeltCamera,
}

impl Loading {
    pub fn agreed(&self) -> bool {
        self.machine.agreed()
    }

    pub fn plays(self) -> Play {
        Play::of(self.lobby, self.machine)
    }

    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let points_per_pixel = 1.0 / ctx.pixels_per_point();
        let size = ctx.window_size();
        let viewport = Viewport::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &viewport, ctx);

        let window = panel::window_of(size, points_per_pixel);
        let scene = &self.scene;
        ctx.ui(|ui| {
            hud::paint(scene, &viewport, ui.painter());
            Bars::at_rest(scene, &viewport).paint(ui.painter());
            let panel = Panel::new(ui.painter(), window, Pos2::ZERO, false);
            panel.text(
                BUILDING,
                Pos2::new(window.center().x, window.bottom() - panel::MARGIN),
                Align2::CENTER_CENTER,
                panel::INK,
                panel::BODY_SIZE,
            );
        });
    }

    pub fn of(
        lobby: Lobby,
        started: Started,
        crew: &Crew,
        transport: &mut dyn Transport,
    ) -> Loading {
        let machine = Machine::of(started, crew, transport);
        let scene = Scene::of_belt(&Belt::from_seed(lobby.seed()), Belt::GRAVITY, Time::ZERO);
        let camera = BeltCamera::framing(scene.belt_inner_radius, scene.belt_outer_radius);
        Loading {
            lobby,
            machine,
            scene,
            camera,
        }
    }

    pub fn tick(&mut self, transport: &mut dyn Transport) {
        self.machine.receive(transport);
    }
}
