use mirage_engine::egui::{self, Pos2, Rect, Shape};
use neumannarch_sim::{AsteroidId, Material};

use crate::display::glyph::{Cell, Drawing, Look};
use crate::display::hue;
use crate::display::scene::{AsteroidView, Scene};
use crate::display::wheel::{self, Placed, Side};
use crate::screens::panel;

pub const ROW_WIDTH: f32 = wheel::PAD
    + wheel::GLYPH_SLOT
    + wheel::GAP
    + wheel::MARK
    + wheel::GAP
    + wheel::CHARACTER_WIDTH
    + wheel::CELL_GAP
    + wheel::PAD;

pub const ICON_SLOT: f32 = 2.0 * wheel::GLYPH_HALF;

pub const BAR_LENGTH: f32 = ROW_WIDTH;

pub const BAND_HEIGHT: f32 = wheel::LINE_HEIGHT;

pub const CAP_ALPHA: f32 = 0.35;

pub struct Bars {
    bars: Vec<Bar>,
}

struct Bar {
    material: Material,
    frame: Rect,
    band: Rect,
    pull: f64,
    cap: f64,
    placed: Placed,
}

impl Bars {
    pub fn over(scene: &Scene, placed: &[Placed]) -> Bars {
        let largest = scene.largest_cap();
        let bars = placed
            .iter()
            .filter_map(|placed| {
                let asteroid = scene
                    .asteroids
                    .iter()
                    .find(|asteroid| asteroid.id == placed.asteroid)?;
                Some(stacked(asteroid, *placed, largest))
            })
            .flatten()
            .collect();
        Bars { bars }
    }

    pub fn frames(&self) -> impl Iterator<Item = Rect> + '_ {
        self.bars.iter().map(|bar| bar.frame)
    }

    pub fn paint(&self, painter: &egui::Painter) {
        for spine in self.spines() {
            painter.add(spine);
        }
        for bar in &self.bars {
            bar.paint(painter);
        }
    }

    pub fn spoken_at(&self, at: Pos2) -> Option<(Pos2, Material, f64, f64)> {
        self.bars
            .iter()
            .find(|bar| bar.frame.contains(at))
            .map(|bar| (bar.frame.center(), bar.material, bar.pull, bar.cap))
    }

    pub fn of_asteroid(
        &self,
        asteroid: AsteroidId,
    ) -> impl Iterator<Item = (Material, Rect, Rect)> + '_ {
        self.bars
            .iter()
            .filter(move |bar| bar.placed.asteroid == asteroid)
            .map(|bar| (bar.material, bar.frame, bar.band))
    }

    fn spines(&self) -> impl Iterator<Item = Shape> + '_ {
        self.bars
            .iter()
            .filter(|bar| {
                self.bars
                    .iter()
                    .find(|first| first.placed.asteroid == bar.placed.asteroid)
                    .is_some_and(|first| core::ptr::eq(first, *bar))
            })
            .map(|first| {
                let Placed {
                    asteroid,
                    centre,
                    stand_off,
                    scale,
                    alpha,
                    ..
                } = first.placed;
                let stack = self
                    .of_asteroid(asteroid)
                    .map(|(_, frame, _)| frame)
                    .reduce(|stack, frame| stack.union(frame))
                    .unwrap_or(first.frame);
                Shape::line(
                    wheel::spine_points(
                        centre,
                        scale,
                        stand_off,
                        stack.top() - centre.y,
                        stack.bottom() - centre.y,
                        Side::Left,
                    ),
                    wheel::spine_stroke(panel::DIM_INK, alpha),
                )
            })
    }
}

impl Bar {
    fn paint(&self, painter: &egui::Painter) {
        let scale = self.placed.scale;
        let alpha = self.placed.alpha;
        let hue = hue::of(self.material);
        let toned = |strength: f32| panel::BACKDROP.lerp_to_gamma(hue, strength * alpha);
        painter.rect_filled(self.band, 0.0, toned(CAP_ALPHA));
        let filled = (self.pull / self.cap).clamp(0.0, 1.0) as f32;
        if filled > 0.0 {
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(
                        self.band.right() - self.band.width() * filled,
                        self.band.top(),
                    ),
                    self.band.max,
                ),
                0.0,
                toned(1.0),
            );
        }
        Drawing::icon(self.material).paint(
            painter,
            Cell {
                centre: egui::pos2(
                    self.frame.right() - (wheel::PAD + ICON_SLOT / 2.0) * scale,
                    self.frame.center().y,
                ),
                half: wheel::GLYPH_HALF * scale,
            },
            Look::solid(toned(1.0)),
        );
    }
}

