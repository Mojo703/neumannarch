use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use neumannarch_sim::Vec3;

use crate::display::glyph;
use crate::display::glyph_quad::seat_color32;
use crate::display::hue;
use crate::display::scene::{Arc, AsteroidView, Scene};
use crate::display::viewport::Viewport;
use crate::display::wheel::{self, Side};
use crate::display::zoom;

const FLIGHT_WIDTH: f32 = 1.5;

const FLIGHT_COLOUR: Color32 = Color32::from_gray(200);

const FLIGHT_DASH_LENGTH: f32 = 6.0;

const FLIGHT_GAP_LENGTH: f32 = 5.0;

const FLIGHT_SPEED: f32 = 30.0;

const FLIGHT_FAINT_ALPHA: f32 = 0.15;

const FLIGHT_FULL_ALPHA: f32 = 0.9;

const CIRCLE_WIDTH: f32 = 1.0;

const CIRCLE_SEGMENTS: usize = 48;

const ZONE_COLOUR: Color32 = Color32::from_gray(150);

const ZONE_ALPHA: f32 = 0.3;

const RANGE_ALPHA: f32 = 0.22;

pub const RESTING_WHOLE_ZONE_POINTS: f32 = 2.0;

pub const RESTING_GONE_ZONE_POINTS: f32 = 0.25;

pub const FIGHT_WHOLE_ZONE_POINTS: f32 = 4.0;

pub const FIGHT_GONE_ZONE_POINTS: f32 = 0.2;

pub const YIELD_MARK_FLOOR_POINTS: f32 = 5.0;

pub const YIELD_MARK_THICKNESS_POINTS: f32 = 12.0;

const YIELD_MARK_THICKNESS_SHARE: f32 = 0.3;

const YIELD_MARK_SLIVER_POINTS: f32 = 2.0;

const YIELD_MARK_SEGMENTS: usize = 10;

const YIELD_MARK_FIRST_SECTOR_RADIANS: f32 = core::f32::consts::FRAC_PI_2;

const SECTOR_RADIANS: f32 = core::f32::consts::TAU / 3.0;

pub fn paint(scene: &Scene, viewport: &Viewport, painter: &egui::Painter) {
    for asteroid in &scene.asteroids {
        let alpha = resting_alpha(scene, viewport, asteroid.pos);
        if alpha <= 0.0 {
            continue;
        }
        paint_circle(
            painter,
            viewport,
            asteroid.pos,
            scene.zone,
            ZONE_COLOUR.gamma_multiply(ZONE_ALPHA * alpha),
        );
        if let (Some(centre), Some(meters_per_point)) = (
            viewport.point_of(asteroid.pos),
            viewport.meters_per_point(asteroid.pos),
        ) {
            paint_yield_mark(
                painter,
                asteroid,
                centre,
                scene.zone as f32 / meters_per_point,
                scene.largest_cap(),
                alpha,
            );
        }
    }

    for ship in &scene.entities {
        let Some(range) = ship.reach() else {
            continue;
        };
        let alpha = resting_alpha(scene, viewport, ship.pos);
        if alpha <= 0.0 {
            continue;
        }
        paint_circle(
            painter,
            viewport,
            ship.pos,
            range,
            seat_color32(ship.seat).gamma_multiply(RANGE_ALPHA * alpha),
        );
    }

    for flight in &scene.flights {
        let (Some(from), Some(to)) = (viewport.point_of(flight.from), viewport.point_of(flight.to))
        else {
            continue;
        };
        paint_flight_line(painter, from, to, flight.previewed);
    }

    for (asteroid, arcs) in &scene.fights {
        if scene.wheel_of(*asteroid).is_some() {
            continue;
        }
        let Some(standing) = scene.asteroids.iter().find(|it| it.id == *asteroid) else {
            continue;
        };
        let alpha = zone_alpha(
            scene,
            viewport,
            standing.pos,
            FIGHT_GONE_ZONE_POINTS,
            FIGHT_WHOLE_ZONE_POINTS,
        );
        if let (Some(centre), Some(meters_per_point)) = (
            viewport.point_of(standing.pos).filter(|_| alpha > 0.0),
            viewport.meters_per_point(standing.pos),
        ) {
            let stand_off = wheel::Wheel::stand_off(scene.zone as f32 / meters_per_point);
            paint_fight_bars(painter, centre, stand_off, arcs, alpha);
        }
    }
}

pub fn resting_alpha(scene: &Scene, viewport: &Viewport, at: Vec3) -> f32 {
    zone_alpha(
        scene,
        viewport,
        at,
        RESTING_GONE_ZONE_POINTS,
        RESTING_WHOLE_ZONE_POINTS,
    )
}

fn zone_alpha(scene: &Scene, viewport: &Viewport, at: Vec3, gone: f32, whole: f32) -> f32 {
    viewport
        .meters_per_point(at)
        .map_or(0.0, |meters_per_point| {
            zoom::faded(scene.zone as f32 / meters_per_point, gone, whole)
        })
}

