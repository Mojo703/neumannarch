use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::Game;

use crate::controls::Controls;
use crate::display::glyph_quad::GlyphQuad;

pub trait Playable: Game<Actions = Controls>
where
    Self::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
{
}

impl<G> Playable for G
where
    G: Game<Actions = Controls>,
    G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
{
}

pub mod control;
pub mod field;
pub mod flow;
pub mod held;
pub mod loading;
pub mod lobby;
pub mod order;
pub mod panel;
pub mod panning;
pub mod pause;
pub mod play;
pub mod results;
pub mod title;
