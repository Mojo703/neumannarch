//! The ship glyph: a mesh value per `(Glyph, SeatId)`, its own texture
//! rasterized in the seat's colour with a white outline, for the belt.
//! Frame and marks are the sheet's own primitives (`glyph::Frame::points`,
//! `glyph::primitives_of`), the same table the screen painter reads.

use mirage_engine::egui::Color32;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::{Mesh, MeshData, Quad};
use mirage_engine::{Assets, Catalog, Color, Material, TextureData};
use probe_sim::roster::Roster;
use probe_sim::{MAX_SEATS, SeatId};

use crate::display::glyph::{self, Frame, Glyph, Primitive, Size};

/// A glyph texture's side, in texels.
pub const CELL_PIXELS: u32 = 48;

/// The seat palette: one colour per seat a match can hold, indexed by
/// [`SeatId`].
const PALETTE: [Color; MAX_SEATS] = [
    Color::rgb(0.90, 0.25, 0.25),
    Color::rgb(0.25, 0.55, 0.95),
    Color::rgb(0.30, 0.80, 0.35),
    Color::rgb(0.95, 0.80, 0.20),
];

/// A glyph's fraction of the cell its frame fills by cost class, `Large`
/// reaching the cell's own edge; the size class over
/// [`crate::display::glyph::WIDEST_SCALE`].
fn cell_fraction(size: &Size) -> f32 {
    size.scale() / crate::display::glyph::WIDEST_SCALE
}

/// `seat`'s colour. Every seat of a match has one, since the palette is
/// [`MAX_SEATS`] long.
pub fn seat_colour(seat: SeatId) -> Color {
    PALETTE[usize::from(seat.0)]
}

/// `seat`'s colour as the HUD's painter takes it, fully opaque.
pub fn seat_color32(seat: SeatId) -> Color32 {
    let [red, green, blue, alpha] = encode(seat_colour(seat));
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

/// A ship's billboarded quad: `glyph`, of the row it stands for, rasterized
/// in `seat`'s colour with a white outline. Equal values build the same
/// texture, so the shipped roster's glyphs across the seat palette are the
/// whole catalog.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphQuad {
    pub glyph: Glyph,
    pub seat: SeatId,
}

impl Catalog for GlyphQuad {
    fn catalog() -> Vec<Self> {
        let roster = Roster::shipped();
        (0..MAX_SEATS as u8)
            .flat_map(|seat| {
                roster.iter().map(move |(_, row)| GlyphQuad {
                    glyph: Glyph::of(row),
                    seat: SeatId(seat),
                })
            })
            .collect()
    }
}

impl Mesh for GlyphQuad {
    fn build(&self, assets: &Assets) -> MeshData {
        Quad.build(assets)
            .with_texture(rasterize(&self.glyph, seat_colour(self.seat)))
            .with_material(Material::lit(Color::WHITE).cutout())
    }
}

/// A pixel's coverage: what `rasterize` paints there.
enum Coverage {
    Outside,
    Outline,
    Fill,
    Mark,
}

/// `glyph` rasterized at [`CELL_PIXELS`] square: the frame filled in
/// `colour` with a white outline and white marks, nearest-sampled.
pub fn rasterize(glyph: &Glyph, colour: Color) -> TextureData {
    rasterize_primitives(
        &glyph.frame,
        &glyph::primitives_of(&glyph.marks),
        &glyph.size,
        colour,
    )
}

/// `frame` and `primitives`, at `size`'s class, rasterized at
/// [`CELL_PIXELS`] square exactly as [`rasterize`] would from the glyph
/// they belong to; the seam a test drives with the sheet's own primitives
/// to check they agree.
fn rasterize_primitives(
    frame: &Frame,
    primitives: &[Primitive],
    size: &Size,
    colour: Color,
) -> TextureData {
    let side = CELL_PIXELS;
    let mut pixels = vec![0u8; 4 * (side * side) as usize];
    for row in 0..side {
        for col in 0..side {
            let (x, y) = to_local(col, row, side);
            let rgba = match coverage(frame, primitives, size, x, y) {
                Coverage::Outside => [0, 0, 0, 0],
                Coverage::Outline | Coverage::Mark => encode(Color::WHITE),
                Coverage::Fill => encode(colour),
            };
            let at = 4 * (row * side + col) as usize;
            pixels[at..at + 4].copy_from_slice(&rgba);
        }
    }
    TextureData::rgba8(UVec2::new(side, side), pixels).pixelated()
}

