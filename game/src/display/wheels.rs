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
use crate::display::wheel::{ButtonRefusals, Buttons, Footprint, Placed, Wheel};

pub const RESTING_ALPHA: f32 = 0.7;

const HOVER_MARGIN: f32 = 48.0;

const GONE_SCALE: f32 = 0.01;

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
            Eased::Scale => 0.0,
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
    target: f64,
    stepped: u64,
}

impl Motion {
    pub fn begin(&mut self, dt: f64) {
        self.frame += 1;
        self.dt = dt;
        let gone: Vec<AsteroidId> = self
            .scales()
            .filter(|(_, tween)| tween.target == 0.0 && tween.value <= f64::from(GONE_SCALE))
            .map(|(asteroid, _)| asteroid)
            .collect();
        self.tweens
            .retain(|(asteroid, _), _| !gone.contains(asteroid));
    }

    pub fn shrinking(&self) -> Vec<AsteroidId> {
        self.scales()
            .filter(|(_, tween)| tween.value > f64::from(GONE_SCALE))
            .map(|(asteroid, _)| asteroid)
            .collect()
    }

    fn scales(&self) -> impl Iterator<Item = (AsteroidId, &Tween)> {
        self.tweens
            .iter()
            .filter(|((_, eased), _)| *eased == Eased::Scale)
            .map(|((asteroid, _), tween)| (*asteroid, tween))
    }
}

