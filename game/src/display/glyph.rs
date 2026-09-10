use core::f32::consts::TAU;

use mirage_engine::egui::epaint::{Mesh, WHITE_UV};
use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use mirage_engine::math::UVec2;
use mirage_engine::{Color, TextureData};
use neumannarch_sim::Material;
use neumannarch_sim::pattern::{Frame, Glyph, Role, Tier};

use crate::display::hue;
use crate::display::scene::Fill;
use crate::screens::panel;

pub const HALF: f32 = 11.0;

pub const CELL_PIXELS: u32 = 48;

const CENTRE: (f32, f32) = (30.0, 30.0);

const REFERENCE_HALF: f32 = 28.0;

pub const OUTLINE_WIDTH: f32 = 2.0;

pub const MARK_WIDTH: f32 = 4.5;

const DASH_LENGTH: f32 = 3.0;

const GAP_LENGTH: f32 = 2.0;

pub const DIM_ALPHA: f32 = 0.5;

const STARVED_HALF_WIDTH: f32 = 13.0;

const ROUND_STEPS: usize = 16;

const UNIT_CUT_DROP: f32 = 3.0;

const ICON_SCALE: f32 = 1.7;

type Ring = Vec<(f32, f32)>;

#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    rings: Vec<Ring>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub centre: Pos2,
    pub half: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub colour: Color32,
    pub outline: Color32,
    pub fill: Fill,
    pub alpha: f32,
    pub starved: Option<Material>,
}

impl Look {
    pub fn solid(colour: Color32) -> Look {
        Look {
            colour,
            outline: colour,
            fill: Fill::Solid,
            alpha: 1.0,
            starved: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Dot { at: (f32, f32), radius: f32 },
    Line(Vec<(f32, f32)>),
}

impl Drawing {
    pub fn of(glyph: Glyph) -> Drawing {
        let drop = match glyph.frame {
            Frame::Unit => UNIT_CUT_DROP,
            Frame::Structure | Frame::Defence => 0.0,
        };
        let mut rings = frame(glyph.frame, glyph.tier);
        rings.extend(
            cut(glyph.role, glyph.material)
                .into_iter()
                .map(|ring| lowered(ring, drop)),
        );
        rings.extend(notches(glyph.tier));
        Drawing { rings }
    }

    pub fn icon(material: Material) -> Drawing {
        Drawing {
            rings: material_icon(material)
                .into_iter()
                .map(|ring| {
                    ring.into_iter()
                        .map(|(x, y)| {
                            (
                                CENTRE.0 + (x - CENTRE.0) * ICON_SCALE,
                                CENTRE.1 + (y - 29.0) * ICON_SCALE,
                            )
                        })
                        .collect()
                })
                .collect(),
        }
    }

    pub fn paint(&self, painter: &egui::Painter, cell: Cell, look: Look) {
        let rings = cell.rings_at(&self.rings);
        let faded = |colour: Color32| panel::BACKDROP.lerp_to_gamma(colour, look.alpha);
        let outline = Stroke::new(cell.length(OUTLINE_WIDTH), faded(look.outline));
        let fill = faded(look.colour);
        let dim = panel::BACKDROP.lerp_to_gamma(look.colour, look.alpha * DIM_ALPHA);
        match look.fill {
            Fill::Solid => {
                painter.add(Shape::mesh(even_odd_mesh(&rings, fill)));
            }
            Fill::Hollow => {
                painter.add(Shape::mesh(even_odd_mesh(&rings, fill)));
            }
            Fill::Filling(fraction) => {
                painter.add(Shape::mesh(even_odd_mesh(&rings, dim)));
                let bottom = cell.centre.y + cell.half;
                let cutoff = bottom - fraction.clamp(0.0, 1.0) * 2.0 * cell.half;
                let clip = egui::Rect::from_min_max(
                    egui::pos2(f32::NEG_INFINITY, cutoff),
                    egui::pos2(f32::INFINITY, f32::INFINITY),
                );
                painter
                    .with_clip_rect(clip)
                    .add(Shape::mesh(even_odd_mesh(&rings, fill)));
            }
            Fill::Dashed => {
                painter.add(Shape::mesh(even_odd_mesh(&rings, dim)));
                let mut path = rings[0].clone();
                path.push(rings[0][0]);
                painter.extend(Shape::dashed_line(&path, outline, DASH_LENGTH, GAP_LENGTH));
            }
        }
        if let Some(material) = look.starved {
            let width = cell.length(STARVED_HALF_WIDTH);
            let thickness = cell.length(MARK_WIDTH) / 2.0;
            let base = cell.centre.y + cell.half + thickness;
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(cell.centre.x - width, base - thickness),
                    egui::pos2(cell.centre.x + width, base + thickness),
                ),
                0.0,
                faded(hue::of(material)),
            );
        }
    }