/// `col`, `row` of a `side`-square grid as `(x, y)` in `-1.0..=1.0`, `y`
/// increasing downward with the texture's own rows.
fn to_local(col: u32, row: u32, side: u32) -> (f32, f32) {
    let to = |value: u32| ((value as f32 + 0.5) / side as f32) * 2.0 - 1.0;
    (to(col), to(row))
}

/// What point `(x, y)` of the cell paints, for `frame` and `primitives` at
/// `size`'s class.
fn coverage(frame: &Frame, primitives: &[Primitive], size: &Size, x: f32, y: f32) -> Coverage {
    // Shrinks the point toward the centre before testing the unit shape, so
    // a smaller size class draws a smaller frame within the same cell.
    let extent = cell_fraction(size);
    let (x, y) = (x / extent, y / extent);
    // Checked before the frame itself: the arc mark reads over the apex,
    // outside the frame's own silhouette.
    if primitives
        .iter()
        .any(|primitive| primitive_covers(primitive, x, y))
    {
        return Coverage::Mark;
    }
    let depth = frame_depth(frame, x, y);
    if depth < 0.0 {
        Coverage::Outside
    } else if depth < glyph::unit_length(glyph::OUTLINE_WIDTH) {
        Coverage::Outline
    } else {
        Coverage::Fill
    }
}

/// The distance from `(x, y)` to the nearest edge of `frame`'s outline,
/// positive inside, negative outside.
fn frame_depth(frame: &Frame, x: f32, y: f32) -> f32 {
    let points: Vec<(f32, f32)> = frame
        .points()
        .iter()
        .map(|&point| glyph::unit(point))
        .collect();
    polygon_depth(&points, x, y)
}

/// The distance from `(x, y)` to the nearest edge of the closed convex
/// polygon `points`, listed apex or corner first as [`Frame::points`]
/// does; positive inside, negative outside.
fn polygon_depth(points: &[(f32, f32)], x: f32, y: f32) -> f32 {
    let count = points.len();
    (0..count)
        .map(|index| {
            let (ax, ay) = points[index];
            let (bx, by) = points[(index + 1) % count];
            let (edge_x, edge_y) = (bx - ax, by - ay);
            let (rel_x, rel_y) = (x - ax, y - ay);
            let cross = edge_x * rel_y - edge_y * rel_x;
            cross / edge_x.hypot(edge_y)
        })
        .fold(f32::INFINITY, f32::min)
}

/// Whether `primitive` covers the point `(x, y)`.
fn primitive_covers(primitive: &Primitive, x: f32, y: f32) -> bool {
    let half_stroke = glyph::unit_length(glyph::MARK_WIDTH) / 2.0;
    match primitive {
        Primitive::Dot { at, radius } => {
            let (ax, ay) = glyph::unit(*at);
            (x - ax).hypot(y - ay) <= glyph::unit_length(*radius)
        }
        Primitive::Line(points) => {
            let unit_points: Vec<(f32, f32)> =
                points.iter().map(|&point| glyph::unit(point)).collect();
            unit_points
                .windows(2)
                .any(|pair| distance_to_segment((x, y), pair[0], pair[1]) <= half_stroke)
        }
        Primitive::Ring { at, radius } => {
            let (ax, ay) = glyph::unit(*at);
            ((x - ax).hypot(y - ay) - glyph::unit_length(*radius)).abs() <= half_stroke
        }
        Primitive::Arc { at, radius } => {
            let (ax, ay) = glyph::unit(*at);
            let (dx, dy) = (x - ax, y - ay);
            let on_ring = (dx.hypot(dy) - glyph::unit_length(*radius)).abs() <= half_stroke;
            // `0` is straight up, matching the screen painter's own arc.
            on_ring && dx.atan2(-dy).abs() <= core::f32::consts::FRAC_PI_2
        }
    }
}

