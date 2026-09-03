//! The glyph of a row: DISPLAY.md's rules for frame, marks and size as one
//! pure function.

use probe_sim::roster::{Kind, Row, Weapon};

/// A glyph's nominal half-width, in points, before its size class scales
/// it: on a ring, on the wheel, and on a ship's billboard alike, so the
/// two layers agree in size.
pub const HALF: f32 = 11.0;

/// Cost total, in cost units, below which a glyph is `Size::Small`.
pub const SMALL_BELOW: f64 = 50.0;

/// Cost total, in cost units, from which a glyph is `Size::Large`.
pub const LARGE_FROM: f64 = 150.0;

/// Radar strictly above this multiple of sight earns the arc mark.
pub const RADAR_ABOVE_DEFAULT: f64 = 2.0;

/// A glyph's scale by its cost class, over whatever size a drawn glyph's
/// own medium class takes: `0.85` for `Small`, `1.0` for `Medium`, `1.2`
/// for `Large`. Compressed toward `1.0` from a wider spread so `Small`
/// still leaves a mark room to read.
const SIZE_SCALE: [f32; 3] = [0.85, 1.0, 1.2];

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
    /// The row's earned marks, in the order [`marks_of`] states.
    pub marks: Vec<GlyphMark>,
    pub size: Size,
}

/// A mark inside or on the frame; each variant's place on the frame is
/// fixed, never chosen by the caller. See [`GlyphMark::anchor`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GlyphMark {
    /// Damage with range within the row's sight: a dot at the incircle's
    /// top.
    Dot,
    /// Damage with range beyond the row's sight: a bar from the incircle's
    /// top to the base.
    Bar,
    /// Build: a plus at the incircle's centre.
    Plus,
    /// Extract: a chevron pointing down, touching the base.
    Chevron,
    /// Radar above [`RADAR_ABOVE_DEFAULT`] times sight: an arc over the
    /// apex.
    Arc,
    /// Plating above zero: a belt along the base.
    Belt,
    /// Capacity above zero: a hollow ring at the incircle's centre.
    Ring,
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

    /// This frame's inscribed circle, in its own frame: x right, y down,
    /// half-width one; `(centre, radius)`, the centre on the frame's own
    /// axis. A square's incircle is the square itself, so its bottom sits
    /// where the square's does.
    pub fn incircle(&self) -> (f32, f32) {
        match self {
            Frame::Square => (0.0, 1.0),
            // The incentre and inradius of the upward triangle apex
            // (0, -1), base corners (±1, 1): sides 2, sqrt(5), sqrt(5).
            Frame::Triangle => {
                let leg = 5.0_f32.sqrt();
                ((leg - 1.0) / (leg + 1.0), (leg - 1.0) / 2.0)
            }
        }
    }
}

impl Glyph {
    /// The glyph of `row` by DISPLAY.md's rules.
    pub fn of(row: &Row) -> Glyph {
        Glyph {
            frame: Frame::of(row.kind()),
            marks: marks_of(row),
            size: Size::of(row.cost.total()),
        }
    }
}

/// `row`'s earned marks: one per weapon in weapon order, then the arc, the
/// belt and the ring where the row's fields earn them.
fn marks_of(row: &Row) -> Vec<GlyphMark> {
    let mut marks: Vec<GlyphMark> = row
        .weapons
        .iter()
        .map(|weapon| GlyphMark::of_weapon(weapon, row.sight.0))
        .collect();
    if row.radar.0 > RADAR_ABOVE_DEFAULT * row.sight.0 {
        marks.push(GlyphMark::Arc);
    }
    if row.plating.0 > 0.0 {
        marks.push(GlyphMark::Belt);
    }
    if row.capacity.total() > 0.0 {
        marks.push(GlyphMark::Ring);
    }
    marks
}

impl GlyphMark {
    /// `sight` is the row's, in meters; a damage range at most `sight` is
    /// within it.
    fn of_weapon(weapon: &Weapon, sight: f64) -> GlyphMark {
        match weapon {
            Weapon::Damage { range, .. } if range.0 <= sight => GlyphMark::Dot,
            Weapon::Damage { .. } => GlyphMark::Bar,
            Weapon::Build { .. } => GlyphMark::Plus,
            Weapon::Extract { .. } => GlyphMark::Chevron,
        }
    }

