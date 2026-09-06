use mirage_engine::egui::epaint::{Mesh, WHITE_UV};
use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use neumannarch_sim::Material;

use crate::display::glyph::{self, Glyph, Primitive};
use crate::display::hue;
use crate::display::scene::Fill;
use crate::screens::panel;

const DASH_LENGTH: f32 = 3.0;

const GAP_LENGTH: f32 = 2.0;

pub const DIM_ALPHA: f32 = 0.5;

const STARVED_HALF_WIDTH: f32 = 13.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub centre: Pos2,
    pub half: f32,
}

pub struct Stencil<'a> {
    pub glyph: &'a Glyph,
    pub cell: Cell,
    pub colour: Color32,
    pub outline: Color32,
    pub fill: Fill,
    pub alpha: f32,
    pub starved: Option<Material>,
}

impl Cell {
    pub fn at(&self, point: (f32, f32)) -> Pos2 {
        let (x, y) = glyph::unit(point);
        egui::pos2(self.centre.x + x * self.half, self.centre.y + y * self.half)
    }

    pub fn length(&self, length: f32) -> f32 {
        glyph::unit_length(length) * self.half
    }

    pub fn paint(&self, painter: &egui::Painter, primitive: &Primitive, colour: Color32) {
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
            Primitive::Path(rings) => {
                let rings: Vec<Vec<Pos2>> = rings
                    .iter()
                    .map(|ring| ring.iter().map(|&point| self.at(point)).collect())
                    .collect();
                painter.add(Shape::mesh(even_odd_mesh(&rings, colour)));
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
}

impl Stencil<'_> {
    pub fn paint(&self, painter: &egui::Painter) {
        let points = self.frame_points();
        let outline = Stroke::new(
            self.cell.length(glyph::OUTLINE_WIDTH),
            self.faded(self.outline),
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
                let bottom = self.cell.centre.y + self.cell.half;
                let cutoff = bottom - fraction.clamp(0.0, 1.0) * 2.0 * self.cell.half;
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

        for primitive in glyph::primitives_of(&self.glyph.marks) {
            self.cell.paint(painter, &primitive, outline.color);
        }
        if let Some(material) = self.starved {
            self.paint_starved(painter, material);
        }
    }

    fn paint_starved(&self, painter: &egui::Painter, material: Material) {
        let width = self.cell.length(STARVED_HALF_WIDTH);
        let thickness = self.cell.length(glyph::MARK_WIDTH) / 2.0;
        let centre = self.cell.centre;
        let base = centre.y + self.cell.half + thickness;
        painter.add(Shape::convex_polygon(
            vec![
                egui::pos2(centre.x - width, base - thickness),
                egui::pos2(centre.x + width, base - thickness),
                egui::pos2(centre.x + width, base + thickness),
                egui::pos2(centre.x - width, base + thickness),
            ],
            self.faded(hue::of(material)),
            Stroke::NONE,
        ));
    }

    fn frame_points(&self) -> Vec<Pos2> {
        self.glyph
            .frame
            .points()
            .iter()
            .map(|&point| self.cell.at(point))
            .collect()
    }

    fn faded(&self, colour: Color32) -> Color32 {
        panel::BACKDROP.lerp_to_gamma(colour, self.alpha)
    }
}

fn even_odd_mesh(rings: &[Vec<Pos2>], colour: Color32) -> Mesh {
    let edges: Vec<(Pos2, Pos2)> = rings
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
        .map(|(&a, &b)| (a, b))
        .filter(|(a, b)| a.y != b.y)
        .collect();
    let mut levels: Vec<f32> = edges.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
    levels.sort_by(f32::total_cmp);
    levels.dedup();

    let mut mesh = Mesh::default();
    for band in levels.windows(2) {
        let (top, bottom) = (band[0], band[1]);
        let middle = (top + bottom) / 2.0;
        let mut crossings: Vec<(f32, f32)> = edges
            .iter()
            .filter(|(a, b)| a.y.min(b.y) <= middle && middle < a.y.max(b.y))
            .map(|(a, b)| {
                let x_at = |y: f32| a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y);
                (x_at(top), x_at(bottom))
            })
            .collect();
        crossings.sort_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1)));
        for pair in crossings.chunks_exact(2) {
            let (left, right) = (pair[0], pair[1]);
            let corners = [
                egui::pos2(left.0, top),
                egui::pos2(right.0, top),
                egui::pos2(right.1, bottom),
                egui::pos2(left.1, bottom),
            ];
            let first = mesh.vertices.len() as u32;
            for corner in corners {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: corner,
                    uv: WHITE_UV,
                    color: colour,
                });
            }
            mesh.add_triangle(first, first + 1, first + 2);
            mesh.add_triangle(first, first + 2, first + 3);
        }
    }
    mesh
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area(mesh: &Mesh) -> f32 {
        mesh.indices
            .chunks_exact(3)
            .map(|triangle| {
                let [a, b, c] = [0, 1, 2].map(|at| mesh.vertices[triangle[at] as usize].pos);
                ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn a_ring_inside_a_ring_is_a_hole_and_the_mesh_covers_the_rest() {
        let square = |half: f32| {
            vec![
                egui::pos2(-half, -half),
                egui::pos2(half, -half),
                egui::pos2(half, half),
                egui::pos2(-half, half),
            ]
        };
        let solid = even_odd_mesh(&[square(10.0)], Color32::WHITE);
        assert!((area(&solid) - 400.0).abs() < 1e-3, "{}", area(&solid));

        let pierced = even_odd_mesh(&[square(10.0), square(4.0)], Color32::WHITE);
        assert!((area(&pierced) - 336.0).abs() < 1e-3, "{}", area(&pierced));
    }

    #[test]
    fn a_concave_ring_is_filled_without_bridging_its_notch() {
        let notched = vec![
            egui::pos2(0.0, 0.0),
            egui::pos2(10.0, 0.0),
            egui::pos2(10.0, 10.0),
            egui::pos2(6.0, 10.0),
            egui::pos2(6.0, 4.0),
            egui::pos2(4.0, 4.0),
            egui::pos2(4.0, 10.0),
            egui::pos2(0.0, 10.0),
        ];
        let mesh = even_odd_mesh(&[notched], Color32::WHITE);
        assert!((area(&mesh) - 88.0).abs() < 1e-3, "{}", area(&mesh));
    }
}