fn stacked(asteroid: &AsteroidView, placed: Placed, largest: f64) -> Vec<Bar> {
    let scale = placed.scale;
    let centre = placed.centre;
    let rows: Vec<(Material, f64)> = asteroid
        .caps
        .amounts()
        .filter(|(_, cap)| *cap > 0.0)
        .collect();
    let height = wheel::SECTION_HEIGHT * scale;
    let step = (wheel::SECTION_HEIGHT + wheel::ROW_GAP) * scale;
    let total =
        rows.len() as f32 * height + rows.len().saturating_sub(1) as f32 * wheel::ROW_GAP * scale;
    rows.into_iter()
        .enumerate()
        .map(|(index, (material, cap))| {
            let top = -total / 2.0 + index as f32 * step;
            let middle = top + height / 2.0;
            let length = (cap / largest) as f32 * BAR_LENGTH;
            let width = (wheel::PAD + length + wheel::GAP + ICON_SLOT + wheel::PAD) * scale;
            let right = centre.x - wheel::x_at(scale, placed.stand_off, middle);
            let frame = Rect::from_min_size(
                egui::pos2(right - width, centre.y + top),
                egui::vec2(width, height),
            );
            let band = Rect::from_center_size(
                egui::pos2(
                    frame.left() + (wheel::PAD + length / 2.0) * scale,
                    frame.center().y,
                ),
                egui::vec2(length * scale, BAND_HEIGHT * scale),
            );
            Bar {
                material,
                frame,
                band,
                pull: asteroid.pull[material],
                cap,
                placed,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use neumannarch_sim::belt::Belt;

    use neumannarch_sim::{Materials, Vec3};

    use super::*;
    use crate::display::camera::BeltCamera;
    use crate::display::scene::Scene;
    use crate::display::viewport::Viewport;
    use crate::display::wheels::RESTING_ALPHA;

    const RICH: AsteroidId = AsteroidId(0);

    const STAND_OFF: f32 = 80.0;

    const POOR: AsteroidId = AsteroidId(1);

    fn scene() -> Scene {
        let asteroid = |id: AsteroidId, pos: Vec3, caps: Materials, pull: Materials| AsteroidView {
            id,
            pos,
            radius: 6.0,
            caps,
            pull,
        };
        Scene {
            asteroids: vec![
                asteroid(
                    RICH,
                    Vec3::new(-300.0, 0.0, 0.0),
                    Materials::new(20.0, 10.0, 0.0),
                    Materials::new(12.0, 0.0, 0.0),
                ),
                asteroid(
                    POOR,
                    Vec3::new(300.0, 0.0, 0.0),
                    Materials::new(5.0, 5.0, 5.0),
                    Materials::ZERO,
                ),
            ],
            entities: Vec::new(),
            wheels: Vec::new(),
            fights: BTreeMap::new(),
            flights: Vec::new(),
            stockpile_bar: None,
            zone: 1.0,
            star_radius: Belt::STAR_RADIUS_METERS,
            star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
            belt_inner_radius: Belt::inner_radius_meters(),
            belt_outer_radius: Belt::OUTER_RADIUS_METERS,
            seat: neumannarch_sim::SeatId(0),
            selection: None,
            gesture: None,
        }
    }

    fn asteroid_point(asteroid: AsteroidId) -> Pos2 {
        let viewport = Viewport::of(
            &BeltCamera::new(Vec3::ZERO, 2_000.0, 14_000.0, 25_000.0),
            mirage_engine::math::UVec2::new(1280, 720),
            1.0,
        );
        let scene = scene();
        let asteroid = scene
            .asteroids
            .iter()
            .find(|view| view.id == asteroid)
            .expect("an asteroid");
        viewport.point_of(asteroid.pos).expect("on screen")
    }

    fn placed(asteroid: AsteroidId, scale: f32, alpha: f32) -> Placed {
        Placed {
            asteroid,
            centre: asteroid_point(asteroid),
            stand_off: STAND_OFF,
            scale,
            alpha,
            shrinking: false,
        }
    }

    fn both() -> Bars {
        Bars::over(
            &scene(),
            &[placed(RICH, 1.0, 1.0), placed(POOR, 1.0, RESTING_ALPHA)],
        )
    }

    #[test]
    fn an_asteroid_carries_a_bar_per_material_it_caps_at_its_left_stacked_on_its_height() {
        let bars = both();
        let centre = asteroid_point(RICH);

        let rich: Vec<(Material, Rect, Rect)> = bars.of_asteroid(RICH).collect();
        assert_eq!(
            rich.iter()
                .map(|(material, _, _)| *material)
                .collect::<Vec<_>>(),
            vec![Material::Metals, Material::Volatiles],
            "a cap of zero draws no bar"
        );
        assert!(rich.iter().all(|(_, frame, _)| frame.right() < centre.x));
        assert!(
            rich[0].1.bottom() <= rich[1].1.top(),
            "stacked without touching"
        );
        let stack = rich[0].1.union(rich[1].1);
        assert!((stack.center().y - centre.y).abs() < 1.0);
        assert_eq!(bars.of_asteroid(POOR).count(), 3);
    }

    #[test]
    fn a_bar_grows_leftward_from_its_icon_at_the_asteroids_side_by_its_cap_against_the_largest() {
        let bars = both();
        let bar = |asteroid, material| {
            bars.of_asteroid(asteroid)
                .find(|(shown, _, _)| *shown == material)
                .map(|(_, frame, band)| (frame, band))
                .expect("a bar")
        };

        let (frame, longest) = bar(RICH, Material::Metals);
        assert!((longest.width() - BAR_LENGTH).abs() < 1e-3);
        assert!(
            longest.right() < frame.right() - ICON_SLOT,
            "the icon stands right of the bar, nearest the asteroid"
        );
        let (shorter_frame, shorter) = bar(RICH, Material::Volatiles);
        assert!((shorter.width() - longest.width() / 2.0).abs() < 1e-3);
        assert!(
            (shorter_frame.right() - frame.right()).abs() < 2.0,
            "both rows end at the asteroid's side and the short one starts nearer it"
        );
        assert!((bar(POOR, Material::Energy).1.width() - longest.width() / 4.0).abs() < 1e-3);
        assert!(
            (longest.width() - ROW_WIDTH).abs() < 1e-3,
            "the longest bar is a wheel strip's width"
        );
    }

    #[test]
    fn the_bars_right_ends_stand_on_the_wheels_arc_mirrored_and_a_spine_runs_along_it() {
        let poor = placed(POOR, 1.0, 1.0);
        let bars = Bars::over(&scene(), &[poor]);
        let frames: Vec<Rect> = bars.of_asteroid(POOR).map(|(_, frame, _)| frame).collect();

        assert_eq!(frames.len(), 3);
        assert!(
            frames[1].right() < frames[0].right() && frames[1].right() < frames[2].right(),
            "the middle row ends farther from the asteroid than the ends, the wheel's bow mirrored"
        );
        let spines: Vec<Shape> = bars.spines().collect();
        assert_eq!(spines.len(), 1, "one spine per asteroid");
        let Shape::Path(path) = &spines[0] else {
            panic!("a spine is a line: {:?}", spines[0]);
        };
        let stack = frames
            .iter()
            .fold(frames[0], |stack, frame| stack.union(*frame));
        assert!(
            path.points.iter().all(|point| {
                point.x > frames[1].right()
                    && point.x < poor.centre.x
                    && (stack.top()..=stack.bottom()).contains(&point.y)
            }),
            "the spine runs between the bars' right ends and the asteroid"
        );
    }

    #[test]
    fn only_a_placed_asteroid_carries_bars_and_at_the_scale_and_alpha_it_was_placed_at() {
        let bars = Bars::over(&scene(), &[placed(RICH, 0.5, RESTING_ALPHA)]);

        assert_eq!(
            bars.of_asteroid(POOR).count(),
            0,
            "an asteroid with no wheel carries no bar"
        );
        let bar = bars.bars.first().expect("a bar");
        assert!((bar.frame.height() - wheel::SECTION_HEIGHT * 0.5).abs() < 1e-3);
        assert_eq!(bar.placed.alpha, RESTING_ALPHA);
    }

    #[test]
    fn a_bar_under_the_pointer_says_its_pull_of_its_cap() {
        let bars = both();
        let (_, frame, _) = bars.of_asteroid(RICH).next().expect("a bar");

        assert_eq!(
            bars.spoken_at(frame.center())
                .map(|(_, material, pull, cap)| (material, pull, cap)),
            Some((Material::Metals, 12.0, 20.0))
        );
        assert_eq!(bars.spoken_at(asteroid_point(RICH)), None);
    }
}
