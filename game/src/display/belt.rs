use mirage_engine::prelude::*;

use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{EntityView, RockView, Scene};
use crate::display::tint;
use crate::display::viewport::Viewport;

const ROCK_SUBDIVISIONS: u32 = 2;

const ROCK_COLOUR: Color = Color::rgb(0.55, 0.5, 0.45);

const TINT_STRENGTH: f32 = 0.7;

const SUN: Vec3 = Vec3::new(-0.6, -0.7, -0.4);

const SUN_COLOUR: Color = Color::rgb(1.0, 0.97, 0.9);

pub fn draw<G: Game>(scene: &Scene, viewport: &Viewport, ctx: &mut FrameCtx<'_, G>)
where
    G::Meshes: Holds<Sphere> + Holds<GlyphQuad>,
{
    ctx.set_camera(viewport.camera());
    ctx.light(Light::directional(SUN, SUN_COLOUR));

    for rock in &scene.rocks {
        ctx.draw(rock_instance::<G>(rock, viewport));
    }

    for entity in &scene.entities {
        let Some(meters_per_point) = viewport.meters_per_point(entity.pos) else {
            continue;
        };
        ctx.draw(ship_instance::<G>(entity, meters_per_point, viewport));
    }
}

fn rock_instance<G: Game>(rock: &RockView, viewport: &Viewport) -> Instance<Sphere, G::Styles> {
    let diameter = (2.0 * rock.radius) as f32;
    Sphere {
        subdivisions: ROCK_SUBDIVISIONS,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(diameter),
        Quat::IDENTITY,
        viewport.local(rock.pos),
    ))
    .material(Material::lit(tint::toward(
        ROCK_COLOUR,
        rock.caps,
        TINT_STRENGTH,
    )))
}

fn ship_instance<G: Game>(
    entity: &EntityView,
    meters_per_point: f32,
    viewport: &Viewport,
) -> Instance<GlyphQuad, G::Styles> {
    let side = 2.0 * crate::display::glyph::HALF * meters_per_point;
    GlyphQuad {
        glyph: entity.glyph.clone(),
        seat: entity.seat,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(side),
        Quat::IDENTITY,
        viewport.local(entity.pos),
    ))
    .billboard()
}