fn paint_yield_mark(
    painter: &egui::Painter,
    asteroid: &AsteroidView,
    centre: Pos2,
    zone_points: f32,
    largest: f64,
    alpha: f32,
) {
    let inner = stand_off(zone_points);
    let thickness = zoom::floored(
        inner * YIELD_MARK_THICKNESS_SHARE,
        YIELD_MARK_THICKNESS_POINTS,
    );
    for (at, (material, cap)) in asteroid.caps.amounts().enumerate() {
        let left = (cap - asteroid.pull[material]).max(0.0);
        if largest <= 0.0 {
            continue;
        }
        let reach = thickness * (left / largest).clamp(0.0, 1.0) as f32;
        if reach < YIELD_MARK_SLIVER_POINTS {
            continue;
        }
        let from = YIELD_MARK_FIRST_SECTOR_RADIANS + SECTOR_RADIANS * at as f32;
        let outer = inner + reach;
        painter.add(sector(
            centre,
            from,
            inner,
            outer,
            hue::of(material).gamma_multiply(alpha),
        ));
    }
}

fn stand_off(zone_points: f32) -> f32 {
    zoom::floored(zone_points + wheel::SECTION_GAP, YIELD_MARK_FLOOR_POINTS)
}

fn sector(centre: Pos2, from: f32, inner: f32, outer: f32, colour: Color32) -> Shape {
    let at = |radius: f32, step: usize| {
        let angle = from + SECTOR_RADIANS * step as f32 / YIELD_MARK_SEGMENTS as f32;
        let (sin, cos) = angle.sin_cos();
        let aside = match step {
            0 => wheel::SECTION_GAP / 2.0,
            YIELD_MARK_SEGMENTS => -wheel::SECTION_GAP / 2.0,
            _ => 0.0,
        };
        centre + egui::vec2(radius * cos - aside * sin, radius * sin + aside * cos)
    };
    let ring: Vec<Pos2> = (0..=YIELD_MARK_SEGMENTS)
        .map(|step| at(outer, step))
        .chain((0..=YIELD_MARK_SEGMENTS).rev().map(|step| at(inner, step)))
        .collect();
    Shape::mesh(glyph::even_odd_mesh(&[ring], colour))
}

fn paint_fight_bars(
    painter: &egui::Painter,
    centre: Pos2,
    stand_off: f32,
    arcs: &[Arc],
    alpha: f32,
) {
    let length = wheel::BAR_LENGTH;
    for (at, arc) in arcs.iter().enumerate() {
        let top = -length / 2.0 + (length + wheel::SECTION_GAP) * at as f32;
        let along = |fraction: f32| top + length * fraction.clamp(0.0, 1.0);
        let segment = |from: f32, to: f32, colour: Color32| {
            (to > from).then(|| {
                Shape::line(
                    wheel::spine_points(centre, 1.0, stand_off, from, to, Side::Right),
                    Stroke::new(wheel::BAR_WIDTH, colour.gamma_multiply(alpha)),
                )
            })
        };
        painter.extend(
            segment(top, along(arc.fraction), seat_color32(arc.seat))
                .into_iter()
                .chain(segment(
                    along(arc.fraction),
                    along(arc.trailing),
                    wheel::BAR_TRAIL,
                )),
        );
    }
}

fn paint_circle(
    painter: &egui::Painter,
    viewport: &Viewport,
    centre: Vec3,
    radius: f64,
    colour: Color32,
) {
    let points: Option<Vec<Pos2>> = (0..=CIRCLE_SEGMENTS)
        .map(|step| {
            let angle = core::f64::consts::TAU * step as f64 / CIRCLE_SEGMENTS as f64;
            let (sin, cos) = angle.sin_cos();
            viewport.point_of(centre + Vec3::new(radius * cos, 0.0, radius * sin))
        })
        .collect();
    if let Some(points) = points {
        painter.add(Shape::line(points, Stroke::new(CIRCLE_WIDTH, colour)));
    }
}

fn paint_flight_line(painter: &egui::Painter, from: Pos2, to: Pos2, previewed: bool) {
    let preview = match previewed {
        true => wheel::PREVIEW_ALPHA,
        false => 1.0,
    };
    let delta = to - from;
    let length = delta.length();
    if length <= 0.0 {
        return;
    }
    let direction = delta / length;
    let cycle = FLIGHT_DASH_LENGTH + FLIGHT_GAP_LENGTH;
    let elapsed = painter.ctx().input(|input| input.time) as f32;
    let offset = (elapsed * FLIGHT_SPEED) % cycle;

    let mut start = offset - cycle;
    while start < length {
        let end = (start + FLIGHT_DASH_LENGTH).min(length);
        let clipped_start = start.max(0.0);
        if clipped_start < end {
            let mid = (clipped_start + end) / 2.0;
            let alpha =
                FLIGHT_FAINT_ALPHA + (FLIGHT_FULL_ALPHA - FLIGHT_FAINT_ALPHA) * (mid / length);
            painter.line_segment(
                [from + direction * clipped_start, from + direction * end],
                Stroke::new(FLIGHT_WIDTH, FLIGHT_COLOUR.gamma_multiply(alpha * preview)),
            );
        }
        start += cycle;
    }
}
