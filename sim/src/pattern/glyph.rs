use crate::materials::Material;
use crate::pattern::{EntityPattern, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Glyph {
    pub frame: Frame,
    pub role: Role,
    pub material: Option<Material>,
    pub tier: Tier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Frame {
    Unit,
    Structure,
    Defence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Build,
    Extract,
    Store,
    ShortFire,
    LongFire,
    Scout,
    Brawl,
    Artillery,
    Carry,
    Tend,
    Sense,
    Shield,
    Refine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tier(u8);

impl Tier {
    pub const ONE: Tier = Tier(1);

    pub const TWO: Tier = Tier(2);

    pub const THREE: Tier = Tier(3);
}

impl EntityPattern {
    pub fn glyph(self) -> Glyph {
        Glyph {
            frame: match (self.kind(), self.does_damage()) {
                (Kind::Unit, _) => Frame::Unit,
                (Kind::Structure, false) => Frame::Structure,
                (Kind::Structure, true) => Frame::Defence,
            },
            role: self.role(),
            material: self
                .extractions()
                .next()
                .map(|extraction| extraction.material),
            tier: self.tier(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patterns_glyph_is_its_kind_its_role_its_extract_material_and_its_tier() {
        assert_eq!(
            EntityPattern::Shipyard.glyph(),
            Glyph {
                frame: Frame::Structure,
                role: Role::Build,
                material: None,
                tier: Tier::ONE
            }
        );
        assert_eq!(
            EntityPattern::Extractor(Material::Metals).glyph().material,
            Some(Material::Metals)
        );
        assert_eq!(EntityPattern::Raider.glyph().frame, Frame::Unit);
    }
}
