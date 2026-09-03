//! Loading: the lobby's belt, still, until the match is built and agreed.

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
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};
use crate::screens::play::Play;
use crate::screens::{Playable, lobby};

/// The lobby's match being built: the belt it will be played on, held still
/// while every machine builds the same initial state and agrees its hash.
///
/// A skirmish has no peer to agree with, so it holds for one frame; the
/// socket unit is what makes it wait.
pub struct Loading {
    lobby: Lobby,
    setup: Setup,
    me: PlayerId,
    scene: Scene,
    camera: BeltCamera,
    /// Whether this machine has built the match and reported tick zero.
    agreed: bool,
}

impl Loading {
    /// The match `setup` names, over the belt `lobby`'s seed lays, with
    /// `me` the person at this machine.
    pub fn of(lobby: Lobby, setup: Setup, me: PlayerId) -> Loading {
        let scene = Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Tick::ZERO);
        let camera = BeltCamera::new(scene.centre(), lobby::PREVIEW_ZOOM);
        Loading {
            lobby,
            setup,
            me,
            scene,
            camera,
            agreed: false,
        }
    }

    /// Builds the match, once. The next frame hands it over.
    pub fn tick(&mut self) {
        self.agreed = true;
    }

    /// Paints the still belt and, once the match is agreed, hands it over.
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

        self.agreed.then(|| {
            Step::Play(Box::new(Play::of(
                self.lobby.clone(),
                self.setup.clone(),
                self.me,
            )))
        })
    }
}
