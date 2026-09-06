use std::sync::LazyLock;

use neumannarch_sim::Material;
use usvg::tiny_skia_path::{PathSegment, Point};

use crate::display::glyph::Primitive;

const CURVE_STEPS: usize = 8;

pub const CENTRE: (f32, f32) = (30.0, 30.0);

const SHEET: &str = include_str!("../../icons/materials.svg");

const GROUPS: [&str; 3] = ["metals", "volatiles", "energy"];

static ICONS: LazyLock<[Icon; 3]> = LazyLock::new(|| {
    Material::EVERY.map(|material| {
        Icon::parse(SHEET, GROUPS[material as usize])
            .unwrap_or_else(|why| panic!("the {material:?} icon is not an icon: {why:?}"))
    })
});

#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    rings: Vec<Vec<(f32, f32)>>,
}

#[derive(Debug)]
pub enum NotAnIcon {
    Svg(usvg::Error),
    NoSuchGroup,
    NotOnePath,
    Stroked,
    Unfilled,
    Open,
}

pub fn of(material: Material) -> &'static Icon {
    &ICONS[material as usize]
}

impl Icon {
    pub fn parse(sheet: &str, group: &str) -> Result<Icon, NotAnIcon> {
        let tree =
            usvg::Tree::from_str(sheet, &usvg::Options::default()).map_err(NotAnIcon::Svg)?;
        let Some(usvg::Node::Group(group)) = tree.node_by_id(group) else {
            return Err(NotAnIcon::NoSuchGroup);
        };
        let [usvg::Node::Path(path)] = group.children() else {
            return Err(NotAnIcon::NotOnePath);
        };
        if path.stroke().is_some() {
            return Err(NotAnIcon::Stroked);
        }
        if path.fill().is_none() {
            return Err(NotAnIcon::Unfilled);
        }
        let transform = path.abs_transform();
        let mut rings: Vec<Vec<(f32, f32)>> = Vec::new();
        let mut open: Option<Vec<(f32, f32)>> = None;
        let place = |point: Point| {
            let mut placed = point;
            transform.map_point(&mut placed);
            (placed.x, placed.y)
        };
        for segment in path.data().segments() {
            match segment {
                PathSegment::MoveTo(to) => {
                    if open.is_some() {
                        return Err(NotAnIcon::Open);
                    }
                    open = Some(vec![place(to)]);
                }
                PathSegment::LineTo(to) => {
                    open.get_or_insert_default().push(place(to));
                }
                PathSegment::QuadTo(control, to) => {
                    let ring = open.get_or_insert_default();
                    let from = ring.last().copied().unwrap_or_default();
                    ring.extend(curve(from, &[place(control), place(to)]));
                }
                PathSegment::CubicTo(first, second, to) => {
                    let ring = open.get_or_insert_default();
                    let from = ring.last().copied().unwrap_or_default();
                    ring.extend(curve(from, &[place(first), place(second), place(to)]));
                }
                PathSegment::Close => match open.take() {
                    Some(ring) if ring.len() >= 3 => rings.push(ring),
                    _ => return Err(NotAnIcon::Open),
                },
            }
        }
        if open.is_some() || rings.is_empty() {
            return Err(NotAnIcon::Open);
        }
        Ok(Icon { rings })
    }

    pub fn placed(&self, centre: (f32, f32), width: f32) -> Primitive {
        let xs = self.points().map(|(x, _)| x);
        let ys = self.points().map(|(_, y)| y);
        let (left, right) = (
            xs.clone().fold(f32::INFINITY, f32::min),
            xs.fold(f32::NEG_INFINITY, f32::max),
        );
        let (top, bottom) = (
            ys.clone().fold(f32::INFINITY, f32::min),
            ys.fold(f32::NEG_INFINITY, f32::max),
        );
        let scale = width / (right - left);
        let middle = ((left + right) / 2.0, (top + bottom) / 2.0);
        Primitive::Path(
            self.rings
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|&(x, y)| {
                            (
                                centre.0 + (x - middle.0) * scale,
                                centre.1 + (y - middle.1) * scale,
                            )
                        })
                        .collect()
                })
                .collect(),
        )
    }

    fn points(&self) -> impl Iterator<Item = (f32, f32)> + Clone + '_ {
        self.rings.iter().flatten().copied()
    }
}

