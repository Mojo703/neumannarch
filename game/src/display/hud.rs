use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::roster::MassClass;
use probe_sim::{Band, RowId, SeatId};

use crate::display::glyph;
use crate::display::glyph_quad::seat_color32;
use crate::display::ring::{Geometry, Layout, Span};
use crate::display::scene::{Arc, Blip, Hover, Mark, RingView, Scene, WheelBand};
use crate::display::stencil::Stencil;
use crate::display::tint;
use crate::display::viewport::Viewport;
use crate::display::wheel::Wheel;

pub const INNER_RADIUS: f32 = 56.0;

pub const OUTER_RADIUS: f32 = 88.0;

const _: () = assert!(
    crate::display::wheel::RADIUS - crate::display::wheel::BAND_WIDTH > OUTER_RADIUS
        && OUTER_RADIUS > INNER_RADIUS
);

const STACK_STEP: f32 = 2.5;

const RING_WIDTH: f32 = 1.5;

const SELECTED_RING_WIDTH: f32 = 3.0;

const RING_COLOUR: Color32 = Color32::from_gray(140);

const RING_TINT: f32 = 0.5;

const SELECTED_RING_COLOUR: Color32 = Color32::WHITE;

const ARC_INSET: f32 = 6.0;

const ARC_WIDTH: f32 = 3.0;

const ARC_TRAIL_COLOUR: Color32 = Color32::from_rgb(220, 40, 40);

const ARC_SEGMENTS: usize = 24;

const FLIGHT_WIDTH: f32 = 1.5;

const FLIGHT_COLOUR: Color32 = Color32::from_gray(200);

const FLIGHT_DASH_LENGTH: f32 = 6.0;

const FLIGHT_GAP_LENGTH: f32 = 5.0;

const FLIGHT_SPEED: f32 = 30.0;

const FLIGHT_FAINT_ALPHA: f32 = 0.15;

const FLIGHT_FULL_ALPHA: f32 = 0.9;

const BLIP_COLOUR: Color32 = Color32::from_gray(170);

const BLIP_RADII: [f32; 3] = [2.0, 3.0, 4.5];

const STREAK_SECONDS: f64 = 20.0;

const STREAK_MAX: f32 = 18.0;

pub fn paint(scene: &Scene, viewport: &Viewport, wheel: Option<&Wheel>, painter: &egui::Painter) {
    for ring in &scene.rings {
        let Some(centre) = centre_of(scene, viewport, ring) else {
            continue;
        };
        paint_ring(painter, ring, centre, scene.selection == Some(ring.place));
    }

    for flight in &scene.flights {
        let Some(destination) = scene.rocks.iter().find(|rock| rock.id == flight.to) else {
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

    for blip in &scene.blips {
        paint_blip(painter, viewport, blip);
    }

    if let Some(wheel) = wheel {
        wheel.paint(painter, hovered_slot(scene, wheel));
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

fn geometry(radius: f32) -> Geometry {
    Geometry {
        radius,
        glyph: 2.0 * glyph::HALF * glyph::WIDEST_SCALE,
    }
}

pub fn ring_radius(band: Band) -> f32 {
    match band {
        Band::Inner => INNER_RADIUS,
        Band::Outer => OUTER_RADIUS,
    }
}

fn hovered_slot(scene: &Scene, wheel: &Wheel) -> Option<(RowId, WheelBand)> {
    match scene.hover {
        Some(Hover::Wheel { place, row, band }) if place == wheel.place() => Some((row, band)),
        _ => None,
    }
}

struct Placed<'a> {
    mark: &'a Mark,
    seat: SeatId,
    centre: Pos2,
    half: f32,
}

fn placed<'a>(ring: &'a RingView, centre: Pos2) -> Vec<Placed<'a>> {
    let radius = ring_radius(ring.place.band);
    let layout = Layout::of(&ring.runs, geometry(radius));
    layout
        .placements()
        .iter()
        .map(|placement| {
            let run = &ring.runs[placement.run];
            let mark = &run.marks[placement.mark];
            let (sin, cos) = placement.angle.sin_cos();
            let out = radius - f32::from(placement.depth) * STACK_STEP;
            Placed {
                mark,
                seat: run.seat,
                centre: egui::pos2(centre.x + out * sin, centre.y - out * cos),
                half: glyph::HALF * mark.glyph.size.scale() * placement.scale,
            }
        })
        .collect()
}

pub fn glyph_at<'a>(scene: &'a Scene, viewport: &Viewport, at: Pos2) -> Option<(Pos2, &'a Mark)> {
    scene
        .rings
        .iter()
        .filter_map(|ring| Some((ring, centre_of(scene, viewport, ring)?)))
        .flat_map(|(ring, centre)| placed(ring, centre))
        .filter(|placed| placed.centre.distance(at) <= placed.half)
        .min_by(|a, b| a.centre.distance(at).total_cmp(&b.centre.distance(at)))
        .map(|placed| (placed.centre, placed.mark))
}

fn centre_of(scene: &Scene, viewport: &Viewport, ring: &RingView) -> Option<Pos2> {
    scene
        .rocks
        .iter()
        .find(|rock| rock.id == ring.place.rock)
        .and_then(|rock| viewport.point_of(rock.pos))
}