    /// This mark's anchor on `frame`, in the frame's own space: x right, y
    /// down, half-width one, origin at the frame's bounding-box centre.
    /// Every anchor but the arc's sits on `frame`'s own incircle, where the
    /// frame has room for it; the arc alone stays above the apex.
    pub fn anchor(&self, frame: &Frame) -> (f32, f32) {
        let (centre, radius) = frame.incircle();
        match self {
            GlyphMark::Dot | GlyphMark::Bar => (0.0, centre - radius),
            GlyphMark::Plus | GlyphMark::Ring => (0.0, centre),
            GlyphMark::Chevron | GlyphMark::Belt => (0.0, centre + radius),
            GlyphMark::Arc => (0.0, -1.0),
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

/// A mark's geometry, as fractions of the glyph's own half-width alone,
/// never of the frame's width at the mark's anchor, shared by the screen
/// painter and the texture rasterizer so a mark places and sizes
/// identically in both.
pub mod geometry {
    pub const DOT_RADIUS: f32 = 0.22;
    pub const BAR_HALF_WIDTH: f32 = 0.16;
    pub const PLUS_ARM: f32 = 0.34;
    pub const PLUS_THICKNESS: f32 = 0.14;
    pub const CHEVRON_HALF_WIDTH: f32 = 0.32;
    pub const CHEVRON_HEIGHT: f32 = 0.30;
    pub const CHEVRON_THICKNESS: f32 = 0.12;
    pub const ARC_RADIUS: f32 = 0.30;
    pub const ARC_THICKNESS: f32 = 0.13;
    pub const ARC_HALF_ANGLE: f32 = 0.9;
    pub const BELT_HALF_WIDTH: f32 = 0.7;
    pub const BELT_THICKNESS: f32 = 0.12;
    pub const RING_RADIUS: f32 = 0.32;
    pub const RING_THICKNESS: f32 = 0.13;

    /// The least a mark's stroke or thickness is ever painted at, in
    /// points, so it does not vanish at the small size class.
    pub const MIN_STROKE: f32 = 2.0;

    /// The least a dot's radius is ever painted at, in points.
    pub const MIN_DOT_RADIUS: f32 = 3.0;
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::Roster;
    use probe_sim::{Materials, Real};

    use super::*;

    /// `(name, marks)` for every shipped row, from DISPLAY.md's rules read
    /// off `sim/src/roster/shipped.rs`'s table.
    const SHIPPED_MARKS: [(&str, &[GlyphMark]); 8] = [
        ("constructor", &[GlyphMark::Plus]),
        ("extractor", &[GlyphMark::Chevron]),
        ("storage", &[GlyphMark::Ring]),
        ("shipyard", &[GlyphMark::Plus, GlyphMark::Ring]),
        ("scout", &[GlyphMark::Arc]),
        ("raider", &[GlyphMark::Dot]),
        ("frigate", &[GlyphMark::Dot, GlyphMark::Belt]),
        ("lancer", &[GlyphMark::Bar]),
    ];

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
    fn radar_above_twice_sight_earns_the_arc_mark() {
        let mut plain = row(60.0, 1.0, 5.0, vec![]);
        plain.radar = Real(10.0);
        assert_eq!(Glyph::of(&plain).marks, vec![]);
        let mut radared = row(60.0, 1.0, 5.0, vec![]);
        radared.radar = Real(10.01);
        assert_eq!(Glyph::of(&radared).marks, vec![GlyphMark::Arc]);
    }

    #[test]
    fn plating_above_zero_earns_the_belt_mark() {
        let mut plated = row(60.0, 1.0, 5.0, vec![]);
        plated.plating = Real(1.0);
        assert_eq!(Glyph::of(&plated).marks, vec![GlyphMark::Belt]);
    }

    #[test]
    fn capacity_above_zero_earns_the_ring_mark_alongside_a_build_plus() {
        let mut stores = row(60.0, 0.0, 5.0, vec![]);
        stores.capacity = Materials::new(1.0, 0.0, 0.0);
        assert_eq!(Glyph::of(&stores).marks, vec![GlyphMark::Ring]);

        let mut yards = row(60.0, 0.0, 5.0, vec![Weapon::Build { rate: Real(1.0) }]);
        yards.capacity = Materials::new(1.0, 0.0, 0.0);
        assert_eq!(
            Glyph::of(&yards).marks,
            vec![GlyphMark::Plus, GlyphMark::Ring]
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
    fn every_shipped_rows_marks_are_exactly_what_its_fields_earn() {
        let roster = Roster::shipped();
        for (name, marks) in SHIPPED_MARKS {
            let row = roster
                .iter()
                .map(|(_, row)| row)
                .find(|row| row.name == name);
            let row = row.unwrap_or_else(|| panic!("no shipped row named {name}"));
            assert_eq!(Glyph::of(row).marks, marks.to_vec(), "{name}");
        }
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

    #[test]
    fn every_mark_but_the_arc_sits_off_the_triangles_apex() {
        for mark in [
            GlyphMark::Dot,
            GlyphMark::Bar,
            GlyphMark::Plus,
            GlyphMark::Chevron,
            GlyphMark::Belt,
            GlyphMark::Ring,
        ] {
            let (_, y) = mark.anchor(&Frame::Triangle);
            assert!(y > -1.0, "{mark:?} sits at the triangle's narrowest point");
        }
        assert_eq!(GlyphMark::Arc.anchor(&Frame::Triangle), (0.0, -1.0));
    }
}
