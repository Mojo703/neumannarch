//! The HUD: rings, runs, fight arcs, flight lines, radar contacts and the
//! wheel, painted in screen space over the belt's projection.

use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::state::view::MassClass;
use probe_sim::{Band, RowId};

use crate::display::glyph;
use crate::display::glyph_quad::seat_color32;
use crate::display::ring::{Geometry, Layout, Span};
use crate::display::scene::{Arc, Blip, Hover, RingView, Scene, WheelBand};
use crate::display::screen::Screen;
use crate::display::stencil::Stencil;
use crate::display::tint;
use crate::display::wheel::Wheel;

/// The inner ring's screen radius, in points.
pub const INNER_RADIUS: f32 = 56.0;

/// The outer ring's screen radius, in points.
pub const OUTER_RADIUS: f32 = 88.0;

/// The wheel stands clear of both rings, so a click on one of its bands is
/// never a click on a ring.
const _: () = assert!(
    crate::display::wheel::RADIUS - crate::display::wheel::BAND_WIDTH > OUTER_RADIUS
        && OUTER_RADIUS > INNER_RADIUS
);

/// A stacked glyph's inward step per card, in points.
const STACK_STEP: f32 = 2.5;

/// A ring's own stroke width, in points, unselected.
const RING_WIDTH: f32 = 1.5;

/// A selected ring's stroke width, in points.
const SELECTED_RING_WIDTH: f32 = 3.0;

/// A ring's own stroke colour where no material leads at its rock.
const RING_COLOUR: Color32 = Color32::from_gray(140);

/// How far a ring of one material alone is pulled from [`RING_COLOUR`]
/// toward that material's own, in `0..=1`. The lobby and the match paint
/// rings through this same code, so the pull is faint enough to sit under
/// a match's runs and clear enough to group a region.
const RING_TINT: f32 = 0.5;

/// A selected ring's stroke colour.
const SELECTED_RING_COLOUR: Color32 = Color32::WHITE;

/// A fight arc's radial inset from its ring, in points.
const ARC_INSET: f32 = 6.0;

/// A fight arc's stroke width, in points.
const ARC_WIDTH: f32 = 3.0;

/// The colour a fight arc's trailing damage segment is drawn in.
const ARC_TRAIL_COLOUR: Color32 = Color32::from_rgb(220, 40, 40);

/// The straight segments an arc is approximated by.
const ARC_SEGMENTS: usize = 24;

/// A flight line's stroke width, in points.
const FLIGHT_WIDTH: f32 = 1.5;

/// A flight line's colour, at [`FLIGHT_FULL_ALPHA`] at the destination and
/// [`FLIGHT_FAINT_ALPHA`] at the ship.
const FLIGHT_COLOUR: Color32 = Color32::from_gray(200);

/// A flight line's dash length, in points.
const FLIGHT_DASH_LENGTH: f32 = 6.0;

/// A flight line's gap length, in points.
const FLIGHT_GAP_LENGTH: f32 = 5.0;

/// How fast a flight line's dashes roll toward the destination, in points
/// per second of the frame's own elapsed time: `egui`'s input clock, the
/// client's, never the sim's tick.
const FLIGHT_SPEED: f32 = 30.0;

/// A flight line's alpha at the ship's own end.
const FLIGHT_FAINT_ALPHA: f32 = 0.15;

/// A flight line's alpha at the destination.
const FLIGHT_FULL_ALPHA: f32 = 0.9;

/// A radar contact's colour: no seat, since radar does not say whose it is.
const BLIP_COLOUR: Color32 = Color32::from_gray(170);

/// A radar contact's dot radius by mass class, in points.
const BLIP_RADII: [f32; 3] = [2.0, 3.0, 4.5];

/// How far ahead of a radar contact its streak reaches, in seconds of its
/// drift past the nearest rock.
const STREAK_SECONDS: f64 = 20.0;

/// The longest a radar contact's streak is drawn, in points.
const STREAK_MAX: f32 = 18.0;