impl Ease for Motion {
    fn ease(&mut self, asteroid: AsteroidId, eased: Eased, target: f32) -> f32 {
        let frame = self.frame;
        let dt = self.dt;
        let tween = self.tweens.entry((asteroid, eased)).or_insert(Tween {
            value: f64::from(eased.at_rest()),
            target: f64::from(target),
            stepped: 0,
        });
        if tween.stepped != frame {
            tween.stepped = frame;
            tween.target = f64::from(target);
            tween.value = ease::toward(tween.value, tween.target, dt, Span::Fast);
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
        let hovered = hovered(&footprints, aim);
        let asked =
            |asteroid: AsteroidId| Some(asteroid) == hovered || Some(asteroid) == scene.selection;
        let placed: Vec<Placed> = laid(&footprints, hovered, scene.selection)
            .into_iter()
            .map(|placed| match asked(placed.asteroid) {
                true => placed,
                false => Placed {
                    scale: 0.0,
                    shrinking: true,
                    ..placed
                },
            })
            .map(|placed| placed.eased(ease))
            .filter(|placed| placed.scale > GONE_SCALE)
            .collect();
        let mut wheels: Vec<Wheel> = placed
            .iter()
            .filter_map(|placed| {
                let view = scene.wheel_of(placed.asteroid)?;
                let buttons = aim.buttons_at(placed.asteroid, roster);
                let wheel = Wheel::over(*placed, view, roster, aim.viewer, Some(buttons));
                wheel.draws().then_some(wheel)
            })
            .collect();
        wheels.sort_by(|a, b| b.alpha().total_cmp(&a.alpha()));
        Wheels {
            wheels,
            bars: Bars::over(scene, &placed),
            hovered,
        }
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
    fn eased(self, ease: &mut impl Ease) -> Placed {
        Placed {
            scale: ease.ease(self.asteroid, Eased::Scale, self.scale),
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
    let asteroids: BTreeMap<AsteroidId, (Pos2, f32)> = scene
        .asteroids
        .iter()
        .filter_map(|asteroid| {
            let zone_points = scene.zone as f32 / viewport.meters_per_point(asteroid.pos)?;
            Some((
                asteroid.id,
                (
                    viewport.point_of(asteroid.pos)?,
                    Wheel::stand_off(zone_points),
                ),
            ))
        })
        .collect();
    scene
        .wheels
        .iter()
        .filter_map(|wheel| {
            let (centre, stand_off) = *asteroids.get(&wheel.asteroid)?;
            Some(Footprint::of(
                wheel.asteroid,
                centre,
                stand_off,
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
        .map(|footprint| Placed {
            asteroid: footprint.asteroid,
            centre: footprint.centre,
            stand_off: footprint.stand_off,
            scale: 1.0,
            alpha: match Some(footprint.asteroid) == whole {
                true => 1.0,
                false => RESTING_ALPHA,
            },
            shrinking: false,
        })
        .collect()
}

fn hovered(footprints: &[Footprint], aim: &Aim) -> Option<AsteroidId> {
    let pointer = aim.pointer?;
    let held = footprints
        .iter()
        .find(|footprint| Some(footprint.asteroid) == aim.hovered)
        .filter(|footprint| footprint.rect().expand(HOVER_MARGIN).contains(pointer))
        .map(|footprint| footprint.asteroid);
    held.or_else(|| {
        footprints
            .iter()
            .filter(|footprint| footprint.rect().contains(pointer))
            .min_by(|a, b| {
                a.rect().area().total_cmp(&b.rect().area()).then(
                    a.centre
                        .distance(pointer)
                        .total_cmp(&b.centre.distance(pointer)),
                )
            })
            .map(|footprint| footprint.asteroid)
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
    use neumannarch_sim::{Materials, Retention, Sequence, Session, Setup, TeamId, Tick, Vec3};

    use super::*;
    use crate::display::camera::BeltCamera;
    use crate::display::local::Local;
    use crate::display::scene::{AsteroidView, Entry, RowView, SectorView, WheelView};

    const A: AsteroidId = AsteroidId(0);

    const STAND_OFF: f32 = 80.0;

    const B: AsteroidId = AsteroidId(1);

    static WATCHING: LazyLock<Watching> = LazyLock::new(Watching::of);

    fn view_of(asteroid: AsteroidId, seat: SeatId) -> WheelView {
        WheelView {
            asteroid,
            sectors: vec![SectorView {
                seat,
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

    fn view(asteroid: AsteroidId) -> WheelView {
        view_of(asteroid, SeatId(0))
    }

    fn footprints(apart: f32) -> Vec<Footprint> {
        let roster = Roster::shipped();
        [(A, egui::pos2(0.0, 0.0)), (B, egui::pos2(apart, 0.0))]
            .into_iter()
            .map(|(asteroid, centre)| {
                Footprint::of(
                    asteroid,
                    centre,
                    STAND_OFF,
                    &view(asteroid),
                    &roster,
                    SeatId(0),
                )
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

    fn scaled(placed: &[Placed], asteroid: AsteroidId) -> Option<f32> {
        placed
            .iter()
            .find(|placed| placed.asteroid == asteroid)
            .map(|placed| placed.scale)
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
        4.0 * footprints(0.0)[0].rect().width()
    }

    fn scene(selection: Option<AsteroidId>, wheels: Vec<WheelView>) -> Scene {
        Scene {
            asteroids: vec![
                AsteroidView {
                    id: A,
                    pos: Vec3::new(-300.0, 0.0, 0.0),
                    radius: 6.0,
                    caps: Materials::new(20.0, 10.0, 5.0),
                    pull: Materials::ZERO,
                },
                AsteroidView {
                    id: B,
                    pos: Vec3::new(300.0, 0.0, 0.0),
                    radius: 6.0,
                    caps: Materials::new(20.0, 10.0, 5.0),
                    pull: Materials::ZERO,
                },
            ],
            entities: Vec::new(),
            wheels,
            fights: BTreeMap::new(),
            flights: Vec::new(),
            stockpile_bar: None,
            zone: 1.0,
            star_radius: Belt::STAR_RADIUS_METERS,
            star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
            belt_inner_radius: Belt::inner_radius_meters(),
            belt_outer_radius: Belt::OUTER_RADIUS_METERS,
            seat: SeatId(0),
            selection,
            gesture: None,
        }
    }

    fn viewport() -> Viewport {
        Viewport::of(
            &BeltCamera::new(Vec3::ZERO, 2_000.0, 14_000.0, 25_000.0),
            mirage_engine::math::UVec2::new(1280, 720),
            1.0,
        )
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
    fn only_the_hovered_and_the_selected_wheel_are_laid_whole_and_take_a_click() {
        let laid = placed(far(), Some(A), Some(B));
        assert_eq!(scaled(&laid, A), Some(1.0));
        assert_eq!(scaled(&laid, B), Some(1.0));

        let roster = Roster::shipped();
        let viewport = viewport();
        let scene = |selection| scene(selection, vec![view(A), view(B)]);
        let centre = |asteroid: usize| {
            viewport
                .point_of(scene(None).asteroids[asteroid].pos)
                .expect("on screen")
        };
        let mut motion = Motion::default();
        let fast = Span::Fast.seconds();

        motion.begin(fast);
        let held = Wheels::over(
            &scene(Some(B)),
            &roster,
            &viewport,
            &aim(centre(1), None),
            &mut motion,
        );
        assert_eq!(
            held.iter().map(Wheel::asteroid).collect::<Vec<_>>(),
            vec![B],
            "a wheel neither hovered nor selected is sent to nothing"
        );

        motion.begin(fast / 2.0);
        let turning = Wheels::over(
            &scene(Some(A)),
            &roster,
            &viewport,
            &aim(centre(0), None),
            &mut motion,
        );
        let wheel = |asteroid| {
            turning
                .iter()
                .find(|wheel| wheel.asteroid() == asteroid)
                .expect("a wheel")
        };
        assert_eq!(
            wheel(B).frames().count(),
            wheel(A).frames().count(),
            "a wheel shrinking away keeps its sector laid as it was"
        );
        let plus = |asteroid| {
            wheel(asteroid)
                .button(FRIGATE, WheelButton::Plus(1))
                .expect("its buttons are drawn")
        };
        assert_eq!(
            wheel(A).button_at(plus(A)),
            Some(ButtonAt {
                posting: Posting::of(A, SeatId(0), FRIGATE),
                button: WheelButton::Plus(1),
            }),
            "the selected wheel takes the click"
        );
        assert_eq!(
            wheel(B).button_at(plus(B)),
            None,
            "a wheel shrinking away takes none"
        );
        assert_eq!(wheel(B).spoken_at(plus(B)), None, "and says nothing");
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
    }

    #[test]
    fn a_hovered_wheel_keeps_the_pointer_until_it_leaves_by_the_margin() {
        let footprints = footprints(far());
        let rect = footprints[0].rect();
        let outside = egui::pos2(rect.right() + 2.0, 0.0);

        assert_eq!(
            hovered(&footprints, &aim(outside, None)),
            None,
            "past a wheel's edge nothing is hovered"
        );
        assert_eq!(
            hovered(&footprints, &aim(outside, Some(A))),
            Some(A),
            "and a hovered wheel keeps the pointer"
        );
        assert_eq!(
            hovered(
                &footprints,
                &aim(egui::pos2(rect.right() + HOVER_MARGIN - 1.0, 0.0), Some(A))
            ),
            Some(A),
            "an overshoot past its edge keeps it"
        );
        assert_eq!(
            hovered(
                &footprints,
                &aim(egui::pos2(rect.right() + HOVER_MARGIN + 1.0, 0.0), Some(A))
            ),
            None,
            "until the pointer leaves its extent by the margin"
        );
    }

    #[test]
    fn a_wheel_grows_from_nothing_and_shrinks_back_over_the_same_span_then_is_dropped() {
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

        let fast = Span::Fast.seconds();
        let born = frame(&mut motion, fast / 2.0, 1.0);
        assert!(
            (born - 0.5).abs() < 1e-3,
            "{born}: a wheel first drawn grows from nothing"
        );
        assert_eq!(frame(&mut motion, fast, 1.0), 1.0);
        assert_eq!(frame(&mut motion, 1.0, 1.0), 1.0, "and holds");
        assert_eq!(motion.shrinking(), vec![A], "a wheel on screen is drawn");
        let back = frame(&mut motion, fast / 2.0, 0.0);
        assert!(
            (back - 0.5).abs() < 1e-3,
            "{back}: it shrinks by the same span"
        );
        assert_eq!(motion.shrinking(), vec![A], "and is drawn while it shrinks");
        assert_eq!(frame(&mut motion, fast, 0.0), 0.0);
        assert!(
            motion.shrinking().is_empty(),
            "a wheel shrunk to nothing is drawn no more"
        );

        motion.begin(fast);
        assert!(motion.tweens.is_empty(), "and its tweens are dropped");
    }

    #[test]
    fn a_narrow_wheel_under_the_pointer_is_hovered_though_a_wider_one_reaches_it() {
        let roster = Roster::shipped();
        let apart = footprints(0.0)[0].rect().width() / 2.0;
        let footprints = vec![
            Footprint::of(
                A,
                egui::pos2(0.0, 0.0),
                STAND_OFF,
                &view(A),
                &roster,
                SeatId(0),
            ),
            Footprint::of(
                B,
                egui::pos2(apart, 0.0),
                STAND_OFF,
                &view_of(B, SeatId(1)),
                &roster,
                SeatId(0),
            ),
        ];
        let inside_b = footprints[1].rect().center();
        assert!(
            footprints[1].rect().width() < footprints[0].rect().width(),
            "a rival's wheel shows what it holds alone"
        );
        assert!(
            footprints[0].rect().contains(inside_b),
            "the wider wheel reaches over the narrow one"
        );

        assert_eq!(hovered(&footprints, &aim(inside_b, None)), Some(B));
    }

    #[test]
    fn the_pointer_over_a_wheel_hovers_the_nearest_one() {
        let footprints = footprints(far());

        assert_eq!(
            hovered(&footprints, &aim(egui::pos2(10.0, 0.0), None)),
            Some(A)
        );
        assert_eq!(
            hovered(&footprints, &aim(egui::pos2(far() + 10.0, 0.0), None)),
            Some(B)
        );
    }
}