/// The distance from `point` to the segment from `a` to `b`.
fn distance_to_segment(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = point;
    let (ax, ay) = a;
    let (bx, by) = b;
    let (dx, dy) = (bx - ax, by - ay);
    let span = dx * dx + dy * dy;
    let t = if span > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    (px - cx).hypot(py - cy)
}

/// `colour`'s channels as sRGB-encoded bytes, fully opaque.
pub(crate) fn encode(colour: Color) -> [u8; 4] {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(colour.red), byte(colour.green), byte(colour.blue), 255]
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::Roster;

    use super::*;

    /// The most texels the rasterised cell of a shipped row may differ
    /// from the sheet's own drawing of it, at the same size: zero, since
    /// both read the identical, deterministic primitive table.
    const MAX_SHEET_DIFFERENCE: usize = 0;

    /// The least texels a row's marks must change in its cell at the small
    /// size class, so a mark reads there and not only at medium or large.
    const MIN_MARK_TEXELS: usize = 20;

    /// The least texels a bare frame (no marks) must fill in its own
    /// colour, so the frame's own coverage — not only its marks — is
    /// checked, independent of whatever `frame_depth` computes.
    const MIN_FILL_TEXELS: usize = 100;

    /// The least white texels a bare frame (no marks) must carry as its
    /// outline, by the same independent check.
    const MIN_OUTLINE_TEXELS: usize = 20;

    fn glyphs() -> Vec<Glyph> {
        Roster::shipped()
            .iter()
            .map(|(_, row)| Glyph::of(row))
            .collect()
    }

    /// `name`'s primitives transcribed straight from
    /// `art/concepts/hull-gallery.html`'s `icon` table, independent of
    /// `glyph::primitives_of`, so the sheet's-drawing test is not
    /// circular.
    fn sheet_primitives(name: &str) -> Vec<Primitive> {
        let plus = |at: (f32, f32), arm: f32| {
            vec![
                Primitive::Line(vec![(at.0 - arm, at.1), (at.0 + arm, at.1)]),
                Primitive::Line(vec![(at.0, at.1 - arm), (at.0, at.1 + arm)]),
            ]
        };
        match name {
            "constructor" => plus((30.0, 38.0), 8.5),
            "extractor" => vec![Primitive::Line(vec![
                (30.0 - 15.0, 32.0 - 15.0 * 0.7),
                (30.0, 32.0 + 15.0 * 0.7),
                (30.0 + 15.0, 32.0 - 15.0 * 0.7),
            ])],
            "storage" => vec![Primitive::Ring {
                at: (30.0, 30.0),
                radius: 13.0,
            }],
            "shipyard" => {
                let mut primitives = vec![Primitive::Ring {
                    at: (30.0, 30.0),
                    radius: 15.0,
                }];
                primitives.extend(plus((30.0, 30.0), 6.5));
                primitives
            }
            "scout" => vec![
                Primitive::Arc {
                    at: (30.0, 42.0),
                    radius: 11.0,
                },
                Primitive::Dot {
                    at: (30.0, 43.0),
                    radius: 3.5,
                },
            ],
            "raider" => vec![Primitive::Dot {
                at: (30.0, 34.0),
                radius: 8.0,
            }],
            "frigate" => vec![
                Primitive::Dot {
                    at: (30.0, 29.0),
                    radius: 7.0,
                },
                Primitive::Line(vec![(17.0, 45.0), (43.0, 45.0)]),
            ],
            "lancer" => vec![Primitive::Line(vec![(30.0, 14.0), (30.0, 48.0)])],
            _ => panic!("no sheet drawing for {name}"),
        }
    }

    #[test]
    fn every_shipped_row_rasterizes_to_distinct_pixels() {
        let textures: Vec<TextureData> = glyphs()
            .iter()
            .map(|glyph| rasterize(glyph, seat_colour(SeatId(0))))
            .collect();

        for (i, a) in textures.iter().enumerate() {
            for b in &textures[i + 1..] {
                assert_ne!(a.pixels(), b.pixels());
            }
        }
    }

    #[test]
    fn a_rasterized_glyph_has_a_white_outline_pixel() {
        for glyph in glyphs() {
            let texture = rasterize(&glyph, Color::rgb(0.1, 0.2, 0.9));
            let has_white = texture
                .pixels()
                .chunks_exact(4)
                .any(|texel| texel == [255, 255, 255, 255]);
            assert!(has_white, "{glyph:?} has no white outline pixel");
        }
    }

    /// A bare frame (no marks, so every white texel is the outline, not a
    /// mark) rasterises to a filled body in its own colour and a white
    /// outline around it — checked by counting the two colours in the
    /// output directly, never by calling `frame_depth` or `polygon_depth`.
    #[test]
    fn every_shipped_rows_bare_frame_fills_and_outlines() {
        let colour = Color::rgb(0.1, 0.2, 0.9);
        let fill_texel = encode(colour);
        for glyph in glyphs() {
            let bare = Glyph {
                marks: Vec::new(),
                ..glyph
            };
            let pixels = rasterize(&bare, colour);
            let pixels = pixels.pixels();
            let fill = pixels
                .chunks_exact(4)
                .filter(|texel| *texel == fill_texel)
                .count();
            let outline = pixels
                .chunks_exact(4)
                .filter(|texel| *texel == [255, 255, 255, 255])
                .count();
            assert!(
                fill >= MIN_FILL_TEXELS,
                "{:?}'s bare frame filled only {fill} texels",
                bare.frame
            );
            assert!(
                outline >= MIN_OUTLINE_TEXELS,
                "{:?}'s bare frame outlined only {outline} texels",
                bare.frame
            );
        }
    }

    #[test]
    fn a_marked_rows_small_cell_reads_its_marks() {
        for (_, row) in Roster::shipped().iter() {
            let marked = Glyph::of(row);
            if marked.marks.is_empty() {
                continue;
            }
            let small = |glyph: Glyph| Glyph {
                size: Size::Small,
                ..glyph
            };
            let bare = small(Glyph {
                marks: Vec::new(),
                ..marked.clone()
            });
            let marked = small(marked);

            let with_marks = rasterize(&marked, seat_colour(SeatId(0)));
            let bare = rasterize(&bare, seat_colour(SeatId(0)));
            let changed = with_marks
                .pixels()
                .chunks_exact(4)
                .zip(bare.pixels().chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                changed >= MIN_MARK_TEXELS,
                "{row}'s small cell changed only {changed} texels for its marks",
                row = row.name,
            );
        }
    }

    #[test]
    fn every_shipped_rows_rasterised_cell_matches_the_sheets_drawing() {
        for (_, row) in Roster::shipped().iter() {
            let glyph = Glyph::of(row);
            let sheet = sheet_primitives(row.name);
            for size in [Size::Small, Size::Medium, Size::Large] {
                let ours = rasterize_primitives(
                    &glyph.frame,
                    &glyph::primitives_of(&glyph.marks),
                    &size,
                    seat_colour(SeatId(0)),
                );
                let theirs =
                    rasterize_primitives(&glyph.frame, &sheet, &size, seat_colour(SeatId(0)));
                let diff = ours
                    .pixels()
                    .chunks_exact(4)
                    .zip(theirs.pixels().chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(
                    diff == MAX_SHEET_DIFFERENCE,
                    "{}'s {size:?} cell differs from the sheet's drawing by {diff} texels",
                    row.name,
                );
            }
        }
    }
}
