use mirage_engine::prelude::*;

use crate::display::glyph_quad::GlyphQuad;
use crate::display::scene::{AsteroidView, EntityView, Scene};
use crate::display::tint;
use crate::display::viewport::Viewport;
use crate::display::zoom;

const ASTEROID_SUBDIVISIONS: u32 = 2;

const ASTEROID_COLOUR: Color = Color::rgb(0.55, 0.5, 0.45);

const TINT_STRENGTH: f32 = 0.7;

const STAR_SUBDIVISIONS: u32 = 3;

const STAR_COLOUR: Color = Color::rgb(1.0, 0.97, 0.9);

pub const ASTEROID_FLOOR_POINTS: f32 = 6.0;

pub const ENTITY_SIDE_METERS: f32 = 1.0;

pub const ENTITY_FLOOR_POINTS: f32 = 2.0 * crate::display::glyph::HALF;

pub fn draw<G: Game>(scene: &Scene, viewport: &Viewport, ctx: &mut FrameCtx<'_, G>)
where
    G::Meshes: Holds<Sphere> + Holds<GlyphQuad>,
{
    ctx.set_camera(viewport.camera());
    ctx.light(Light::point(
        viewport.local(neumannarch_sim::Vec3::ZERO),
        STAR_COLOUR,
        scene.star_light_range as f32,
    ));
    ctx.draw(star_instance::<G>(scene.star_radius, viewport));

    for asteroid in &scene.asteroids {
        let Some(meters_per_point) = viewport.meters_per_point(asteroid.pos) else {
            continue;
        };
        ctx.draw(asteroid_instance::<G>(asteroid, meters_per_point, viewport));
    }

    for entity in &scene.entities {
        let Some(meters_per_point) = viewport.meters_per_point(entity.pos) else {
            continue;
        };
        ctx.draw(entity_instance::<G>(entity, meters_per_point, viewport));
    }
}

fn star_instance<G: Game>(star_radius: f64, viewport: &Viewport) -> Instance<Sphere, G::Styles> {
    let diameter = (2.0 * star_radius) as f32;
    Sphere {
        subdivisions: STAR_SUBDIVISIONS,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(diameter),
        Quat::IDENTITY,
        viewport.local(neumannarch_sim::Vec3::ZERO),
    ))
    .material(Material::color(STAR_COLOUR).emissive(STAR_COLOUR))
}

fn asteroid_instance<G: Game>(
    asteroid: &AsteroidView,
    meters_per_point: f32,
    viewport: &Viewport,
) -> Instance<Sphere, G::Styles> {
    let points = (2.0 * asteroid.radius) as f32 / meters_per_point;
    let diameter = zoom::floored(points, ASTEROID_FLOOR_POINTS) * meters_per_point;
    Sphere {
        subdivisions: ASTEROID_SUBDIVISIONS,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(diameter),
        Quat::IDENTITY,
        viewport.local(asteroid.pos),
    ))
    .material(Material::lit(tint::toward(
        ASTEROID_COLOUR,
        asteroid.caps,
        TINT_STRENGTH,
    )))
}

fn entity_instance<G: Game>(
    entity: &EntityView,
    meters_per_point: f32,
    viewport: &Viewport,
) -> Instance<GlyphQuad, G::Styles> {
    let points = ENTITY_SIDE_METERS / meters_per_point;
    let side = zoom::floored(points, ENTITY_FLOOR_POINTS) * meters_per_point;
    GlyphQuad {
        pattern: entity.pattern,
        seat: entity.seat,
    }
    .at(Transform::from_scale_rotation_translation(
        Vec3::splat(side),
        Quat::IDENTITY,
        viewport.local(entity.pos),
    ))
    .material(Material::lit(Color::WHITE).cutout())
    .billboard()
}
