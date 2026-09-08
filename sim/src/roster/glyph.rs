use crate::materials::Material;
use crate::roster::row::{Kind, Row, Weapon};

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

    pub const fn new(tier: u8) -> Option<Tier> {
        match tier {
            1..=3 => Some(Tier(tier)),
            _ => None,
        }
    }
}

impl Row {
    pub fn glyph(&self) -> Glyph {
        Glyph {
            frame: match (self.kind(), self.is_armed()) {
                (Kind::Unit, _) => Frame::Unit,
                (Kind::Structure, false) => Frame::Structure,
                (Kind::Structure, true) => Frame::Defence,
            },
            role: self.role,
            material: self.weapons.iter().find_map(|weapon| match weapon {
                Weapon::Extract { material, .. } => Some(*material),
                _ => None,
            }),
            tier: self.tier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::{METALS_EXTRACTOR, RAIDER, Roster, SHIPYARD};

    #[test]
    fn a_rows_glyph_is_its_kind_its_role_its_extract_material_and_its_tier() {
        let roster = Roster::shipped();
        let glyph = |row| roster[row].glyph();
        assert_eq!(
            glyph(SHIPYARD),
            Glyph {
                frame: Frame::Structure,
                role: Role::Build,
                material: None,
                tier: Tier::ONE
            }
        );
        assert_eq!(glyph(METALS_EXTRACTOR).material, Some(Material::Metals));
        assert_eq!(glyph(RAIDER).frame, Frame::Unit);
        assert_eq!(Tier::new(4), None);
    }
}
