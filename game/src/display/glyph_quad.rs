use mirage_engine::egui::Color32;
use mirage_engine::mesh::{Mesh, MeshData, Quad};
use mirage_engine::{Assets, Catalog, Color, Material, TextureData};
use neumannarch_sim::roster::{Glyph, Roster};
use neumannarch_sim::{MAX_SEATS, SeatId};

use crate::display::glyph::{Drawing, encode};

const PALETTE: [Color; MAX_SEATS] = [
    Color::rgb(0.90, 0.25, 0.25),
    Color::rgb(0.25, 0.55, 0.95),
    Color::rgb(0.30, 0.80, 0.35),
    Color::rgb(0.95, 0.80, 0.20),
];

pub fn seat_colour(seat: SeatId) -> Color {
    PALETTE[usize::from(seat.0)]
}

pub fn seat_color32(seat: SeatId) -> Color32 {
    let [red, green, blue, alpha] = encode(seat_colour(seat));
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

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
                    glyph: row.glyph(),
                    seat: SeatId(seat),
                })
            })
            .collect()
    }
}

impl Mesh for GlyphQuad {
    fn build(&self, assets: &Assets) -> MeshData {
        Quad.build(assets)
            .with_texture(rasterize(self.glyph, seat_colour(self.seat)))
            .with_material(Material::lit(Color::WHITE).cutout())
    }
}

pub fn rasterize(glyph: Glyph, colour: Color) -> TextureData {
    Drawing::of(glyph).texture(colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque(texture: &TextureData) -> usize {
        texture
            .pixels()
            .chunks_exact(4)
            .filter(|texel| texel[3] == 255)
            .count()
    }

    #[test]
    fn the_catalog_holds_one_cell_per_shipped_row_and_seat() {
        let cells = GlyphQuad::catalog();
        assert_eq!(cells.len(), MAX_SEATS * Roster::shipped().iter().count());
        let mut keys: Vec<(Glyph, SeatId)> = cells.iter().map(|it| (it.glyph, it.seat)).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), cells.len(), "no cell twice");
    }

    #[test]
    fn a_rows_cell_is_its_silhouette_in_the_seats_colour_with_its_role_cut_out() {
        let roster = Roster::shipped();
        let texels =
            (crate::display::glyph::CELL_PIXELS * crate::display::glyph::CELL_PIXELS) as usize;
        let mut seen = Vec::new();
        for (_, row) in roster.iter() {
            let cell = rasterize(row.glyph(), seat_colour(SeatId(0)));
            let filled = opaque(&cell);
            assert!(
                filled > texels / 5 && filled < texels * 4 / 5,
                "{} fills {filled} of {texels} texels",
                row.name
            );
            assert!(
                !seen.contains(&cell.pixels().to_vec()),
                "{} repeats a cell",
                row.name
            );
            seen.push(cell.pixels().to_vec());
        }
        let red = rasterize(
            roster[neumannarch_sim::roster::FRIGATE].glyph(),
            seat_colour(SeatId(0)),
        );
        let blue = rasterize(
            roster[neumannarch_sim::roster::FRIGATE].glyph(),
            seat_colour(SeatId(1)),
        );
        assert_eq!(opaque(&red), opaque(&blue), "the colour changes no shape");
        assert_ne!(red.pixels(), blue.pixels());
    }
}
