use std::collections::BTreeMap;

use mirage_engine::egui::{self, Pos2, Rect};
use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::State;
use neumannarch_sim::state::view::View;
use neumannarch_sim::{AsteroidId, Material, Posting, RowId, SeatId};

use crate::display::bars::Bars;
use crate::display::ease::{self, Span};
use crate::display::label::{self, titled};
use crate::display::scene::{ButtonAt, Scene, Shown, WheelButton, WheelGesture};
use crate::display::viewport::Viewport;
use crate::display::wheel::{ButtonRefusals, Buttons, Detail, Footprint, Placed, Sizing, Wheel};

pub const RESTING_ALPHA: f32 = 0.7;

const HOVER_MARGIN: f32 = 48.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Aim<'a> {
    pub viewer: SeatId,
    pub pointer: Option<Pos2>,
    pub hovered: Option<AsteroidId>,
    pub step: u32,
    pub view: &'a View,
    pub state: &'a State,
}

impl Aim<'_> {
    fn buttons_at(&self, asteroid: AsteroidId, roster: &Roster) -> Buttons {
        let mut wants = BTreeMap::new();
        let mut refusals = BTreeMap::new();
        for (row, _) in roster.iter() {
            let posting = Posting::of(asteroid, self.viewer, row);
            let want = self.view.want_of(posting);
            let refused = |count: u32| self.state.admits_want(posting, count).err();
            wants.insert(row, want);
            refusals.insert(
                row,
                ButtonRefusals {
                    adding: refused(WheelButton::Plus(self.step).wanted(want)),
                    removing: refused(WheelButton::Minus(self.step).wanted(want)),
                },
            );
        }
        Buttons {
            step: self.step,
            wants,
            refusals,
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
    fn ease(&mut self, asteroid: AsteroidId, eased: Eased, target: f32) -> f32;
}

pub struct Still;

impl Ease for Still {
    fn ease(&mut self, _asteroid: AsteroidId, _eased: Eased, target: f32) -> f32 {
        target
    }
}

#[derive(Debug, Default)]
pub struct Motion {
    frame: u64,
    dt: f64,
    tweens: BTreeMap<(AsteroidId, Eased), Tween>,
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
    fn ease(&mut self, asteroid: AsteroidId, eased: Eased, target: f32) -> f32 {
        let frame = self.frame;
        let dt = self.dt;
        let tween = self.tweens.entry((asteroid, eased)).or_insert(Tween {
            value: f64::from(eased.at_rest()),
            stepped: 0,
        });
        if tween.stepped != frame {
            tween.stepped = frame;
            tween.value = ease::toward(tween.value, f64::from(target), dt, Span::Fast);
        }
        tween.value as f32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Spoken {
    Row {
        row: RowId,
        shown: Option<Shown>,
    },
    Bar {
        material: Material,
        pull: f64,
        cap: f64,
    },
    Refused {
        why: String,
    },
}

impl Spoken {
    pub fn phrase(&self, roster: &Roster) -> String {
        match self {
            Spoken::Row { row, shown } => {
                let name = titled(roster[*row].name);
                match shown {
                    Some(shown) => shown.entry.phrase(&name),
                    None => name,
                }
            }
            Spoken::Refused { why } => why.clone(),
            Spoken::Bar {
                material,
                pull,
                cap,
            } => format!(
                "{} {} of {}",
                label::material(*material),
                pull.round() as u64,
                cap.round() as u64
            ),
        }
    }
}

pub struct Wheels {
    wheels: Vec<Wheel>,
    bars: Bars,
    hovered: Option<AsteroidId>,
}

impl Wheels {
    pub fn over(
        scene: &Scene,
        roster: &Roster,
        viewport: &Viewport,
        aim: &Aim<'_>,
        ease: &mut impl Ease,
    ) -> Wheels {
        let footprints = footprints(scene, roster, viewport, aim.viewer);
        let resting = laid(&footprints, None, scene.selection);
        let hovered = hovered(&footprints, &resting, aim);
        let placed: Vec<Placed> = match hovered {
            None => resting,
            Some(_) => laid(&footprints, hovered, scene.selection),
        }
        .into_iter()
        .map(|placed| placed.eased(ease))
        .collect();
        let mut wheels: Vec<Wheel> = placed
            .iter()
            .filter_map(|placed| {
                let view = scene.wheel_of(placed.asteroid)?;
                let buttons = (placed.sizing.detail == Detail::Full)
                    .then(|| aim.buttons_at(placed.asteroid, roster));
                let wheel = Wheel::over(*placed, view, roster, aim.viewer, buttons);
                wheel.draws().then_some(wheel)
            })
            .collect();
        wheels.sort_by(|a, b| b.alpha().total_cmp(&a.alpha()));
        Wheels {
            wheels,
            bars: Bars::over(scene, viewport, &placed),
            hovered,
        }
    }

    pub fn clear_of(mut self, rect: Rect) -> Wheels {
        for wheel in &mut self.wheels {
            wheel.clear_of(rect);
        }
        self.wheels.retain(Wheel::draws);
        self.bars.clear_of(rect);
        self
    }

    pub fn frames(&self) -> impl Iterator<Item = Rect> + '_ {
        self.wheels
            .iter()
            .flat_map(Wheel::frames)
            .chain(self.bars.frames())
    }

    pub fn hovered(&self) -> Option<AsteroidId> {
        self.hovered
    }

    pub fn iter(&self) -> impl Iterator<Item = &Wheel> {
        self.wheels.iter()
    }

    pub fn at(&self, at: Pos2) -> Option<AsteroidId> {
        self.wheels
            .iter()
            .filter(|wheel| wheel.holds(at))
            .min_by(|a, b| a.centre().distance(at).total_cmp(&b.centre().distance(at)))
            .map(Wheel::asteroid)
    }

    pub fn button_at(&self, at: Pos2) -> Option<ButtonAt> {
        self.wheels.iter().find_map(|wheel| wheel.button_at(at))
    }

    pub fn spoken_at(&self, at: Pos2) -> Option<(Pos2, Spoken)> {
        self.wheels
            .iter()
            .find_map(|wheel| wheel.spoken_at(at))
            .or_else(|| {
                self.bars
                    .spoken_at(at)
                    .map(|(beside, material, pull, cap)| {
                        (
                            beside,
                            Spoken::Bar {
                                material,
                                pull,
                                cap,
                            },
                        )
                    })
            })
    }

    pub fn paint(&self, painter: &egui::Painter, gesture: Option<&WheelGesture>) {
        self.bars.paint(painter);
        for wheel in self.wheels.iter().rev() {
            wheel.paint(painter, button_of(gesture, wheel.asteroid()));
        }
    }
}

impl Placed {
    pub fn resting(asteroid: AsteroidId, centre: Pos2) -> Placed {
        Placed {
            asteroid,
            centre,
            sizing: Sizing::settled(Detail::Small),
            alpha: RESTING_ALPHA,
        }
    }

    fn eased(self, ease: &mut impl Ease) -> Placed {
        Placed {
            sizing: Sizing {
                detail: self.sizing.detail,
                scale: ease.ease(self.asteroid, Eased::Scale, self.sizing.scale),
            },
            alpha: ease.ease(self.asteroid, Eased::Alpha, self.alpha),
            ..self
        }
    }
}

fn button_of(gesture: Option<&WheelGesture>, asteroid: AsteroidId) -> Option<ButtonAt> {
    match gesture {
        Some(WheelGesture::Button(at, _)) if at.posting.asteroid() == asteroid => Some(*at),
        _ => None,
    }
}

fn footprints(
    scene: &Scene,
    roster: &Roster,
    viewport: &Viewport,
    viewer: SeatId,
) -> Vec<Footprint> {
    let asteroids: BTreeMap<AsteroidId, Pos2> = scene
        .asteroids
        .iter()
        .filter_map(|asteroid| Some((asteroid.id, viewport.point_of(asteroid.pos)?)))
        .collect();
    scene
        .wheels
        .iter()
        .filter_map(|wheel| {
            Some(Footprint::of(
                wheel.asteroid,
                *asteroids.get(&wheel.asteroid)?,
                wheel,
                roster,
                viewer,
            ))
        })
        .collect()
}

fn laid(
    footprints: &[Footprint],
    hovered: Option<AsteroidId>,
    selected: Option<AsteroidId>,
) -> Vec<Placed> {
    let whole = hovered.or(selected);
    footprints
        .iter()
        .map(|footprint| {
            let asteroid = Some(footprint.asteroid);
            let detail = match asteroid == hovered || asteroid == selected {
                true => Detail::Full,
                false => Detail::Small,
            };
            Placed {
                sizing: Sizing::settled(detail),
                alpha: match asteroid == whole {
                    true => 1.0,
                    false => RESTING_ALPHA,
                },
                ..Placed::resting(footprint.asteroid, footprint.centre)
            }
        })
        .collect()
}

fn hovered(footprints: &[Footprint], resting: &[Placed], aim: &Aim) -> Option<AsteroidId> {
    let pointer = aim.pointer?;
    let extent = |placed: &Placed, detail: Detail| {
        footprints
            .iter()
            .find(|footprint| footprint.asteroid == placed.asteroid)
            .map(|footprint| footprint.at(detail))
    };
    let held = aim.hovered.filter(|asteroid| {
        resting.iter().any(|placed| {
            placed.asteroid == *asteroid
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
            .map(|(placed, _)| placed.asteroid)
    })
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::belt::Belt;

    use std::sync::LazyLock;

    use neumannarch_sim::roster::FRIGATE;
    use neumannarch_sim::state::view::View;
    use neumannarch_sim::state::{Command, Rejected};
    use neumannarch_sim::step::fire::Shots;
    use neumannarch_sim::{Retention, Sequence, Session, Setup, TeamId, Tick};

    use super::*;
    use crate::display::local::Local;
    use crate::display::scene::{Entry, RowView, SectorView, WheelView};

    const A: AsteroidId = AsteroidId(0);

    const B: AsteroidId = AsteroidId(1);

    static WATCHING: LazyLock<Watching> = LazyLock::new(Watching::of);

    fn view(asteroid: AsteroidId) -> WheelView {
        WheelView {
            asteroid,
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
            .map(|(asteroid, centre)| {
                Footprint::of(asteroid, centre, &view(asteroid), &roster, SeatId(0))
            })
            .collect()
    }

    struct Watching {
        local: Local,
        view: View,
    }

    impl Watching {
        fn of() -> Watching {
            let local = Local::start(2);
            let view = local.view();
            Watching { local, view }
        }
    }

    fn aim(pointer: Pos2, hovered: Option<AsteroidId>) -> Aim<'static> {
        Aim {
            viewer: SeatId(0),
            pointer: Some(pointer),
            hovered,
            step: 1,
            view: &WATCHING.view,
            state: WATCHING.local.session().state(),
        }
    }

    fn detail(placed: &[Placed], asteroid: AsteroidId) -> Option<Detail> {
        placed
            .iter()
            .find(|placed| placed.asteroid == asteroid)
            .map(|placed| placed.sizing.detail)
    }

    fn alpha(placed: &[Placed], asteroid: AsteroidId) -> Option<f32> {
        placed
            .iter()
            .find(|placed| placed.asteroid == asteroid)
            .map(|placed| placed.alpha)
    }

    fn placed(
        apart: f32,
        hovered: Option<AsteroidId>,
        selected: Option<AsteroidId>,
    ) -> Vec<Placed> {
        laid(&footprints(apart), hovered, selected)
    }

    fn far() -> f32 {
        4.0 * footprints(0.0)[0].at(Detail::Full).width()
    }

    #[test]
    fn nothing_of_a_wheel_or_a_bar_stands_inside_a_rect_it_is_cleared_of() {
        use crate::display::camera::BeltCamera;
        use crate::display::scene::AsteroidView;
        use neumannarch_sim::{Materials, Vec3};

        let viewport = Viewport::of(
            &BeltCamera::new(Vec3::ZERO, 2_000.0, 14_000.0, 25_000.0),
            mirage_engine::math::UVec2::new(1280, 720),
            1.0,
        );
        let scene = Scene {
            asteroids: vec![AsteroidView {
                id: A,
                pos: Vec3::ZERO,
                radius: 6.0,
                caps: Materials::new(20.0, 10.0, 5.0),
                pull: Materials::ZERO,
            }],
            entities: Vec::new(),
            wheels: vec![view(A)],
            flights: Vec::new(),
            stockpile_bar: None,
            zone: 1.0,
            star_radius: Belt::STAR_RADIUS_METERS,
            star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
            belt_inner_radius: Belt::inner_radius_meters(),
            belt_outer_radius: Belt::OUTER_RADIUS_METERS,
            seat: SeatId(0),
            selection: Some(A),
            gesture: None,
        };
        let roster = Roster::shipped();
        let centre = viewport.point_of(Vec3::ZERO).expect("on screen");
        let whole = Wheels::over(&scene, &roster, &viewport, &aim(centre, None), &mut Still);
        let top = whole
            .frames()
            .fold(Rect::NOTHING, |bounds, frame| bounds.union(frame))
            .top();
        let bar = Rect::from_min_max(
            egui::pos2(centre.x - 400.0, top - 10.0),
            egui::pos2(centre.x + 400.0, top + 20.0),
        );
        assert!(
            whole.frames().any(|frame| frame.intersects(bar)),
            "the wheel and its bars reach into the stockpile bar"
        );
        let crossing = whole
            .frames()
            .find(|frame| frame.intersects(bar))
            .expect("a frame under the stockpile bar");

        let cleared =
            Wheels::over(&scene, &roster, &viewport, &aim(centre, None), &mut Still).clear_of(bar);

        assert!(cleared.frames().all(|frame| !frame.intersects(bar)));
        assert!(cleared.frames().count() < whole.frames().count());
        assert_eq!(
            cleared.button_at(crossing.center()),
            None,
            "what is not drawn takes no click"
        );
        assert_eq!(cleared.spoken_at(crossing.center()), None);
    }

    #[test]
    fn a_wheels_buttons_carry_the_refusals_the_view_reports_for_its_own_asteroid() {
        let setup = Setup::new(vec![TeamId(0), TeamId(1)], 0, Tick(600)).expect("two teams");
        let mut session =
            Session::new(setup, Retention::shipped(), &[SeatId(0), SeatId(1)]).expect("seated");
        let first = session.state().draft().stages()[0];
        let stamped = Sequence::new(first.seat).stamp(
            session.state().tick(),
            Command::Want {
                asteroid: A,
                row: first.row,
                count: 1,
            },
        );
        session.insert(stamped).expect("the pick is taken");
        assert!(session.advance().rejected.is_empty());
        let next = session
            .state()
            .draft()
            .running()
            .expect("the next stage runs");
        let view = View::of(session.state(), next.seat, &Shots::default());
        let aim = Aim {
            viewer: next.seat,
            pointer: None,
            hovered: None,
            step: 1,
            view: &view,
            state: session.state(),
        };
        let roster = session.state().roster();

        let taken = aim.buttons_at(A, roster);
        let free = aim.buttons_at(B, roster);

        assert_eq!(
            taken.refusals[&next.row].adding,
            Some(Rejected::AsteroidTaken),
            "a pick at a taken asteroid is refused"
        );
        assert_eq!(free.refusals[&next.row].adding, None);
        assert_eq!(
            taken.refusals[&FRIGATE].adding, None,
            "a frigate is no pick and stands at a taken asteroid"
        );
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

        let fast = Span::Fast.seconds();
        let born = frame(&mut motion, fast / 2.0, 1.0);
        assert!(
            (born - (small + 1.0) / 2.0).abs() < 1e-3,
            "{born}: a wheel first drawn full grows from small"
        );
        assert_eq!(frame(&mut motion, fast, 1.0), 1.0);
        assert_eq!(frame(&mut motion, 1.0, 1.0), 1.0, "and holds");
        let back = frame(&mut motion, fast / 2.0, small);
        assert!(
            (back - (small + 1.0) / 2.0).abs() < 1e-3,
            "{back}: it shrinks by the same span"
        );
        assert_eq!(frame(&mut motion, fast, small), small);
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
