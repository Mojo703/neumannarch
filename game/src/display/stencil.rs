use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::Material;

use crate::display::glyph::{self, Glyph, Primitive};
use crate::display::hue;
use crate::display::scene::Fill;
use crate::screens::panel;

const DASH_LENGTH: f32 = 3.0;

const GAP_LENGTH: f32 = 2.0;

pub const DIM_ALPHA: f32 = 0.5;

const STARVED_HALF_WIDTH: f32 = 13.0;

pub struct Stencil<'a> {
    pub glyph: &'a Glyph,
    pub centre: Pos2,
    pub half: f32,
    pub colour: Color32,
    pub fill: Fill,
    pub alpha: f32,
    pub starved: Option<Material>,
}

impl Stencil<'_> {
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

    fn at(&self, point: (f32, f32)) -> Pos2 {
        let (x, y) = glyph::unit(point);
        egui::pos2(self.centre.x + x * self.half, self.centre.y + y * self.half)
    }

    fn length(&self, length: f32) -> f32 {
        glyph::unit_length(length) * self.half
    }

    fn frame_points(&self) -> Vec<Pos2> {
        self.glyph
            .frame
            .points()
            .iter()
            .map(|&point| self.at(point))
            .collect()
    }

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
        }
    }

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
        panel::BACKDROP.lerp_to_gamma(colour, self.alpha)
    }
}

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
