use std::collections::BTreeMap;

use mirage_engine::egui::{self, Pos2};
use probe_sim::roster::Roster;
use probe_sim::{RockId, RowId, SeatId};

use crate::display::ease;
use crate::display::scene::{Hover, Scene, Shown, WheelBand};
use crate::display::viewport::Viewport;
use crate::display::wheel::{Bands, Detail, Footprint, Placed, Sizing, Wheel};

const RESTING_ALPHA: f32 = 0.7;

const HOVER_MARGIN: f32 = 48.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aim {
    pub viewer: SeatId,
    pub pointer: Option<Pos2>,
    pub hovered: Option<RockId>,
    pub step: u32,
    pub wants: BTreeMap<(RockId, RowId), u32>,
}

impl Aim {
    fn bands_at(&self, rock: RockId) -> Bands {
        Bands {
            step: self.step,
            wants: self
                .wants
                .iter()
                .filter(|((at, _), _)| *at == rock)
                .map(|((_, row), want)| (*row, *want))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Eased {
    Scale,
    Alpha,
}

impl Eased {
    fn at_rest(self) -> f32 {
        match self {
            Eased::Scale => Detail::Small.scale(),
            Eased::Alpha => RESTING_ALPHA,
        }
    }
}

pub trait Ease {
    fn ease(&mut self, rock: RockId, eased: Eased, target: f32) -> f32;
}

pub struct Still;

impl Ease for Still {
    fn ease(&mut self, _rock: RockId, _eased: Eased, target: f32) -> f32 {
        target
    }
}

#[derive(Debug, Default)]
pub struct Motion {
    frame: u64,
    dt: f64,
    tweens: BTreeMap<(RockId, Eased), Tween>,
}

#[derive(Clone, Copy, Debug)]
struct Tween {
    value: f64,
    stepped: u64,
}

impl Motion {
    pub fn begin(&mut self, dt: f64) {
        self.frame += 1;
        self.dt = dt;
    }
}

impl Ease for Motion {
    fn ease(&mut self, rock: RockId, eased: Eased, target: f32) -> f32 {
        let frame = self.frame;
        let dt = self.dt;
        let tween = self.tweens.entry((rock, eased)).or_insert(Tween {
            value: f64::from(eased.at_rest()),
            stepped: 0,
        });
        if tween.stepped != frame {
            tween.stepped = frame;
            tween.value = ease::toward(tween.value, f64::from(target), dt);
        }
        tween.value as f32
    }
}

pub struct Wheels {
    wheels: Vec<Wheel>,
    hovered: Option<RockId>,
}

impl Wheels {
    pub fn over(
        scene: &Scene,
        roster: &Roster,
        viewport: &Viewport,
        aim: &Aim,
        ease: &mut impl Ease,
    ) -> Wheels {
        let footprints = footprints(scene, roster, viewport, aim.viewer);
        let resting = laid(&footprints, None, scene.selection);
        let hovered = hovered(&footprints, &resting, aim);
        let placed = match hovered {
            None => resting,
            Some(_) => laid(&footprints, hovered, scene.selection),
        };
        let mut wheels: Vec<Wheel> = placed
            .iter()
            .filter_map(|placed| {
                let view = scene.wheel_of(placed.rock)?;
                let bands =
                    (placed.sizing.detail == Detail::Full).then(|| aim.bands_at(placed.rock));
                let wheel = Wheel::over(placed.eased(ease), view, roster, aim.viewer, bands);
                wheel.draws().then_some(wheel)
            })
            .collect();
        wheels.sort_by(|a, b| b.alpha().total_cmp(&a.alpha()));
        Wheels { wheels, hovered }
    }

    pub fn hovered(&self) -> Option<RockId> {
        self.hovered
    }

    pub fn iter(&self) -> impl Iterator<Item = &Wheel> {
        self.wheels.iter()
    }

    pub fn at(&self, at: Pos2) -> Option<RockId> {
        self.wheels
            .iter()
            .filter(|wheel| wheel.holds(at))
            .min_by(|a, b| a.centre().distance(at).total_cmp(&b.centre().distance(at)))
            .map(Wheel::rock)
    }

    pub fn band_at(&self, at: Pos2) -> Option<(RockId, RowId, WheelBand)> {
        self.wheels.iter().find_map(|wheel| {
            wheel
                .band_at(at)
                .map(|(row, band)| (wheel.rock(), row, band))
        })
    }

    pub fn spoken_at(&self, at: Pos2) -> Option<(Pos2, RowId, Option<Shown>)> {
        self.wheels.iter().find_map(|wheel| wheel.spoken_at(at))
    }

    pub fn paint(&self, painter: &egui::Painter, hover: Option<&Hover>) {
        for wheel in self.wheels.iter().rev() {
            wheel.paint(painter, band_of(hover, wheel.rock()));
        }
    }
}

impl Placed {
    fn eased(self, ease: &mut impl Ease) -> Placed {
        Placed {
            sizing: Sizing {
                detail: self.sizing.detail,
                scale: ease.ease(self.rock, Eased::Scale, self.sizing.scale),
            },
            alpha: ease.ease(self.rock, Eased::Alpha, self.alpha),
            ..self
        }
    }
}

fn band_of(hover: Option<&Hover>, rock: RockId) -> Option<(RowId, WheelBand)> {
    match hover {
        Some(Hover::Wheel {
            rock: over,
            row,
            band,
        }) if *over == rock => Some((*row, *band)),
        _ => None,
    }
}

fn footprints(
    scene: &Scene,
    roster: &Roster,
    viewport: &Viewport,
    viewer: SeatId,
) -> Vec<Footprint> {
    let rocks: BTreeMap<RockId, Pos2> = scene
        .rocks
        .iter()
        .filter_map(|rock| Some((rock.id, viewport.point_of(rock.pos)?)))
        .collect();
    scene
        .wheels
        .iter()
        .filter_map(|wheel| {
            Some(Footprint::of(
                wheel.rock,
                *rocks.get(&wheel.rock)?,
                wheel,
                roster,
                viewer,
            ))
        })
        .collect()
}

fn laid(
    footprints: &[Footprint],
    hovered: Option<RockId>,
    selected: Option<RockId>,
) -> Vec<Placed> {
    let whole = hovered.or(selected);
    footprints
        .iter()
        .map(|footprint| {
            let rock = Some(footprint.rock);
            let detail = match rock == hovered || rock == selected {
                true => Detail::Full,
                false => Detail::Small,
            };
            Placed {
                rock: footprint.rock,
                centre: footprint.centre,
                sizing: Sizing::settled(detail),
                alpha: match rock == whole {
                    true => 1.0,
                    false => RESTING_ALPHA,
                },
            }
        })
        .collect()
}

fn hovered(footprints: &[Footprint], resting: &[Placed], aim: &Aim) -> Option<RockId> {
    let pointer = aim.pointer?;
    let extent = |placed: &Placed, detail: Detail| {
        footprints
            .iter()
            .find(|footprint| footprint.rock == placed.rock)
            .map(|footprint| footprint.at(detail))
    };
    let held = aim.hovered.filter(|rock| {
        resting.iter().any(|placed| {
            placed.rock == *rock
                && extent(placed, Detail::Full)
                    .is_some_and(|extent| extent.expand(HOVER_MARGIN).contains(pointer))
        })
    });
    held.or_else(|| {
        resting
            .iter()
            .filter_map(|placed| Some((placed, extent(placed, placed.sizing.detail)?)))
            .filter(|(_, extent)| extent.contains(pointer))
            .min_by(|(a, a_extent), (b, b_extent)| {
                a_extent.area().total_cmp(&b_extent.area()).then(
                    a.centre
                        .distance(pointer)
                        .total_cmp(&b.centre.distance(pointer)),
                )
            })
            .map(|(placed, _)| placed.rock)
    })
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::FRIGATE;

    use super::*;
    use crate::display::scene::{Entry, RowView, SectorView, WheelView};

    const A: RockId = RockId(0);

    const B: RockId = RockId(1);

    fn view(rock: RockId) -> WheelView {
        WheelView {
            rock,
            sectors: vec![SectorView {
                seat: SeatId(0),
                rows: vec![RowView {
                    row: FRIGATE,
                    entries: vec![Shown {
                        entry: Entry::Present(1),
                        previewed: false,
                    }],
                }],
                arc: None,
            }],
        }
    }

    fn footprints(apart: f32) -> Vec<Footprint> {
        let roster = Roster::shipped();
        [(A, egui::pos2(0.0, 0.0)), (B, egui::pos2(apart, 0.0))]
            .into_iter()
            .map(|(rock, centre)| Footprint::of(rock, centre, &view(rock), &roster, SeatId(0)))
            .collect()
    }

    fn aim(pointer: Pos2, hovered: Option<RockId>) -> Aim {
        Aim {
            viewer: SeatId(0),
            pointer: Some(pointer),
            hovered,
            step: 1,
            wants: BTreeMap::new(),
        }
    }

    fn detail(placed: &[Placed], rock: RockId) -> Option<Detail> {
        placed
            .iter()
            .find(|placed| placed.rock == rock)
            .map(|placed| placed.sizing.detail)
    }

    fn alpha(placed: &[Placed], rock: RockId) -> Option<f32> {
        placed
            .iter()
            .find(|placed| placed.rock == rock)
            .map(|placed| placed.alpha)
    }

    fn placed(apart: f32, hovered: Option<RockId>, selected: Option<RockId>) -> Vec<Placed> {
        laid(&footprints(apart), hovered, selected)
    }

    fn far() -> f32 {
        4.0 * footprints(0.0)[0].at(Detail::Full).width()
    }

    #[test]
    fn a_wheel_is_full_where_the_pointer_or_the_selection_rests_and_small_elsewhere() {
        let none = placed(far(), None, None);
        assert_eq!(detail(&none, A), Some(Detail::Small));
        assert_eq!(detail(&none, B), Some(Detail::Small));

        let selected = placed(far(), None, Some(B));
        assert_eq!(detail(&selected, A), Some(Detail::Small));
        assert_eq!(detail(&selected, B), Some(Detail::Full));

        let hovered = placed(far(), Some(A), None);
        assert_eq!(detail(&hovered, A), Some(Detail::Full));
        assert_eq!(detail(&hovered, B), Some(Detail::Small));

        let both = placed(far(), Some(A), Some(B));
        assert_eq!(detail(&both, A), Some(Detail::Full));
        assert_eq!(
            detail(&both, B),
            Some(Detail::Full),
            "two may be full at once"
        );
    }

    #[test]
    fn the_wheel_the_pointer_is_at_is_whole_and_every_other_faint() {
        let none = placed(far(), None, None);
        assert_eq!(alpha(&none, A), Some(RESTING_ALPHA));
        assert_eq!(alpha(&none, B), Some(RESTING_ALPHA));

        let selected = placed(far(), None, Some(B));
        assert_eq!(alpha(&selected, A), Some(RESTING_ALPHA));
        assert_eq!(
            alpha(&selected, B),
            Some(1.0),
            "with nothing hovered the selected is whole"
        );

        let both = placed(far(), Some(A), Some(B));
        assert_eq!(alpha(&both, A), Some(1.0), "the hovered is whole");
        assert_eq!(
            alpha(&both, B),
            Some(RESTING_ALPHA),
            "and the selected faint at full size"
        );

        let covering = placed(1.0, None, Some(A));
        assert_eq!(
            alpha(&covering, B),
            Some(RESTING_ALPHA),
            "a covered wheel is no fainter than one at rest"
        );
    }

    #[test]
    fn a_hovered_wheel_stays_hovered_where_it_grows_under_the_pointer() {
        let footprints = footprints(far());
        let resting = laid(&footprints, None, None);
        let small = footprints[0].at(Detail::Small);
        let full = footprints[0].at(Detail::Full);
        let outside_small = egui::pos2(small.right() + 2.0, 0.0);
        assert!(full.contains(outside_small), "but inside the full one");

        assert_eq!(
            hovered(&footprints, &resting, &aim(outside_small, None)),
            None,
            "hovering is decided against the wheels as they stand"
        );
        assert_eq!(
            hovered(&footprints, &resting, &aim(outside_small, Some(A))),
            Some(A),
            "and a hovered wheel keeps the pointer while it grows"
        );
        assert_eq!(
            hovered(
                &footprints,
                &resting,
                &aim(egui::pos2(full.right() + HOVER_MARGIN - 1.0, 0.0), Some(A))
            ),
            Some(A),
            "an overshoot past its edge keeps it"
        );
        assert_eq!(
            hovered(
                &footprints,
                &resting,
                &aim(egui::pos2(full.right() + HOVER_MARGIN + 1.0, 0.0), Some(A))
            ),
            None,
            "until the pointer leaves its full extent by the margin"
        );
    }

    #[test]
    fn a_wheel_grows_from_rest_and_shrinks_back_over_the_same_span() {
        let mut motion = Motion::default();
        let frame = |motion: &mut Motion, dt: f64, target: f32| {
            motion.begin(dt);
            let first = motion.ease(A, Eased::Scale, target);
            assert_eq!(
                motion.ease(A, Eased::Scale, target),
                first,
                "a second read in one frame steps nothing"
            );
            first
        };
        let small = Detail::Small.scale();

        let born = frame(&mut motion, ease::SPAN_SECONDS / 2.0, 1.0);
        assert!(
            (born - (small + 1.0) / 2.0).abs() < 1e-3,
            "{born}: a wheel first drawn full grows from small"
        );
        assert_eq!(frame(&mut motion, ease::SPAN_SECONDS, 1.0), 1.0);
        assert_eq!(frame(&mut motion, 1.0, 1.0), 1.0, "and holds");
        let back = frame(&mut motion, ease::SPAN_SECONDS / 2.0, small);
        assert!(
            (back - (small + 1.0) / 2.0).abs() < 1e-3,
            "{back}: it shrinks by the same span"
        );
        assert_eq!(frame(&mut motion, ease::SPAN_SECONDS, small), small);
    }

    #[test]
    fn a_small_wheel_under_the_pointer_is_hovered_though_the_full_one_reaches_it() {
        let apart = footprints(0.0)[0].at(Detail::Full).width() / 2.0;
        let footprints = footprints(apart);
        let resting = laid(&footprints, None, Some(A));
        let inside_b = footprints[1].at(Detail::Small).center();
        assert!(
            footprints[0].at(Detail::Full).contains(inside_b),
            "the full wheel reaches over the small one"
        );

        assert_eq!(
            hovered(&footprints, &resting, &aim(inside_b, None)),
            Some(B)
        );
    }

    #[test]
    fn the_pointer_over_a_wheel_hovers_the_nearest_one() {
        let footprints = footprints(far());
        let resting = laid(&footprints, None, None);

        assert_eq!(
            hovered(&footprints, &resting, &aim(egui::pos2(10.0, 0.0), None)),
            Some(A)
        );
        assert_eq!(
            hovered(
                &footprints,
                &resting,
                &aim(egui::pos2(far() + 10.0, 0.0), None)
            ),
            Some(B)
        );
    }
}
