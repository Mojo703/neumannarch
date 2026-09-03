//! One glyph painted on the HUD, by the three rules: the frame in its fill
//! state, the marks inside it, and the dim a preview draws it at.

use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};

use crate::display::glyph::{Frame, Glyph, GlyphMark, geometry};
use crate::display::scene::Fill;

/// A glyph's outline width, in points.
const OUTLINE_WIDTH: f32 = 1.5;

/// A dashed outline's dash length, in points.
const DASH_LENGTH: f32 = 3.0;

/// A dashed outline's gap length, in points.
const GAP_LENGTH: f32 = 2.0;

/// The alpha a dimmed glyph is painted at, over its own.
const DIM_ALPHA: f32 = 0.5;

/// The straight segments an arc mark is approximated by.
const ARC_SEGMENTS: usize = 8;

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
}

impl Stencil<'_> {
    /// Paints the frame, then the marks over it.
    pub fn paint(&self, painter: &egui::Painter) {
        let points = self.frame_points();
        let outline = Stroke::new(OUTLINE_WIDTH, self.faded(Color32::WHITE));
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
    }

    /// The frame's corners, apex up for a triangle.
    fn frame_points(&self) -> Vec<Pos2> {
        let (Pos2 { x, y }, half) = (self.centre, self.half);
        match self.glyph.frame {
            Frame::Square => vec![
                egui::pos2(x - half, y - half),
                egui::pos2(x + half, y - half),
                egui::pos2(x + half, y + half),
                egui::pos2(x - half, y + half),
            ],
            Frame::Triangle => vec![
                egui::pos2(x, y - half),
                egui::pos2(x + half, y + half),
                egui::pos2(x - half, y + half),
            ],
        }
    }

    /// Each mark, at its own place on the frame; see [`GlyphMark::anchor`].
    fn paint_marks(&self, painter: &egui::Painter, colour: Color32) {
        for mark in &self.glyph.marks {
            let (ax, ay) = mark.anchor(&self.glyph.frame);
            let at = egui::pos2(
                self.centre.x + ax * self.half,
                self.centre.y + ay * self.half,
            );
            self.paint_mark(painter, mark, at, colour);
        }
    }

    fn paint_mark(&self, painter: &egui::Painter, mark: &GlyphMark, at: Pos2, colour: Color32) {
        let half = self.half;
        match mark {
            GlyphMark::Dot => {
                let radius = (geometry::DOT_RADIUS * half).max(geometry::MIN_DOT_RADIUS);
                painter.circle_filled(at, radius, colour);
            }
            GlyphMark::Bar => {
                let width = (geometry::BAR_HALF_WIDTH * half).max(geometry::MIN_STROKE / 2.0);
                let base = self.centre.y + half;
                painter.add(Shape::convex_polygon(
                    vec![
                        egui::pos2(at.x - width, at.y),
                        egui::pos2(at.x + width, at.y),
                        egui::pos2(at.x + width, base),
                        egui::pos2(at.x - width, base),
                    ],
                    colour,
                    Stroke::NONE,
                ));
            }
            GlyphMark::Plus => {
                let arm = geometry::PLUS_ARM * half;
                let thickness = (geometry::PLUS_THICKNESS * half).max(geometry::MIN_STROKE);
                let stroke = Stroke::new(thickness, colour);
                painter.line_segment(
                    [egui::pos2(at.x, at.y - arm), egui::pos2(at.x, at.y + arm)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(at.x - arm, at.y), egui::pos2(at.x + arm, at.y)],
                    stroke,
                );
            }
            GlyphMark::Chevron => {
                let span = geometry::CHEVRON_HALF_WIDTH * half;
                let rise = geometry::CHEVRON_HEIGHT * half;
                let thickness = (geometry::CHEVRON_THICKNESS * half).max(geometry::MIN_STROKE);
                let stroke = Stroke::new(thickness, colour);
                let tip = egui::pos2(at.x, at.y);
                painter.line_segment([egui::pos2(at.x - span, at.y - rise), tip], stroke);
                painter.line_segment([tip, egui::pos2(at.x + span, at.y - rise)], stroke);
            }
            GlyphMark::Arc => {
                let radius = geometry::ARC_RADIUS * half;
                let thickness = (geometry::ARC_THICKNESS * half).max(geometry::MIN_STROKE);
                let stroke = Stroke::new(thickness, colour);
                let points: Vec<Pos2> = (0..=ARC_SEGMENTS)
                    .map(|step| {
                        let angle = -geometry::ARC_HALF_ANGLE
                            + 2.0 * geometry::ARC_HALF_ANGLE * step as f32 / ARC_SEGMENTS as f32;
                        let (sin, cos) = angle.sin_cos();
                        egui::pos2(at.x + radius * sin, at.y - radius * cos)
                    })
                    .collect();
                painter.add(Shape::line(points, stroke));
            }
            GlyphMark::Belt => {
                let width = geometry::BELT_HALF_WIDTH * half;
                let thickness = (geometry::BELT_THICKNESS * half).max(geometry::MIN_STROKE / 2.0);
                painter.add(Shape::convex_polygon(
                    vec![
                        egui::pos2(at.x - width, at.y - thickness),
                        egui::pos2(at.x + width, at.y - thickness),
                        egui::pos2(at.x + width, at.y + thickness),
                        egui::pos2(at.x - width, at.y + thickness),
                    ],
                    colour,
                    Stroke::NONE,
                ));
            }
            GlyphMark::Ring => {
                let thickness = (geometry::RING_THICKNESS * half).max(geometry::MIN_STROKE);
                painter.circle_stroke(
                    at,
                    geometry::RING_RADIUS * half,
                    Stroke::new(thickness, colour),
                );
            }
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
