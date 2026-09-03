//! The glyph of a row: DISPLAY.md's three rules as one pure function.

use probe_sim::roster::{Kind, Row, Weapon};

/// A glyph's nominal half-width, in points, before its size class scales
/// it: on a ring, on the wheel, and on a ship's billboard alike, so the
/// two layers agree in size.
pub const HALF: f32 = 7.0;

/// Cost total, in cost units, below which a glyph is `Size::Small`.
pub const SMALL_BELOW: f64 = 50.0;

/// Cost total, in cost units, from which a glyph is `Size::Large`.
pub const LARGE_FROM: f64 = 150.0;

/// A glyph's scale by its cost class, over whatever size a drawn glyph's
/// own medium class takes: `0.75` for `Small`, `1.0` for `Medium`, `1.3`
/// for `Large`.
const SIZE_SCALE: [f32; 3] = [0.75, 1.0, 1.3];

/// The widest a glyph is drawn, as a scale over its nominal half-width:
/// [`Size::Large`]'s step. A run is laid at this width, so a glyph of any
/// size class stands clear of its neighbours.
pub const WIDEST_SCALE: f32 = SIZE_SCALE[2];

/// The outline: the row's kind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Frame {
    /// A unit.
    Triangle,
    /// A structure.
    Square,
}

/// A row's glyph: every glyph is drawn from one of these, none by hand.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Glyph {
    pub frame: Frame,
    /// One per weapon, in the row's weapon order.
    pub marks: Vec<GlyphMark>,
    pub size: Size,
}

/// A mark inside the frame: one weapon.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GlyphMark {
    /// Damage with range within the row's sight.
    Dot,
    /// Damage with range beyond the row's sight.
    Bar,
    /// Build.
    Plus,
    /// Extract.
    Chevron,
}

/// The cost class: below `SMALL_BELOW` is small, from `LARGE_FROM` is
/// large, between is medium.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Small,
    Medium,
    Large,
}

impl Frame {
    fn of(kind: Kind) -> Frame {
        match kind {
            Kind::Unit => Frame::Triangle,
            Kind::Structure => Frame::Square,
        }
    }
}

impl Glyph {
    /// The glyph of `row` by the three rules.
    pub fn of(row: &Row) -> Glyph {
        Glyph {
            frame: Frame::of(row.kind()),
            marks: row
                .weapons
                .iter()
                .map(|weapon| GlyphMark::of(weapon, row.sight.0))
                .collect(),
            size: Size::of(row.cost.total()),
        }
    }
}

impl GlyphMark {
    /// `sight` is the row's, in meters; a damage range at most `sight` is
    /// within it.
    fn of(weapon: &Weapon, sight: f64) -> GlyphMark {
        match weapon {
            Weapon::Damage { range, .. } if range.0 <= sight => GlyphMark::Dot,
            Weapon::Damage { .. } => GlyphMark::Bar,
            Weapon::Build { .. } => GlyphMark::Plus,
            Weapon::Extract { .. } => GlyphMark::Chevron,
        }
    }
}

impl Size {
    /// `cost` is the row's cost total, in cost units.
    fn of(cost: f64) -> Size {
        if cost < SMALL_BELOW {
            Size::Small
        } else if cost >= LARGE_FROM {
            Size::Large
        } else {
            Size::Medium
        }
    }

    /// This size's scale over a drawn glyph's medium size; see
    /// [`SIZE_SCALE`].
    pub fn scale(&self) -> f32 {
        SIZE_SCALE[match self {
            Size::Small => 0,
            Size::Medium => 1,
            Size::Large => 2,
        }]
    }
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::Roster;
    use probe_sim::{Materials, Real};

    use super::*;

    /// A row of `cost` metals, seeing `sight` meters, with `accel` and
    /// `weapons`; the rest is what no rule reads.
    fn row(cost: f64, accel: f64, sight: f64, weapons: Vec<Weapon>) -> Row {
        Row {
            name: "test",
            cost: Materials::new(cost, 0.0, 0.0),
            mass: Real(1.0),
            accel: Real(accel),
            maneuver: Real(accel),
            hp: Real(1.0),
            plating: Real(0.0),
            sight: Real(sight),
            radar: Real(sight),
            capacity: Materials::ZERO,
            weapons,
        }
    }

    fn damage(range: f64) -> Weapon {
        Weapon::Damage {
            range: Real(range),
            rate: Real(1.0),
            damage: Real(1.0),
            falloff: Real(0.0),
        }
    }

    #[test]
    fn a_structure_is_a_square_and_a_unit_a_triangle() {
        assert_eq!(Glyph::of(&row(60.0, 0.0, 5.0, vec![])).frame, Frame::Square);
        assert_eq!(
            Glyph::of(&row(60.0, 1.0, 5.0, vec![])).frame,
            Frame::Triangle
        );
    }

    #[test]
    fn damage_marks_a_dot_within_sight_and_a_bar_beyond() {
        let sight = 8.0;
        let marks = |range| Glyph::of(&row(60.0, 1.0, sight, vec![damage(range)])).marks;
        assert_eq!(marks(sight - 1.0), vec![GlyphMark::Dot]);
        assert_eq!(marks(sight), vec![GlyphMark::Dot]);
        assert_eq!(marks(sight + 1.0), vec![GlyphMark::Bar]);
    }

    #[test]
    fn build_marks_a_plus_and_extract_a_chevron_in_weapon_order() {
        let weapons = vec![
            Weapon::Build { rate: Real(1.0) },
            Weapon::Extract { rate: Real(1.0) },
        ];
        assert_eq!(
            Glyph::of(&row(60.0, 0.0, 5.0, weapons)).marks,
            vec![GlyphMark::Plus, GlyphMark::Chevron]
        );
    }

    #[test]
    fn size_steps_at_the_thresholds() {
        let size = |cost| Glyph::of(&row(cost, 1.0, 5.0, vec![])).size;
        assert_eq!(size(SMALL_BELOW - 1.0), Size::Small);
        assert_eq!(size(SMALL_BELOW), Size::Medium);
        assert_eq!(size(LARGE_FROM - 1.0), Size::Medium);
        assert_eq!(size(LARGE_FROM), Size::Large);
    }

    #[test]
    fn every_shipped_row_has_its_own_glyph() {
        let roster = Roster::shipped();
        let glyphs: Vec<Glyph> = roster.iter().map(|(_, row)| Glyph::of(row)).collect();
        assert!(!glyphs.is_empty());
        for (i, a) in glyphs.iter().enumerate() {
            for b in &glyphs[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
