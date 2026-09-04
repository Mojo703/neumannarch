use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::{RowId, SeatId};

use crate::display::glyph;
use crate::display::glyph_quad::seat_color32;
use crate::display::ring::{Geometry, Layout, Span};
use crate::display::scene::{Arc, Hover, Mark, RingView, Scene, WheelBand};
use crate::display::stencil::Stencil;
use crate::display::tint;
use crate::display::viewport::Viewport;
use crate::display::wheel::Wheel;

pub const RING_RADIUS: f32 = 56.0;

const _: () =
    assert!(crate::display::wheel::RADIUS - crate::display::wheel::BAND_WIDTH > RING_RADIUS);

const STACK_STEP: f32 = 2.5;

const RING_WIDTH: f32 = 1.5;

const SELECTED_RING_WIDTH: f32 = 3.0;

const RING_COLOUR: Color32 = Color32::from_gray(140);

const RING_TINT: f32 = 0.5;

const SELECTED_RING_COLOUR: Color32 = Color32::WHITE;

const ARC_INSET: f32 = 6.0;

const ARC_RADIUS: f32 = RING_RADIUS - ARC_INSET;

const _: () = assert!(ARC_RADIUS < RING_RADIUS && ARC_RADIUS > 0.0);

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

pub fn paint(scene: &Scene, viewport: &Viewport, wheel: Option<&Wheel>, painter: &egui::Painter) {
    for ring in &scene.rings {
        let Some(centre) = centre_of(scene, viewport, ring) else {
            continue;
        };
        paint_ring(painter, ring, centre, scene.selection == Some(ring.rock));
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

fn hovered_slot(scene: &Scene, wheel: &Wheel) -> Option<(RowId, WheelBand)> {
    match scene.hover {
        Some(Hover::Wheel { rock, row, band }) if rock == wheel.rock() => Some((row, band)),
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
    let layout = Layout::of(&ring.runs, geometry(RING_RADIUS));
    layout
        .placements()
        .iter()
        .map(|placement| {
            let run = &ring.runs[placement.run];
            let mark = &run.marks[placement.mark];
            let (sin, cos) = placement.angle.sin_cos();
            let out = RING_RADIUS - f32::from(placement.depth) * STACK_STEP;
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
        .find(|rock| rock.id == ring.rock)
        .and_then(|rock| viewport.point_of(rock.pos))
}

fn paint_ring(painter: &egui::Painter, ring: &RingView, centre: Pos2, selected: bool) {
    let (stroke_colour, stroke_width) = match selected {
        true => (SELECTED_RING_COLOUR, SELECTED_RING_WIDTH),
        false => (tint::painted(RING_COLOUR, ring.caps, RING_TINT), RING_WIDTH),
    };
    painter.circle_stroke(
        centre,
        RING_RADIUS,
        Stroke::new(stroke_width, stroke_colour),
    );

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

    let layout = Layout::of(&ring.runs, geometry(RING_RADIUS));
    for arc in &ring.arcs {
        let Some(index) = ring.runs.iter().position(|run| run.seat == arc.seat) else {
            continue;
        };
        paint_arc(painter, centre, layout.span(index), arc);
    }
}

fn paint_arc(painter: &egui::Painter, centre: Pos2, span: Span, arc: &Arc) {
    let radius = ARC_RADIUS;
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
        let laid = Layout::of(&[run(2)], geometry(RING_RADIUS));
        let placed = laid.placements();

        let apart = (placed[1].angle - placed[0].angle) * RING_RADIUS;

        assert!(
            apart >= 2.0 * glyph::HALF * glyph::WIDEST_SCALE - 1e-4,
            "marks {apart} points apart overlap the widest glyph"
        );
    }
}