/// Paints `scene`'s HUD as `screen` projects it, with `wheel` open on the
/// selected ring where there is one.
pub fn paint(scene: &Scene, screen: &Screen, wheel: Option<&Wheel>, painter: &egui::Painter) {
    for ring in &scene.rings {
        let Some(centre) = scene
            .rocks
            .iter()
            .find(|rock| rock.id == ring.place.rock)
            .and_then(|rock| screen.point_of(rock.pos))
        else {
            continue;
        };
        paint_ring(painter, ring, centre, scene.selection == Some(ring.place));
    }

    for flight in &scene.flights {
        let Some(destination) = scene.rocks.iter().find(|rock| rock.id == flight.to) else {
            continue;
        };
        let (Some(from), Some(to)) = (
            screen.point_of(flight.from),
            screen.point_of(destination.pos),
        ) else {
            continue;
        };
        paint_flight_line(painter, from, to);
    }

    for blip in &scene.blips {
        paint_blip(painter, screen, blip);
    }

    if let Some(wheel) = wheel {
        wheel.paint(painter, hovered_slot(scene, wheel));
    }
}

/// A flight's line from `from`, the ship, to `to`, the destination: faint
/// at `from` and full at `to`, its dashes rolling toward `to` by the
/// frame's own elapsed time.
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

/// A ring's measures at `radius` points: marks are laid a widest glyph
/// apart, so a glyph of any size class stands clear of its neighbours.
fn geometry(radius: f32) -> Geometry {
    Geometry {
        radius,
        glyph: 2.0 * glyph::HALF * glyph::WIDEST_SCALE,
    }
}

/// The screen radius of a ring for `band`, in points.
pub fn ring_radius(band: Band) -> f32 {
    match band {
        Band::Inner => INNER_RADIUS,
        Band::Outer => OUTER_RADIUS,
    }
}

/// The wheel slot the scene's hover names, where it names one of `wheel`'s.
fn hovered_slot(scene: &Scene, wheel: &Wheel) -> Option<(RowId, WheelBand)> {
    match scene.hover {
        Some(Hover::Wheel { place, row, band }) if place == wheel.place() => Some((row, band)),
        _ => None,
    }
}

fn paint_ring(painter: &egui::Painter, ring: &RingView, centre: Pos2, selected: bool) {
    let radius = ring_radius(ring.place.band);
    // A selected ring is white and wider, so the brightening wins over the
    // tint rather than mixing with it.
    let (stroke_colour, stroke_width) = match selected {
        true => (SELECTED_RING_COLOUR, SELECTED_RING_WIDTH),
        false => (tint::painted(RING_COLOUR, ring.caps, RING_TINT), RING_WIDTH),
    };
    painter.circle_stroke(centre, radius, Stroke::new(stroke_width, stroke_colour));

    let layout = Layout::of(&ring.runs, geometry(radius));
    for placement in layout.placements() {
        let run = &ring.runs[placement.run];
        let mark = &run.marks[placement.mark];
        let (sin, cos) = placement.angle.sin_cos();
        let out = radius - f32::from(placement.depth) * STACK_STEP;
        Stencil {
            glyph: &mark.glyph,
            centre: egui::pos2(centre.x + out * sin, centre.y - out * cos),
            half: glyph::HALF * mark.glyph.size.scale() * placement.scale,
            colour: seat_color32(run.seat),
            fill: mark.fill,
            dim: mark.dim,
        }
        .paint(painter);
    }

    for arc in &ring.arcs {
        let Some(index) = ring.runs.iter().position(|run| run.seat == arc.seat) else {
            continue;
        };
        paint_arc(painter, centre, radius, layout.span(index), arc);
    }
}

/// A fight arc's own screen radius: [`ARC_INSET`] inside `ring_radius`, the
/// radius of the ring the fight is at, whichever band that is.
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

/// One radar contact: a dot at its position, with a streak along its drift
/// past the nearest rock, held to [`STREAK_MAX`] points.
fn paint_blip(painter: &egui::Painter, screen: &Screen, blip: &Blip) {
    let Some(at) = screen.point_of(blip.pos) else {
        return;
    };
    painter.circle_filled(at, radius_of(blip.mass), BLIP_COLOUR);
    let Some(ahead) = screen.point_of(blip.pos + blip.drift * STREAK_SECONDS) else {
        return;
    };
    let along = ahead - at;
    let held = match along.length() > STREAK_MAX {
        true => at + along.normalized() * STREAK_MAX,
        false => ahead,
    };
    painter.line_segment([at, held], Stroke::new(1.0, BLIP_COLOUR));
}

/// A radar contact's dot radius, in points; see [`BLIP_RADII`].
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

    /// A run of `marks` marks, whose glyphs no test reads.
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
