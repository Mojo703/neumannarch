use neumannarch_sim::roster::{Kind, Row, Weapon};

pub const HALF: f32 = 11.0;

pub const SMALL_BELOW: f64 = 50.0;

pub const LARGE_FROM: f64 = 150.0;

pub const LONG_RANGE_FROM_METERS: f64 = 10.0;

const SIZE_SCALE: [f32; 3] = [0.85, 1.0, 1.2];

pub const WIDEST_SCALE: f32 = SIZE_SCALE[2];

const CENTRE: (f32, f32) = (30.0, 30.0);

const REFERENCE_HALF: f32 = 22.0;

pub const OUTLINE_WIDTH: f32 = 2.0;

pub const MARK_WIDTH: f32 = 4.5;

pub fn unit(point: (f32, f32)) -> (f32, f32) {
    (
        (point.0 - CENTRE.0) / REFERENCE_HALF,
        (point.1 - CENTRE.1) / REFERENCE_HALF,
    )
}

pub fn unit_length(length: f32) -> f32 {
    length / REFERENCE_HALF
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Frame {
    Triangle,
    Square,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Glyph {
    pub frame: Frame,
    pub marks: Vec<GlyphMark>,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GlyphMark {
    Dot,
    Bar,
    Plus,
    Chevron,
    Belt,
    Ring,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Debug)]
pub enum Primitive {
    Dot { at: (f32, f32), radius: f32 },
    Line(Vec<(f32, f32)>),
    Ring { at: (f32, f32), radius: f32 },
}

impl Frame {
    fn of(kind: Kind) -> Frame {
        match kind {
            Kind::Unit => Frame::Triangle,
            Kind::Structure => Frame::Square,
        }
    }

    pub fn points(&self) -> &'static [(f32, f32)] {
        match self {
            Frame::Triangle => &[(30.0, 6.0), (56.0, 52.0), (4.0, 52.0)],
            Frame::Square => &[(8.0, 8.0), (52.0, 8.0), (52.0, 52.0), (8.0, 52.0)],
        }
    }
}

impl Glyph {
    pub fn of(row: &Row) -> Glyph {
        Glyph {
            frame: Frame::of(row.kind()),
            marks: marks_of(row),
            size: Size::of(row.cost.total()),
        }
    }
}

fn marks_of(row: &Row) -> Vec<GlyphMark> {
    let mut marks: Vec<GlyphMark> = row.weapons.iter().map(GlyphMark::of_weapon).collect();
    if row.plating.0 > 0.0 {
        marks.push(GlyphMark::Belt);
    }
    if row.capacity.total() > 0.0 {
        marks.push(GlyphMark::Ring);
    }
    marks
}

impl GlyphMark {
    fn of_weapon(weapon: &Weapon) -> GlyphMark {
        match weapon {
            Weapon::Damage { range, .. } if range.0 < LONG_RANGE_FROM_METERS => GlyphMark::Dot,
            Weapon::Damage { .. } => GlyphMark::Bar,
            Weapon::Build { .. } => GlyphMark::Plus,
            Weapon::Extract { .. } => GlyphMark::Chevron,
        }
    }
}

pub fn primitives_of(marks: &[GlyphMark]) -> Vec<Primitive> {
    let paired = marks.contains(&GlyphMark::Ring) && marks.contains(&GlyphMark::Plus);
    let belted = marks.contains(&GlyphMark::Belt);
    marks
        .iter()
        .flat_map(|mark| primitives_of_mark(mark, paired, belted))
        .collect()
}

fn primitives_of_mark(mark: &GlyphMark, paired: bool, belted: bool) -> Vec<Primitive> {
    match mark {
        GlyphMark::Dot if belted => vec![Primitive::Dot {
            at: (30.0, 29.0),
            radius: 7.0,
        }],
        GlyphMark::Dot => vec![Primitive::Dot {
            at: (30.0, 34.0),
            radius: 8.0,
        }],
        GlyphMark::Bar => vec![Primitive::Line(vec![(30.0, 14.0), (30.0, 48.0)])],
        GlyphMark::Plus if paired => plus_lines((30.0, 30.0), 6.5),
        GlyphMark::Plus => plus_lines((30.0, 38.0), 8.5),
        GlyphMark::Chevron => vec![chevron_line((30.0, 32.0), 15.0)],
        GlyphMark::Belt => vec![Primitive::Line(vec![(17.0, 45.0), (43.0, 45.0)])],
        GlyphMark::Ring => vec![Primitive::Ring {
            at: (30.0, 30.0),
            radius: if paired { 15.0 } else { 13.0 },
        }],
    }
}

fn plus_lines(at: (f32, f32), arm: f32) -> Vec<Primitive> {
    vec![
        Primitive::Line(vec![(at.0 - arm, at.1), (at.0 + arm, at.1)]),
        Primitive::Line(vec![(at.0, at.1 - arm), (at.0, at.1 + arm)]),
    ]
}

fn chevron_line(at: (f32, f32), a: f32) -> Primitive {
    Primitive::Line(vec![
        (at.0 - a, at.1 - a * 0.7),
        (at.0, at.1 + a * 0.7),
        (at.0 + a, at.1 - a * 0.7),
    ])
}

impl Size {
    fn of(cost: f64) -> Size {
        if cost < SMALL_BELOW {
            Size::Small
        } else if cost >= LARGE_FROM {
            Size::Large
        } else {
            Size::Medium
        }
    }

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
    use neumannarch_sim::roster::{Roster, Weights};
    use neumannarch_sim::{Materials, Real};

    use super::*;

    const SHIPPED_MARKS: [(&str, &[GlyphMark]); 7] = [
        ("constructor", &[GlyphMark::Plus]),
        ("extractor", &[GlyphMark::Chevron]),
        ("storage", &[GlyphMark::Ring]),
        ("shipyard", &[GlyphMark::Plus, GlyphMark::Ring]),
        ("raider", &[GlyphMark::Dot]),
        ("frigate", &[GlyphMark::Dot, GlyphMark::Belt]),
        ("lancer", &[GlyphMark::Bar]),
    ];

    fn row(cost: f64, manoeuvring: f64, weapons: Vec<Weapon>) -> Row {
        Row {
            name: "test",
            cost: Materials::new(cost, 0.0, 0.0),
            manoeuvring: Real(manoeuvring),
            steering: Weights::STILL,
            hp: Real(1.0),
            plating: Real(0.0),
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
        assert_eq!(Glyph::of(&row(60.0, 0.0, vec![])).frame, Frame::Square);
        assert_eq!(Glyph::of(&row(60.0, 1.0, vec![])).frame, Frame::Triangle);
    }

    #[test]
    fn damage_marks_a_dot_below_the_long_range_threshold_and_a_bar_from_it() {
        let marks = |range| Glyph::of(&row(60.0, 1.0, vec![damage(range)])).marks;
        assert_eq!(marks(LONG_RANGE_FROM_METERS - 1.0), vec![GlyphMark::Dot]);
        assert_eq!(marks(LONG_RANGE_FROM_METERS), vec![GlyphMark::Bar]);
        assert_eq!(marks(LONG_RANGE_FROM_METERS + 1.0), vec![GlyphMark::Bar]);
    }

    #[test]
    fn build_marks_a_plus_and_extract_a_chevron_in_weapon_order() {
        let weapons = vec![
            Weapon::Build { rate: Real(1.0) },
            Weapon::Extract { rate: Real(1.0) },
        ];
        assert_eq!(
            Glyph::of(&row(60.0, 0.0, weapons)).marks,
            vec![GlyphMark::Plus, GlyphMark::Chevron]
        );
    }

    #[test]
    fn plating_above_zero_earns_the_belt_mark() {
        let mut plated = row(60.0, 1.0, vec![]);
        plated.plating = Real(1.0);
        assert_eq!(Glyph::of(&plated).marks, vec![GlyphMark::Belt]);
    }

    #[test]
    fn capacity_above_zero_earns_the_ring_mark_alongside_a_build_plus() {
        let mut stores = row(60.0, 0.0, vec![]);
        stores.capacity = Materials::new(1.0, 0.0, 0.0);
        assert_eq!(Glyph::of(&stores).marks, vec![GlyphMark::Ring]);

        let mut yards = row(60.0, 0.0, vec![Weapon::Build { rate: Real(1.0) }]);
        yards.capacity = Materials::new(1.0, 0.0, 0.0);
        assert_eq!(
            Glyph::of(&yards).marks,
            vec![GlyphMark::Plus, GlyphMark::Ring]
        );
    }

    #[test]
    fn size_steps_at_the_thresholds() {
        let size = |cost| Glyph::of(&row(cost, 1.0, vec![])).size;
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
    fn a_dot_beside_a_belt_sits_higher_and_smaller_than_alone() {
        let alone = primitives_of(&[GlyphMark::Dot]);
        let beside = primitives_of(&[GlyphMark::Dot, GlyphMark::Belt]);
        let dot_at = |primitives: &[Primitive]| {
            primitives
                .iter()
                .find_map(|primitive| match primitive {
                    Primitive::Dot { at, radius } => Some((*at, *radius)),
                    _ => None,
                })
                .expect("a dot")
        };
        assert_eq!(dot_at(&alone), ((30.0, 34.0), 8.0));
        assert_eq!(dot_at(&beside), ((30.0, 29.0), 7.0));
    }

    #[test]
    fn a_ring_around_a_plus_is_wider_than_alone_and_the_plus_shares_its_centre() {
        let alone = primitives_of(&[GlyphMark::Ring]);
        let paired = primitives_of(&[GlyphMark::Ring, GlyphMark::Plus]);
        let ring_radius = |primitives: &[Primitive]| {
            primitives
                .iter()
                .find_map(|primitive| match primitive {
                    Primitive::Ring { radius, .. } => Some(*radius),
                    _ => None,
                })
                .expect("a ring")
        };
        assert_eq!(ring_radius(&alone), 13.0);
        assert_eq!(ring_radius(&paired), 15.0);

        let plus_arm = paired
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Line(points) if points.len() == 2 => Some(points[0]),
                _ => None,
            })
            .any(|(x, y)| (x, y) == (30.0 - 6.5, 30.0));
        assert!(plus_arm, "the paired plus does not share the ring's centre");
    }
}
