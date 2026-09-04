use mirage_engine::egui::Color32;
use neumannarch_sim::Material;

pub const METALS: Color32 = Color32::from_rgb(150, 170, 200);

pub const VOLATILES: Color32 = Color32::from_rgb(110, 200, 130);

pub const ENERGY: Color32 = Color32::from_rgb(230, 180, 70);

pub fn of(material: Material) -> Color32 {
    match material {
        Material::Metals => METALS,
        Material::Volatiles => VOLATILES,
        Material::Energy => ENERGY,
    }
}
