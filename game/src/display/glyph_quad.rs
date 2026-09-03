//! The ship glyph: a mesh value per `(Glyph, SeatId)`, its own texture
//! rasterized in the seat's colour with a white outline, for the belt.

use mirage_engine::egui::Color32;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::{Mesh, MeshData, Quad};
use mirage_engine::{Assets, Catalog, Color, Material, TextureData};
use probe_sim::roster::Roster;
use probe_sim::{MAX_SEATS, SeatId};

use crate::display::glyph::{Frame, Glyph, GlyphMark, Size, geometry};

/// A glyph texture's side, in texels.
pub const CELL_PIXELS: u32 = 48;

/// The band counted as outline, as a fraction of the shape's own scale.
const OUTLINE_FRACTION: f32 = 0.18;

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
    let side = CELL_PIXELS;
    let mut pixels = vec![0u8; 4 * (side * side) as usize];
    for row in 0..side {
        for col in 0..side {
            let (x, y) = to_local(col, row, side);
            let rgba = match coverage(glyph, x, y) {
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

/// What point `(x, y)` of `glyph`'s cell paints.
fn coverage(glyph: &Glyph, x: f32, y: f32) -> Coverage {
    // Shrinks the point toward the centre before testing the unit shape, so
    // a smaller size class draws a smaller frame within the same cell.
    let extent = cell_fraction(&glyph.size);
    let (x, y) = (x / extent, y / extent);
    // Checked before the frame itself: the arc mark reads over the apex,
    // outside the frame's own silhouette.
    if marks_cover(&glyph.marks, &glyph.frame, x, y) {
        return Coverage::Mark;
    }
    let depth = frame_depth(&glyph.frame, x, y);
    if depth < 0.0 {
        Coverage::Outside
    } else if depth < OUTLINE_FRACTION {
        Coverage::Outline
    } else {
        Coverage::Fill
    }
}

/// The distance from `(x, y)` to the nearest edge of `frame`, positive
/// inside, negative outside.
fn frame_depth(frame: &Frame, x: f32, y: f32) -> f32 {
    match frame {
        Frame::Square => 1.0 - x.abs().max(y.abs()),
        // An upward triangle, apex at (0, -1), base at y = 1: the distance
        // to whichever of its three edges is nearest.
        Frame::Triangle => {
            let left = x + (y + 1.0) / 2.0;
            let right = (y + 1.0) / 2.0 - x;
            let base = 1.0 - y;
            left.min(right).min(base)
        }
    }
}

/// Whether any of `marks`, each at its own place on `frame`, covers `(x, y)`.
fn marks_cover(marks: &[GlyphMark], frame: &Frame, x: f32, y: f32) -> bool {
    marks.iter().any(|mark| {
        let (ax, ay) = mark.anchor(frame);
        mark_covers(mark, frame, x - ax, y - ay)
    })
}

/// Whether `mark`, anchored at the origin, covers the point `(x, y)` offset
/// from its anchor.
fn mark_covers(mark: &GlyphMark, frame: &Frame, x: f32, y: f32) -> bool {
    match mark {
        GlyphMark::Dot => x.hypot(y) <= geometry::DOT_RADIUS,
        // From the anchor, the incircle's top, to the base, its own
        // diameter further down.
        GlyphMark::Bar => {
            let height = 2.0 * frame.incircle().1;
            x.abs() <= geometry::BAR_HALF_WIDTH && (0.0..=height).contains(&y)
        }
        GlyphMark::Plus => {
            (x.abs() <= geometry::PLUS_THICKNESS && y.abs() <= geometry::PLUS_ARM)
                || (x.abs() <= geometry::PLUS_ARM && y.abs() <= geometry::PLUS_THICKNESS)
        }
        GlyphMark::Chevron => {
            let (span, rise) = (geometry::CHEVRON_HALF_WIDTH, geometry::CHEVRON_HEIGHT);
            let near_upper = distance_to_segment((x, y), (-span, -rise), (0.0, 0.0));
            let near_lower = distance_to_segment((x, y), (0.0, 0.0), (span, -rise));
            near_upper.min(near_lower) <= geometry::CHEVRON_THICKNESS
        }
        GlyphMark::Arc => {
            let half_angle = geometry::ARC_HALF_ANGLE;
            let angle = x.atan2(-y);
            let on_ring = (x.hypot(y) - geometry::ARC_RADIUS).abs() <= geometry::ARC_THICKNESS;
            on_ring && angle.abs() <= half_angle
        }
        GlyphMark::Belt => {
            x.abs() <= geometry::BELT_HALF_WIDTH && y.abs() <= geometry::BELT_THICKNESS
        }
        GlyphMark::Ring => (x.hypot(y) - geometry::RING_RADIUS).abs() <= geometry::RING_THICKNESS,
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

    /// The least texels a row's marks must change in its cell at the small
    /// size class, so a mark reads there and not only at medium or large.
    const MIN_MARK_TEXELS: usize = 20;

    fn glyphs() -> Vec<Glyph> {
        Roster::shipped()
            .iter()
            .map(|(_, row)| Glyph::of(row))
            .collect()
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
}
