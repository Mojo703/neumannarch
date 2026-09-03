//! The screens outside a match and the match itself, and the one value
//! that owns which of them is live.

use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::Game;

use crate::controls::Controls;
use crate::display::glyph_quad::GlyphQuad;

/// What a screen needs of the game it runs in: the playable's own input
/// vocabulary, and the two meshes the belt draws with.
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
pub mod panel;
pub mod panning;
pub mod pause;
pub mod play;
pub mod results;
pub mod title;
