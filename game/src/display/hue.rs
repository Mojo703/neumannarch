//! The one colour each material is drawn in, which a starved frame's belt
//! mark takes.

use mirage_engine::egui::Color32;
use probe_sim::Material;

/// Metals: a cold grey-blue.
pub const METALS: Color32 = Color32::from_rgb(150, 170, 200);

/// Volatiles: a green.
pub const VOLATILES: Color32 = Color32::from_rgb(110, 200, 130);

/// Energy: an amber.
pub const ENERGY: Color32 = Color32::from_rgb(230, 180, 70);

/// The colour `material` is drawn in.
pub fn of(material: Material) -> Color32 {
    match material {
        Material::Metals => METALS,
        Material::Volatiles => VOLATILES,
        Material::Energy => ENERGY,
    }
}
