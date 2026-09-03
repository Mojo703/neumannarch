//! One glyph painted on the HUD, by the three rules: the frame in its fill
//! state, the marks inside it, and the dim a preview draws it at. Frame and
//! marks are the sheet's own primitives (`glyph::Frame::points`,
//! `glyph::primitives_of`), scaled by [`Stencil::half`].

use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::Material;

use crate::display::glyph::{self, Glyph, Primitive};
use crate::display::hue;
use crate::display::scene::Fill;

/// A dashed outline's dash length, in points.
const DASH_LENGTH: f32 = 3.0;

/// A dashed outline's gap length, in points.
const GAP_LENGTH: f32 = 2.0;

/// The alpha a dimmed glyph is painted at, over its own.
const DIM_ALPHA: f32 = 0.5;

/// The straight segments an arc mark is approximated by.
const ARC_SEGMENTS: usize = 8;

/// A starved frame's outside belt's half-width, in cell units: the
/// plating belt's own half-width (`43 - 17`, halved), since it sits along
/// the same base.
const STARVED_HALF_WIDTH: f32 = 13.0;

/// One glyph ready to paint: where it goes, how big, whose colour it takes,
/// and what its unit's state is.
pub struct Stencil<'a> {
    pub glyph: &'a Glyph,
    /// The glyph's centre, in points.
    pub centre: Pos2,
    /// Half the glyph's width at its size class, in points.
    pub half: f32,
    /// The fill's colour: the owner's.
    pub colour: Color32,
    pub fill: Fill,
    /// Painted at [`DIM_ALPHA`]: a hover preview, or a glyph the pointer
    /// says is leaving.
    pub dim: bool,
    /// The material a frame has spent nothing on this second for want of,
    /// drawn as a belt along the frame's base in that material's hue.
    pub starved: Option<Material>,
}

impl Stencil<'_> {
    /// Paints the frame, then the marks over it.
    pub fn paint(&self, painter: &egui::Painter) {
        let points = self.frame_points();
        let outline = Stroke::new(
            self.length(glyph::OUTLINE_WIDTH),
            self.faded(Color32::WHITE),
        );
        let fill = self.faded(self.colour);

        match self.fill {
            Fill::Solid => {
                painter.add(Shape::convex_polygon(points, fill, outline));
            }
            Fill::Hollow => {
                painter.add(Shape::closed_line(points, outline));
            }
            Fill::Filling(fraction) => {
                let bottom = self.centre.y + self.half;
                let cutoff = bottom - fraction.clamp(0.0, 1.0) * 2.0 * self.half;
                let filled = below(&points, cutoff);
                if filled.len() >= 3 {
                    painter.add(Shape::convex_polygon(filled, fill, Stroke::NONE));
                }
                painter.add(Shape::closed_line(points, outline));
            }
            Fill::Dashed => {
                let mut path = points;
                path.push(path[0]);
                painter.extend(Shape::dashed_line(&path, outline, DASH_LENGTH, GAP_LENGTH));
            }
        }

        self.paint_marks(painter, outline.color);
        if let Some(material) = self.starved {
            self.paint_starved(painter, material);
        }
    }

    /// The belt a starved frame carries: along the base, just outside it,
    /// so the plating belt inside the frame stays its own mark.
    fn paint_starved(&self, painter: &egui::Painter, material: Material) {
        let width = self.length(STARVED_HALF_WIDTH);
        let thickness = self.length(glyph::MARK_WIDTH) / 2.0;
        let base = self.centre.y + self.half + thickness;
        painter.add(Shape::convex_polygon(
            vec![
                egui::pos2(self.centre.x - width, base - thickness),
                egui::pos2(self.centre.x + width, base - thickness),
                egui::pos2(self.centre.x + width, base + thickness),
                egui::pos2(self.centre.x - width, base + thickness),
            ],
            self.faded(hue::of(material)),
            Stroke::NONE,
        ));
    }

    /// `point`, in the sheet's cell, at this stencil's own centre and
    /// half-width.
    fn at(&self, point: (f32, f32)) -> Pos2 {
        let (x, y) = glyph::unit(point);
        egui::pos2(self.centre.x + x * self.half, self.centre.y + y * self.half)
    }

    /// `length`, in the sheet's cell, at this stencil's own half-width.
    fn length(&self, length: f32) -> f32 {
        glyph::unit_length(length) * self.half
    }

    /// The frame's corners, apex up for a triangle.
    fn frame_points(&self) -> Vec<Pos2> {
        self.glyph
            .frame
            .points()
            .iter()
            .map(|&point| self.at(point))
            .collect()
    }

    /// Every primitive the glyph's marks draw, at its own place and size.
    fn paint_marks(&self, painter: &egui::Painter, colour: Color32) {
        for primitive in glyph::primitives_of(&self.glyph.marks) {
            self.paint_primitive(painter, &primitive, colour);
        }
    }

    fn paint_primitive(&self, painter: &egui::Painter, primitive: &Primitive, colour: Color32) {
        let stroke = Stroke::new(self.length(glyph::MARK_WIDTH), colour);
        match primitive {
            Primitive::Dot { at, radius } => {
                painter.circle_filled(self.at(*at), self.length(*radius), colour);
            }
            Primitive::Line(points) => {
                self.paint_round_line(painter, points, stroke);
            }
            Primitive::Ring { at, radius } => {
                painter.circle_stroke(self.at(*at), self.length(*radius), stroke);
            }
            Primitive::Arc { at, radius } => {
                let centre = self.at(*at);
                let radius = self.length(*radius);
                let points: Vec<Pos2> = (0..=ARC_SEGMENTS)
                    .map(|step| {
                        let angle = -core::f32::consts::FRAC_PI_2
                            + core::f32::consts::PI * step as f32 / ARC_SEGMENTS as f32;
                        let (sin, cos) = angle.sin_cos();
                        egui::pos2(centre.x + radius * sin, centre.y - radius * cos)
                    })
                    .collect();
                painter.add(Shape::line(points, stroke));
            }
        }
    }

    /// `points`, in the sheet's cell, as connected segments with a filled
    /// circle at each vertex, approximating `stroke`'s round caps and
    /// joins.
    fn paint_round_line(&self, painter: &egui::Painter, points: &[(f32, f32)], stroke: Stroke) {
        let screen: Vec<Pos2> = points.iter().map(|&point| self.at(point)).collect();
        for pair in screen.windows(2) {
            painter.line_segment([pair[0], pair[1]], stroke);
        }
        for &vertex in &screen {
            painter.circle_filled(vertex, stroke.width / 2.0, stroke.color);
        }
    }

    fn faded(&self, colour: Color32) -> Color32 {
        match self.dim {
            true => colour.gamma_multiply(DIM_ALPHA),
            false => colour,
        }
    }
}

/// `points`, a convex polygon, clipped to the half-plane at and below
/// `cutoff`, which is toward the bottom of the screen.
fn below(points: &[Pos2], cutoff: f32) -> Vec<Pos2> {
    let mut kept = Vec::new();
    for (index, &current) in points.iter().enumerate() {
        let next = points[(index + 1) % points.len()];
        let (current_in, next_in) = (current.y >= cutoff, next.y >= cutoff);
        if current_in {
            kept.push(current);
        }
        if current_in != next_in {
            let along = (cutoff - current.y) / (next.y - current.y);
            kept.push(egui::pos2(current.x + along * (next.x - current.x), cutoff));
        }
    }
    kept
}
