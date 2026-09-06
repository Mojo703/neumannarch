use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use neumannarch_sim::Vec3;

use crate::display::glyph_quad::seat_color32;
use crate::display::scene::Scene;
use crate::display::viewport::Viewport;

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

pub fn paint(scene: &Scene, viewport: &Viewport, painter: &egui::Painter) {
    for asteroid in &scene.asteroids {
        paint_circle(
            painter,
            viewport,
            asteroid.pos,
            scene.zone,
            ZONE_COLOUR.gamma_multiply(ZONE_ALPHA),
        );
    }

    for ship in &scene.entities {
        let Some(range) = ship.range else {
            continue;
        };
        paint_circle(
            painter,
            viewport,
            ship.pos,
            range,
            seat_color32(ship.seat).gamma_multiply(RANGE_ALPHA),
        );
    }

    for flight in &scene.flights {
        let Some(destination) = scene
            .asteroids
            .iter()
            .find(|asteroid| asteroid.id == flight.to)
        else {
            continue;
        };
        let (Some(from), Some(to)) = (
            viewport.point_of(flight.from),
            viewport.point_of(destination.pos),
        ) else {
            continue;
        };
        paint_flight_line(painter, from, to);
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

fn paint_flight_line(painter: &egui::Painter, from: Pos2, to: Pos2) {
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
                Stroke::new(FLIGHT_WIDTH, FLIGHT_COLOUR.gamma_multiply(alpha)),
            );
        }
        start += cycle;
    }
}