fn curve(from: (f32, f32), controls: &[(f32, f32)]) -> impl Iterator<Item = (f32, f32)> + '_ {
    (1..=CURVE_STEPS).map(move |step| {
        let t = step as f32 / CURVE_STEPS as f32;
        let mut points: Vec<(f32, f32)> = core::iter::once(from)
            .chain(controls.iter().copied())
            .collect();
        while points.len() > 1 {
            points = points
                .windows(2)
                .map(|pair| {
                    (
                        pair[0].0 + (pair[1].0 - pair[0].0) * t,
                        pair[0].1 + (pair[1].1 - pair[0].1) * t,
                    )
                })
                .collect();
        }
        points[0]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(primitive: &Primitive) -> ((f32, f32), (f32, f32)) {
        let Primitive::Path(rings) = primitive else {
            panic!("an icon is a path");
        };
        let points = || rings.iter().flatten().copied();
        let fold = |pick: fn((f32, f32)) -> f32, start: f32, by: fn(f32, f32) -> f32| {
            points().map(pick).fold(start, by)
        };
        (
            (
                fold(|(x, _)| x, f32::INFINITY, f32::min),
                fold(|(x, _)| x, f32::NEG_INFINITY, f32::max),
            ),
            (
                fold(|(_, y)| y, f32::INFINITY, f32::min),
                fold(|(_, y)| y, f32::NEG_INFINITY, f32::max),
            ),
        )
    }

    #[test]
    fn every_materials_icon_parses_to_one_closed_filled_path_and_no_two_are_alike() {
        for material in Material::EVERY {
            assert!(!of(material).rings.is_empty(), "{material:?}");
        }
        for (i, a) in Material::EVERY.iter().enumerate() {
            for b in &Material::EVERY[i + 1..] {
                assert_ne!(of(*a), of(*b), "{a:?} and {b:?} share a drawing");
            }
        }
    }

    #[test]
    fn a_nut_has_a_hole_and_a_drop_and_a_bolt_do_not() {
        assert_eq!(of(Material::Metals).rings.len(), 2);
        assert_eq!(of(Material::Volatiles).rings.len(), 1);
        assert_eq!(of(Material::Energy).rings.len(), 1);
    }

    #[test]
    fn a_placed_icon_is_as_wide_as_asked_and_centred_where_asked() {
        for material in Material::EVERY {
            let ((left, right), (top, bottom)) = extent(&of(material).placed((30.0, 32.0), 30.0));
            assert!(
                (right - left - 30.0).abs() < 1e-3,
                "{material:?} is {} wide",
                right - left
            );
            assert!(((left + right) / 2.0 - 30.0).abs() < 1e-3, "{material:?}");
            assert!(((top + bottom) / 2.0 - 32.0).abs() < 1e-3, "{material:?}");
        }
    }

    #[test]
    fn a_drawing_that_is_not_one_closed_filled_path_is_refused() {
        let svg = |body: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 60"><g id="drawn">{body}</g></svg>"#
            )
        };
        let parse = |body: &str| Icon::parse(&svg(body), "drawn");
        assert!(matches!(
            parse(r#"<path d="M0 0 L10 0 L10 10 Z"/><path d="M20 20 L30 20 L30 30 Z"/>"#),
            Err(NotAnIcon::NotOnePath)
        ));
        assert!(matches!(
            parse(r#"<path stroke="red" d="M0 0 L10 0 L10 10 Z"/>"#),
            Err(NotAnIcon::Stroked)
        ));
        assert!(matches!(
            parse(r#"<path fill="none" d="M0 0 L10 0 L10 10 Z"/>"#),
            Err(NotAnIcon::Unfilled)
        ));
        assert!(matches!(
            parse(r#"<path d="M0 0 L10 0 L10 10"/>"#),
            Err(NotAnIcon::Open)
        ));
        assert!(matches!(
            Icon::parse(&svg(r#"<path d="M0 0 L10 0 L10 10 Z"/>"#), "absent"),
            Err(NotAnIcon::NoSuchGroup)
        ));
        assert!(matches!(
            Icon::parse("not svg", "drawn"),
            Err(NotAnIcon::Svg(_))
        ));
        assert!(
            parse(r#"<path d="M0 0 L10 0 L10 10 Z"/><text x="1" y="1">TEMP</text>"#).is_ok(),
            "a word painted across a drawing is not part of it"
        );
    }

    #[test]
    fn a_curve_is_flattened_through_its_end_point() {
        let flat: Vec<(f32, f32)> = curve((0.0, 0.0), &[(10.0, 0.0), (10.0, 10.0)]).collect();
        assert_eq!(flat.len(), CURVE_STEPS);
        assert_eq!(flat[CURVE_STEPS - 1], (10.0, 10.0));
        assert!(flat[0].0 > 0.0 && flat[0].1 > 0.0);
    }
}