    pub fn texture(&self, colour: Color) -> TextureData {
        let side = CELL_PIXELS;
        let mut pixels = vec![0u8; 4 * (side * side) as usize];
        for row in 0..side {
            for col in 0..side {
                let to = |value: u32| ((value as f32 + 0.5) / side as f32) * 2.0 - 1.0;
                let (x, y) = (
                    CENTRE.0 + to(col) * REFERENCE_HALF,
                    CENTRE.1 + to(row) * REFERENCE_HALF,
                );
                if self.covers(x, y) {
                    let at = 4 * (row * side + col) as usize;
                    pixels[at..at + 4].copy_from_slice(&encode(colour));
                }
            }
        }
        TextureData::rgba8(UVec2::new(side, side), pixels).pixelated()
    }

    fn covers(&self, x: f32, y: f32) -> bool {
        let crossings = self
            .rings
            .iter()
            .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
            .filter(|(a, b)| {
                let ((ax, ay), (bx, by)) = (**a, **b);
                (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay)
            })
            .count();
        crossings % 2 == 1
    }
}

impl Cell {
    pub fn at(&self, point: (f32, f32)) -> Pos2 {
        egui::pos2(
            self.centre.x + (point.0 - CENTRE.0) / REFERENCE_HALF * self.half,
            self.centre.y + (point.1 - CENTRE.1) / REFERENCE_HALF * self.half,
        )
    }

    pub fn length(&self, length: f32) -> f32 {
        length / REFERENCE_HALF * self.half
    }

    pub fn paint(&self, painter: &egui::Painter, primitive: &Primitive, colour: Color32) {
        let stroke = Stroke::new(self.length(MARK_WIDTH), colour);
        match primitive {
            Primitive::Dot { at, radius } => {
                painter.circle_filled(self.at(*at), self.length(*radius), colour);
            }
            Primitive::Line(points) => {
                let screen: Vec<Pos2> = points.iter().map(|&point| self.at(point)).collect();
                for pair in screen.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
                for &vertex in &screen {
                    painter.circle_filled(vertex, stroke.width / 2.0, stroke.color);
                }
            }
        }
    }

    fn rings_at(&self, rings: &[Ring]) -> Vec<Vec<Pos2>> {
        rings
            .iter()
            .map(|ring| ring.iter().map(|&point| self.at(point)).collect())
            .collect()
    }
}

pub(crate) fn encode(colour: Color) -> [u8; 4] {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(colour.red), byte(colour.green), byte(colour.blue), 255]
}

fn frame(frame: Frame, tier: Tier) -> Vec<Ring> {
    match (frame, tier) {
        (Frame::Unit, _) => vec![polygon(&[(30.0, 4.0), (58.0, 54.0), (2.0, 54.0)])],
        (Frame::Structure, Tier::ONE) => vec![square(6.0, 54.0)],
        (Frame::Structure, Tier::TWO) => vec![square(6.0, 54.0), box_ring(12.0, 48.0, 44.0)],
        (Frame::Structure, _) => vec![
            polygon(&[
                (6.0, 14.0),
                (14.0, 6.0),
                (46.0, 6.0),
                (54.0, 14.0),
                (54.0, 54.0),
                (6.0, 54.0),
            ]),
            polygon(&[
                (12.0, 16.0),
                (16.0, 12.0),
                (44.0, 12.0),
                (48.0, 16.0),
                (48.0, 44.0),
                (12.0, 44.0),
            ]),
            polygon(&[
                (15.0, 17.0),
                (17.0, 15.0),
                (43.0, 15.0),
                (45.0, 17.0),
                (45.0, 41.0),
                (15.0, 41.0),
            ]),
        ],
        (Frame::Defence, _) => vec![polygon(&[
            (6.0, 6.0),
            (54.0, 6.0),
            (54.0, 38.0),
            (52.0, 45.0),
            (46.0, 51.0),
            (38.0, 55.0),
            (30.0, 58.0),
            (22.0, 55.0),
            (14.0, 51.0),
            (8.0, 45.0),
            (6.0, 38.0),
        ])],
    }
}

