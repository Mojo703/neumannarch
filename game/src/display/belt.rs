//! The belt: rocks and ships, drawn in 3D through the engine.

use mirage_engine::prelude::*;

use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{EntityView, RockView, Scene};
use crate::display::screen::Screen;

/// A rock mesh's density; see [`Sphere::subdivisions`].
const ROCK_SUBDIVISIONS: u32 = 2;

/// A rock's colour: plain, unlit by ownership.
const ROCK_COLOUR: Color = Color::rgb(0.55, 0.5, 0.45);

/// Where the light comes from, so a rock reads as a sphere: down the belt
/// plane's `+X`, tilted above it.
const SUN: Vec3 = Vec3::new(-0.6, -0.7, -0.4);

/// The light's colour and strength.
const SUN_COLOUR: Color = Color::rgb(1.0, 0.97, 0.9);

/// Draws `scene`'s rocks and ships as `screen` projects them.
pub fn draw<G: Game>(scene: &Scene, screen: &Screen, ctx: &mut FrameCtx<'_, G>)
where
    G::Meshes: Holds<Sphere> + Holds<GlyphQuad>,
{
    ctx.set_camera(screen.camera());
    ctx.light(Light::directional(SUN, SUN_COLOUR));

    for rock in &scene.rocks {
        ctx.draw(rock_instance::<G>(rock, screen));
    }

    for entity in &scene.entities {
        let Some(meters_per_point) = screen.meters_per_point(entity.pos) else {
            continue;
        };
        ctx.draw(ship_instance::<G>(entity, meters_per_point, screen));
    }
}

fn rock_instance<G: Game>(rock: &RockView, screen: &Screen) -> Instance<Sphere, G::Styles> {
    let diameter = (2.0 * rock.radius) as f32;
    Sphere {
        subdivisions: ROCK_SUBDIVISIONS,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(diameter),
        Quat::IDENTITY,
        screen.local(rock.pos),
    ))
    .material(Material::lit(ROCK_COLOUR))
}

/// `entity`'s glyph at a fixed screen size: [`crate::display::glyph::HALF`] points
/// each way, in meters at its own depth, so the belt's ships and the HUD's
/// runs agree in size.
fn ship_instance<G: Game>(
    entity: &EntityView,
    meters_per_point: f32,
    screen: &Screen,
) -> Instance<GlyphQuad, G::Styles> {
    let side = 2.0 * crate::display::glyph::HALF * meters_per_point;
    GlyphQuad {
        glyph: entity.glyph.clone(),
        seat: entity.seat,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(side),
        Quat::IDENTITY,
        screen.local(entity.pos),
    ))
    .billboard()
}