fn paint_ring(painter: &egui::Painter, ring: &RingView, centre: Pos2, selected: bool) {
    let radius = ring_radius(ring.place.band);

    let (stroke_colour, stroke_width) = match selected {
        true => (SELECTED_RING_COLOUR, SELECTED_RING_WIDTH),
        false => (tint::painted(RING_COLOUR, ring.caps, RING_TINT), RING_WIDTH),
    };
    painter.circle_stroke(centre, radius, Stroke::new(stroke_width, stroke_colour));

    for placed in placed(ring, centre) {
        Stencil {
            glyph: &placed.mark.glyph,
            centre: placed.centre,
            half: placed.half,
            colour: seat_color32(placed.seat),
            fill: placed.mark.fill,
            dim: placed.mark.dim,
            starved: placed.mark.reason.starved(),
        }
        .paint(painter);
    }

    let layout = Layout::of(&ring.runs, geometry(radius));
    for arc in &ring.arcs {
        let Some(index) = ring.runs.iter().position(|run| run.seat == arc.seat) else {
            continue;
        };
        paint_arc(painter, centre, radius, layout.span(index), arc);
    }
}

fn arc_radius(ring_radius: f32) -> f32 {
    ring_radius - ARC_INSET
}

fn paint_arc(painter: &egui::Painter, centre: Pos2, ring_radius: f32, span: Span, arc: &Arc) {
    let radius = arc_radius(ring_radius);
    let drained = span.at(arc.fraction);
    paint_arc_segment(
        painter,
        centre,
        radius,
        span.start,
        drained,
        Stroke::new(ARC_WIDTH, seat_color32(arc.seat)),
    );
    if arc.trailing > arc.fraction {
        paint_arc_segment(
            painter,
            centre,
            radius,
            drained,
            span.at(arc.trailing),
            Stroke::new(ARC_WIDTH, ARC_TRAIL_COLOUR),
        );
    }
}

fn paint_arc_segment(
    painter: &egui::Painter,
    centre: Pos2,
    radius: f32,
    start: f32,
    end: f32,
    stroke: Stroke,
) {
    if end <= start {
        return;
    }
    let points: Vec<Pos2> = (0..=ARC_SEGMENTS)
        .map(|step| {
            let angle = start + (end - start) * step as f32 / ARC_SEGMENTS as f32;
            let (sin, cos) = angle.sin_cos();
            egui::pos2(centre.x + radius * sin, centre.y - radius * cos)
        })
        .collect();
    painter.add(Shape::line(points, stroke));
}

fn paint_blip(painter: &egui::Painter, viewport: &Viewport, blip: &Blip) {
    let Some(at) = viewport.point_of(blip.pos) else {
        return;
    };
    painter.circle_filled(at, radius_of(blip.mass), BLIP_COLOUR);
    let Some(ahead) = viewport.point_of(blip.pos + blip.drift * STREAK_SECONDS) else {
        return;
    };
    let along = ahead - at;
    let held = match along.length() > STREAK_MAX {
        true => at + along.normalized() * STREAK_MAX,
        false => ahead,
    };
    painter.line_segment([at, held], Stroke::new(1.0, BLIP_COLOUR));
}

fn radius_of(mass: MassClass) -> f32 {
    BLIP_RADII[match mass {
        MassClass::Light => 0,
        MassClass::Medium => 1,
        MassClass::Heavy => 2,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(marks: usize) -> crate::display::scene::Run {
        crate::display::scene::Run {
            seat: probe_sim::SeatId(0),
            marks: (0..marks)
                .map(|_| crate::display::scene::Mark {
                    glyph: crate::display::glyph::Glyph::of(
                        &probe_sim::roster::Roster::shipped()[RowId(0)],
                    ),
                    fill: crate::display::scene::Fill::Solid,
                    dim: false,
                    reason: crate::display::scene::Reason::Here(RowId(0)),
                })
                .collect(),
        }
    }

    #[test]
    fn consecutive_marks_of_a_run_stand_a_widest_glyph_apart() {
        let laid = Layout::of(&[run(2)], geometry(INNER_RADIUS));
        let placed = laid.placements();

        let apart = (placed[1].angle - placed[0].angle) * INNER_RADIUS;

        assert!(
            apart >= 2.0 * glyph::HALF * glyph::WIDEST_SCALE - 1e-4,
            "marks {apart} points apart overlap the widest glyph"
        );
    }

    #[test]
    fn a_ring_radius_follows_its_band() {
        assert_eq!(ring_radius(Band::Inner), INNER_RADIUS);
        assert_eq!(ring_radius(Band::Outer), OUTER_RADIUS);
    }

    #[test]
    fn a_fight_arc_is_inset_within_the_ring_it_fights_at_whichever_band() {
        for radius in [INNER_RADIUS, OUTER_RADIUS] {
            assert_eq!(arc_radius(radius), radius - ARC_INSET);
            assert!(arc_radius(radius) < radius);
        }
        assert_ne!(arc_radius(INNER_RADIUS), arc_radius(OUTER_RADIUS));
    }

    #[test]
    fn a_heavier_contact_draws_a_bigger_dot() {
        assert!(radius_of(MassClass::Light) < radius_of(MassClass::Medium));
        assert!(radius_of(MassClass::Medium) < radius_of(MassClass::Heavy));
    }
}