fn cut(role: Role, material: Option<Material>) -> Vec<Ring> {
    match role {
        Role::Build => vec![plus((30.0, 29.0), 10.0, 3.5)],
        Role::Extract => material.map(material_icon).unwrap_or_default(),
        Role::Store => vec![circle((30.0, 29.0), 11.0), circle((30.0, 29.0), 5.0)],
        Role::ShortFire => vec![circle((30.0, 29.0), 7.0)],
        Role::LongFire => vec![rect(27.0, 14.0, 33.0, 38.0)],
        Role::Scout => vec![polygon(&[
            (30.0, 16.0),
            (38.0, 32.0),
            (32.0, 32.0),
            (32.0, 42.0),
            (28.0, 42.0),
            (28.0, 32.0),
            (22.0, 32.0),
        ])],
        Role::Brawl => vec![circle((30.0, 25.0), 8.0), rect(18.0, 37.0, 42.0, 41.0)],
        Role::Artillery => vec![arch((30.0, 40.0), 12.0, 6.0)],
        Role::Carry => vec![rect(20.0, 32.0, 40.0, 38.0), rect(24.0, 24.0, 36.0, 29.0)],
        Role::Tend => vec![plus((30.0, 26.0), 8.0, 3.0), rect(18.0, 38.0, 42.0, 41.0)],
        Role::Sense => vec![polygon(&[
            (14.0, 16.0),
            (30.0, 28.0),
            (46.0, 16.0),
            (46.0, 44.0),
            (30.0, 32.0),
            (14.0, 44.0),
        ])],
        Role::Shield => vec![circle((30.0, 29.0), 13.0), circle((30.0, 29.0), 7.0)],
        Role::Refine => vec![
            rect(14.0, 16.0, 46.0, 22.0),
            rect(14.0, 26.0, 46.0, 32.0),
            rect(14.0, 36.0, 46.0, 42.0),
        ],
    }
}

fn material_icon(material: Material) -> Vec<Ring> {
    match material {
        Material::Metals => vec![
            polygon(&[
                (30.0, 15.0),
                (42.0, 22.0),
                (42.0, 36.0),
                (30.0, 43.0),
                (18.0, 36.0),
                (18.0, 22.0),
            ]),
            circle((30.0, 29.0), 4.0),
        ],
        Material::Volatiles => vec![polygon(&[
            (30.0, 14.0),
            (34.0, 21.0),
            (37.0, 27.0),
            (39.0, 32.0),
            (39.0, 36.0),
            (37.0, 40.0),
            (34.0, 43.0),
            (30.0, 44.0),
            (26.0, 43.0),
            (23.0, 40.0),
            (21.0, 36.0),
            (21.0, 32.0),
            (23.0, 27.0),
            (26.0, 21.0),
        ])],
        Material::Energy => vec![polygon(&[
            (34.0, 14.0),
            (22.0, 31.0),
            (30.0, 31.0),
            (26.0, 44.0),
            (39.0, 26.0),
            (31.0, 26.0),
        ])],
    }
}

fn notches(tier: Tier) -> Vec<Ring> {
    let xs: &[f32] = match tier {
        Tier::ONE => &[27.0],
        Tier::TWO => &[20.0, 33.0],
        _ => &[14.0, 27.0, 40.0],
    };
    xs.iter()
        .map(|&left| rect(left, 46.0, left + 6.0, 54.0))
        .collect()
}

fn polygon(points: &[(f32, f32)]) -> Ring {
    points.to_vec()
}

