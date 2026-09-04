use core::ops::{Range, RangeInclusive};

use mirage_engine::math;
use mirage_engine::{Camera, Projection, View};
use probe_sim::Vec3;
use probe_sim::orbit::Gravity;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeltCamera {
    focus: Vec3,
    distance: f64,
}

impl BeltCamera {
    pub const FOV_DEGREES: f32 = 50.0;

    pub const TILT_DEGREES: f64 = 60.0;

    pub const ZOOM_RANGE: RangeInclusive<f64> = 40.0..=12_000.0;

    pub fn new(focus: Vec3, distance: f64) -> Self {
        Self {
            focus,
            distance: Self::within_zoom_range(distance),
        }
    }

    pub fn advance(&mut self, dt: f64, gravity: Gravity) {
        let radius = self.focus.x.hypot(self.focus.z);
        let Some(rate) = circular_rate(radius, gravity) else {
            return;
        };
        let (sin, cos) = (rate * dt).sin_cos();
        let Vec3 { x, y, z } = self.focus;
        self.focus = Vec3::new(x * cos + z * sin, y, z * cos - x * sin);
    }

    pub fn pan(&mut self, delta: Vec3) {
        self.focus += delta;
    }

    pub fn pan_by_pixels(&mut self, delta: math::Vec2, window: math::UVec2) {
        let Some(pixels_per_meter) = self
            .engine_camera()
            .pixels_per_meter(math::Vec3::ZERO, window)
        else {
            return;
        };
        let across = f64::from(delta.x / pixels_per_meter);
        let into = f64::from(delta.y / pixels_per_meter) / Self::TILT_DEGREES.to_radians().sin();
        self.pan(Vec3::new(-across, 0.0, -into));
    }

    pub fn zoom(&mut self, factor: f64) {
        self.distance = Self::within_zoom_range(self.distance * factor);
    }

    pub fn set_focus(&mut self, focus: Vec3) {
        self.focus = focus;
    }

    pub fn focus(&self) -> Vec3 {
        self.focus
    }

    pub fn distance(&self) -> f64 {
        self.distance
    }

    pub fn engine_camera(&self) -> Camera {
        Camera::new(
            View::look_at(self.local(self.eye()), math::Vec3::ZERO),
            Projection::perspective(Self::FOV_DEGREES).clip(Self::clip_range()),
        )
    }

    pub(crate) fn local(&self, point: Vec3) -> math::Vec3 {
        let relative = point - self.focus;
        math::Vec3::new(relative.x as f32, relative.y as f32, relative.z as f32)
    }

    fn eye(&self) -> Vec3 {
        let (rise, run) = Self::TILT_DEGREES.to_radians().sin_cos();
        self.focus + Vec3::new(0.0, rise, run) * self.distance
    }

    fn within_zoom_range(distance: f64) -> f64 {
        distance.clamp(*Self::ZOOM_RANGE.start(), *Self::ZOOM_RANGE.end())
    }

    fn clip_range() -> Range<f32> {
        let (nearest, farthest) = (*Self::ZOOM_RANGE.start(), *Self::ZOOM_RANGE.end());
        (nearest / 10.0) as f32..(farthest * 2.0) as f32
    }
}

