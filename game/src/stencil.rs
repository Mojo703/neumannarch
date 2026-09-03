//! One glyph painted on the HUD, by the three rules: the frame in its fill
//! state, the marks inside it, and the dim a preview draws it at.

use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};

use crate::glyph::{Frame, Glyph, GlyphMark};
use crate::scene::Fill;

/// A glyph's outline width, in points.
const OUTLINE_WIDTH: f32 = 1.5;

/// A dashed outline's dash length, in points.
const DASH_LENGTH: f32 = 3.0;

/// A dashed outline's gap length, in points.
const GAP_LENGTH: f32 = 2.0;

/// The alpha a dimmed glyph is painted at, over its own.
const DIM_ALPHA: f32 = 0.5;

/// A mark's half-size, as a fraction of the glyph's half-width.
const MARK_HALF: f32 = 0.3;

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

    /// The weapon marks, spread evenly across the glyph's width.
    fn paint_marks(&self, painter: &egui::Painter, colour: Color32) {
        let count = self.glyph.marks.len();
        let radius = self.half * MARK_HALF;
        for (index, mark) in self.glyph.marks.iter().enumerate() {
            let at = egui::pos2(
                self.centre.x + spread(index, count) * self.half,
                self.centre.y,
            );
            match mark {
                GlyphMark::Dot => {
                    painter.circle_filled(at, radius, colour);
                }
                GlyphMark::Bar => {
                    let height = radius * 0.7;
                    painter.add(Shape::convex_polygon(
                        vec![
                            egui::pos2(at.x - radius, at.y - height),
                            egui::pos2(at.x + radius, at.y - height),
                            egui::pos2(at.x + radius, at.y + height),
                            egui::pos2(at.x - radius, at.y + height),
                        ],
                        colour,
                        Stroke::NONE,
                    ));
                }
                GlyphMark::Plus => {
                    let stroke = Stroke::new(radius * 0.4, colour);
                    painter.line_segment(
                        [
                            egui::pos2(at.x, at.y - radius),
                            egui::pos2(at.x, at.y + radius),
                        ],
                        stroke,
                    );
                    painter.line_segment(
                        [
                            egui::pos2(at.x - radius, at.y),
                            egui::pos2(at.x + radius, at.y),
                        ],
                        stroke,
                    );
                }
                GlyphMark::Chevron => {
                    let stroke = Stroke::new(radius * 0.35, colour);
                    let tip = egui::pos2(at.x + radius * 0.3, at.y);
                    painter.line_segment([egui::pos2(at.x - radius, at.y - radius), tip], stroke);
                    painter.line_segment([tip, egui::pos2(at.x - radius, at.y + radius)], stroke);
                }
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

/// The `index`th of `count` marks' offset from the centre, as a fraction of
/// the glyph's half-width.
fn spread(index: usize, count: usize) -> f32 {
    match count {
        0 | 1 => 0.0,
        _ => -0.4 + 0.8 * index as f32 / (count - 1) as f32,
    }
}