fn square(from: f32, to: f32) -> Ring {
    rect(from, from, to, to)
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Ring {
    vec![(left, top), (right, top), (right, bottom), (left, bottom)]
}

fn box_ring(from: f32, to: f32, bottom: f32) -> Ring {
    vec![
        (from, from),
        (to, from),
        (to, bottom),
        (from, bottom),
        (from, from + 3.0),
        (from + 3.0, from + 3.0),
        (from + 3.0, bottom - 3.0),
        (to - 3.0, bottom - 3.0),
        (to - 3.0, from + 3.0),
        (from, from + 3.0),
    ]
}

fn plus(at: (f32, f32), arm: f32, half_width: f32) -> Ring {
    let (x, y, a, w) = (at.0, at.1, arm, half_width);
    vec![
        (x - w, y - a),
        (x + w, y - a),
        (x + w, y - w),
        (x + a, y - w),
        (x + a, y + w),
        (x + w, y + w),
        (x + w, y + a),
        (x - w, y + a),
        (x - w, y + w),
        (x - a, y + w),
        (x - a, y - w),
        (x - w, y - w),
    ]
}

fn circle(at: (f32, f32), radius: f32) -> Ring {
    (0..ROUND_STEPS)
        .map(|step| {
            let angle = TAU * step as f32 / ROUND_STEPS as f32;
            (at.0 + radius * angle.cos(), at.1 + radius * angle.sin())
        })
        .collect()
}

fn arch(base: (f32, f32), outer: f32, inner: f32) -> Ring {
    let half = ROUND_STEPS / 2;
    let along = |radius: f32, step: usize| {
        let angle = TAU / 2.0 + TAU / 2.0 * step as f32 / half as f32;
        (base.0 + radius * angle.cos(), base.1 + radius * angle.sin())
    };
    (0..=half)
        .map(|step| along(outer, step))
        .chain((0..=half).rev().map(|step| along(inner, step)))
        .collect()
}

fn lowered(ring: Ring, by: f32) -> Ring {
    ring.into_iter().map(|(x, y)| (x, y + by)).collect()
}

pub(crate) fn even_odd_mesh(rings: &[Vec<Pos2>], colour: Color32) -> Mesh {
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

#[cfg(test)]
mod tests {
    use neumannarch_sim::pattern::EntityPattern;

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

    fn every_glyph() -> Vec<Glyph> {
        let roles = [
            Role::Build,
            Role::Extract,
            Role::Store,
            Role::ShortFire,
            Role::LongFire,
            Role::Scout,
            Role::Brawl,
            Role::Artillery,
            Role::Carry,
            Role::Tend,
            Role::Sense,
            Role::Shield,
            Role::Refine,
        ];
        let mut every = Vec::new();
        for frame in [Frame::Unit, Frame::Structure, Frame::Defence] {
            for role in roles {
                for tier in [Tier::ONE, Tier::TWO, Tier::THREE] {
                    let materials = match role {
                        Role::Extract => Material::EVERY.map(Some).to_vec(),
                        _ => vec![None],
                    };
                    for material in materials {
                        every.push(Glyph {
                            frame,
                            role,
                            material,
                            tier,
                        });
                    }
                }
            }
        }
        every
    }

    #[test]
    fn every_glyph_is_a_silhouette_with_its_role_cut_out_and_no_two_are_alike() {
        let glyphs = every_glyph();
        let drawings: Vec<Drawing> = glyphs.iter().map(|glyph| Drawing::of(*glyph)).collect();
        for (glyph, drawing) in glyphs.iter().zip(&drawings) {
            assert!(drawing.rings.len() >= 2, "a frame and at least one cut");
            let solid = match glyph.frame {
                Frame::Unit => (6.0, 53.0),
                Frame::Structure | Frame::Defence => (8.0, 30.0),
            };
            assert!(
                drawing.covers(solid.0, solid.1),
                "{glyph:?}'s frame is solid"
            );
        }
        for (at, one) in drawings.iter().enumerate() {
            for other in &drawings[at + 1..] {
                assert_ne!(one, other);
            }
        }
    }

    #[test]
    fn a_cut_is_a_hole_in_the_frame_and_the_notches_cut_the_base() {
        let yard = Drawing::of(EntityPattern::Shipyard.glyph());
        assert!(yard.covers(10.0, 10.0));
        assert!(!yard.covers(30.0, 29.0), "the plus is cut out");
        assert!(
            !yard.covers(30.0, 50.0),
            "the one notch is cut from the base"
        );
        assert!(yard.covers(10.0, 50.0));
    }

    #[test]
    fn a_texture_is_the_silhouette_in_the_colour_and_the_colour_changes_no_shape() {
        let glyph = EntityPattern::Frigate.glyph();
        let opaque = |texture: &TextureData| {
            texture
                .pixels()
                .chunks_exact(4)
                .filter(|texel| texel[3] == 255)
                .count()
        };
        let red = Drawing::of(glyph).texture(Color::rgb(1.0, 0.0, 0.0));
        let blue = Drawing::of(glyph).texture(Color::rgb(0.0, 0.0, 1.0));
        let texels = (CELL_PIXELS * CELL_PIXELS) as usize;
        assert!(opaque(&red) > texels / 5 && opaque(&red) < texels * 4 / 5);
        assert_eq!(opaque(&red), opaque(&blue));
        assert_ne!(red.pixels(), blue.pixels());
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