fn circular_rate(radius: f64, gravity: Gravity) -> Option<f64> {
    (radius > 0.0).then(|| (gravity.mu() / radius.powi(3)).sqrt())
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;

    const GRAVITY: Gravity = Gravity::new(1e9);

    const WINDOW: math::UVec2 = math::UVec2::new(1280, 720);

    fn over_the_belt() -> BeltCamera {
        BeltCamera::new(Vec3::new(3000.0, 0.0, -2000.0), 400.0)
    }

    #[test]
    fn a_quarter_period_turns_the_focus_a_quarter_turn_prograde() {
        let radius: f64 = 100.0;
        let period = TAU * (radius.powi(3) / GRAVITY.mu()).sqrt();
        let start = Vec3::new(radius, 0.0, 0.0);
        let mut camera = BeltCamera::new(start, 50.0);

        camera.advance(period / 4.0, GRAVITY);

        let end = camera.focus();
        assert!(
            end.distance(Vec3::new(0.0, 0.0, -radius)) <= 1e-6 * radius,
            "{end:?}"
        );
        assert!(
            start.cross(end).y > 0.0,
            "counter-clockwise seen from +Y turns +X toward -Z"
        );
    }

    #[test]
    fn a_focus_at_the_origin_stays() {
        let mut camera = BeltCamera::new(Vec3::ZERO, 50.0);

        camera.advance(1e3, GRAVITY);

        assert_eq!(camera.focus(), Vec3::ZERO);
    }

    #[test]
    fn pan_shifts_the_focus_by_the_delta() {
        let mut camera = over_the_belt();
        let before = camera.focus();

        camera.pan(Vec3::new(5.0, 0.0, -7.0));

        assert_eq!(camera.focus(), before + Vec3::new(5.0, 0.0, -7.0));
    }

    #[test]
    fn a_pointer_drag_keeps_the_belt_under_the_pointer() {
        let camera = over_the_belt();
        let viewport = crate::display::viewport::Viewport::of(&camera, WINDOW, 1.0);

        let held = camera.focus();
        let was = viewport.pixel_of(held).expect("the point is on screen");
        let delta = math::Vec2::new(-60.0, 25.0);

        let mut dragged = camera;
        dragged.pan_by_pixels(delta, WINDOW);

        let now = crate::display::viewport::Viewport::of(&dragged, WINDOW, 1.0)
            .pixel_of(held)
            .expect("the point stays on screen");

        assert!(
            (now - was - delta).length() <= 0.03 * delta.length(),
            "{was} moved to {now}, not by {delta}"
        );
    }

    #[test]
    fn zoom_clamps_to_its_range() {
        let mut camera = over_the_belt();

        camera.zoom(1e12);
        assert_eq!(camera.distance(), *BeltCamera::ZOOM_RANGE.end());

        camera.zoom(0.0);
        assert_eq!(camera.distance(), *BeltCamera::ZOOM_RANGE.start());
    }

    #[test]
    fn the_farthest_zoom_clips_far_enough_for_a_region_beyond_the_focus() {
        let camera = BeltCamera::new(Vec3::ZERO, *BeltCamera::ZOOM_RANGE.end());
        let viewport = crate::display::viewport::Viewport::of(&camera, WINDOW, 1.0);
        assert!(
            viewport.pixel_of(camera.focus()).is_some(),
            "the focus must render at the farthest zoom"
        );

        let region_depth = *BeltCamera::ZOOM_RANGE.end() - *BeltCamera::ZOOM_RANGE.start();
        let needed = camera.distance() + region_depth;
        let far = f64::from(camera.engine_camera().projection().far());

        assert!(
            far > needed,
            "far clip {far} does not reach {needed}, one region past the focus"
        );
    }

    #[test]
    fn the_widest_zoom_shows_two_rocks_of_the_shipped_belt() {
        let rocks = probe_sim::belt::Belt::fixed(probe_sim::belt::Belt::GRAVITY);
        let body = |at: usize| {
            rocks[at]
                .orbit()
                .at(probe_sim::Tick::ZERO, probe_sim::belt::Belt::GRAVITY)
        };
        let (first, second) = (body(0), body(1));
        let camera = BeltCamera::new(first.pos, *BeltCamera::ZOOM_RANGE.end());
        let viewport = crate::display::viewport::Viewport::of(&camera, WINDOW, 1.0);

        let apart = viewport
            .pixel_of(second.pos)
            .expect("a neighbour is in front of the eye")
            .distance(viewport.pixel_of(first.pos).expect("the focus"));

        assert!(
            apart > 40.0,
            "neighbouring rings would sit {apart} px apart"
        );
        assert!(
            apart < WINDOW.y as f32 / 2.0,
            "a neighbour {apart} px away is off screen"
        );
    }
}
