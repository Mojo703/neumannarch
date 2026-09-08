use core::ops::Range;

use mirage_engine::math;
use mirage_engine::ray::Plane;
use mirage_engine::{Camera, Projection, View};
use neumannarch_sim::Vec3;
use neumannarch_sim::orbit::Gravity;

use crate::display::ease::{self, Span};

const ZOOM_MARGIN: f64 = 1.1;

const FOCUS_FLOOR_SHARE_OF_INNER_RADIUS: f64 = 0.5;

const FOCUS_MARGIN_SHARE_OF_OUTER_RADIUS: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeltCamera {
    focus: Vec3,
    distance: f64,
    shown_focus: Vec3,
    shown_distance: f64,
    yaw: f64,
    inner_radius: f64,
    outer_radius: f64,
}

impl BeltCamera {
    pub const FOV_DEGREES: f32 = 50.0;

    pub const TILT_DEGREES: f64 = 60.0;

    pub const NEAREST_ZOOM: f64 = 40.0;

    pub fn new(focus: Vec3, distance: f64, inner_radius: f64, outer_radius: f64) -> Self {
        let mut camera = Self {
            focus,
            distance,
            shown_focus: focus,
            shown_distance: distance,
            yaw: 0.0,
            inner_radius,
            outer_radius,
        };
        camera.distance = camera.within_zoom_range(distance);
        camera.shown_distance = camera.distance;
        camera.face_the_star();
        camera
    }

    pub fn framing(inner_radius: f64, outer_radius: f64) -> Self {
        let mut camera = Self::new(Vec3::ZERO, Self::NEAREST_ZOOM, inner_radius, outer_radius);
        camera.distance = camera.farthest_zoom();
        camera.shown_distance = camera.distance;
        camera
    }

    pub fn advance(&mut self, dt: f64, gravity: Gravity) {
        let radius = self.focus.x.hypot(self.focus.z).max(self.inner_radius);
        let rate = (gravity.mu() / radius.powi(3)).sqrt();
        let (sin, cos) = (rate * dt).sin_cos();
        let turned = |Vec3 { x, y, z }: Vec3| Vec3::new(x * cos + z * sin, y, z * cos - x * sin);
        self.focus = turned(self.focus);
        self.shown_focus = turned(self.shown_focus);
        self.face_the_star();
    }

    pub fn settle(&mut self, dt: f64) {
        let Vec3 { x, y, z } = self.focus;
        self.shown_focus = Vec3::new(
            ease::toward(self.shown_focus.x, x, dt, Span::Slow),
            ease::toward(self.shown_focus.y, y, dt, Span::Slow),
            ease::toward(self.shown_focus.z, z, dt, Span::Slow),
        );
        self.shown_distance = ease::toward(self.shown_distance, self.distance, dt, Span::Slow);
        self.face_the_star();
    }

    pub fn pan(&mut self, delta: Vec3) {
        let outward = self.focus_outward();
        let outward_meters = delta.dot(outward);
        let held = outward * (outward_meters * (self.radial_share(outward_meters) - 1.0));
        self.focus = self.within_the_belt(self.focus + delta + held);
    }

    pub fn pan_by_pointer(&mut self, from: math::Vec2, to: math::Vec2, window: math::UVec2) {
        let (Some(was), Some(now)) = (
            self.on_the_focus_plane(from, window),
            self.on_the_focus_plane(to, window),
        ) else {
            return;
        };
        self.pan(was - now);
    }

    pub fn zoom(&mut self, factor: f64) {
        self.distance = self.within_zoom_range(self.distance * factor);
    }

    pub fn set_focus(&mut self, focus: Vec3) {
        self.focus = focus;
    }

    pub fn focus(&self) -> Vec3 {
        self.focus
    }

    pub fn shown_focus(&self) -> Vec3 {
        self.shown_focus
    }

    pub fn distance(&self) -> f64 {
        self.distance
    }

    pub fn farthest_zoom(&self) -> f64 {
        let (rise, run) = Self::TILT_DEGREES.to_radians().sin_cos();
        let spread = (f64::from(Self::FOV_DEGREES) / 2.0).to_radians().tan();
        let across = self.outer_radius * (run + 1.0 / spread);
        let along = self.outer_radius * (run + rise / spread);
        (across.max(along) * ZOOM_MARGIN).max(Self::NEAREST_ZOOM)
    }

    pub fn engine_camera(&self) -> Camera {
        Camera::new(
            View::look_at(self.local(self.eye()), math::Vec3::ZERO),
            Projection::perspective(Self::FOV_DEGREES).clip(self.clip_range()),
        )
    }

    pub(crate) fn local(&self, point: Vec3) -> math::Vec3 {
        let relative = point - self.shown_focus;
        math::Vec3::new(relative.x as f32, relative.y as f32, relative.z as f32)
    }

    pub(crate) fn outward(&self) -> Vec3 {
        let (sin, cos) = self.yaw.sin_cos();
        Vec3::new(cos, 0.0, -sin)
    }

    fn on_the_focus_plane(&self, pixel: math::Vec2, window: math::UVec2) -> Option<Vec3> {
        let ray = self.engine_camera().ray_through(pixel, window);
        let plane = Plane {
            point: math::Vec3::ZERO,
            normal: math::Vec3::Y,
        };
        let at = ray.at(ray.hit_plane(plane)?);
        Some(self.shown_focus + Vec3::new(f64::from(at.x), 0.0, f64::from(at.z)))
    }

    fn focus_outward(&self) -> Vec3 {
        let radius = self.focus.x.hypot(self.focus.z);
        match radius > 0.0 {
            true => Vec3::new(self.focus.x / radius, 0.0, self.focus.z / radius),
            false => self.outward(),
        }
    }

    fn radial_share(&self, outward_meters: f64) -> f64 {
        let radius = self.focus.x.hypot(self.focus.z);
        let (floor, ceiling) = (self.focus_floor_radius(), self.focus_ceiling_radius());
        match outward_meters < 0.0 {
            true => ((radius - floor) / (self.inner_radius - floor)).clamp(0.0, 1.0),
            false => ((ceiling - radius) / (ceiling - self.outer_radius)).clamp(0.0, 1.0),
        }
    }

    fn within_the_belt(&self, focus: Vec3) -> Vec3 {
        let radius = focus.x.hypot(focus.z);
        let held = radius.clamp(self.focus_floor_radius(), self.focus_ceiling_radius());
        match radius > 0.0 {
            true => Vec3::new(focus.x / radius * held, focus.y, focus.z / radius * held),
            false => focus,
        }
    }

    fn focus_floor_radius(&self) -> f64 {
        FOCUS_FLOOR_SHARE_OF_INNER_RADIUS * self.inner_radius
    }

    fn focus_ceiling_radius(&self) -> f64 {
        self.outer_radius * (1.0 + FOCUS_MARGIN_SHARE_OF_OUTER_RADIUS)
    }

    fn face_the_star(&mut self) {
        let Vec3 { x, z, .. } = self.shown_focus;
        if x != 0.0 || z != 0.0 {
            self.yaw = (-z).atan2(x);
        }
    }

    fn eye(&self) -> Vec3 {
        let (rise, run) = Self::TILT_DEGREES.to_radians().sin_cos();
        self.shown_focus + (Vec3::new(0.0, rise, 0.0) + self.outward() * run) * self.shown_distance
    }

    fn within_zoom_range(&self, distance: f64) -> f64 {
        distance.clamp(Self::NEAREST_ZOOM, self.farthest_zoom())
    }

    fn clip_range(&self) -> Range<f32> {
        (Self::NEAREST_ZOOM / 10.0) as f32..(self.farthest_zoom() * 2.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use neumannarch_sim::belt::Belt;

    use super::*;
    use crate::display::viewport::Viewport;

    const GRAVITY: Gravity = Gravity::new(1e9);

    const WINDOW: math::UVec2 = math::UVec2::new(1280, 720);

    const SQUARE_WINDOW: math::UVec2 = math::UVec2::new(900, 900);

    const INNER: f64 = 14_000.0;

    const OUTER: f64 = 25_000.0;

    fn over_the_belt() -> BeltCamera {
        BeltCamera::new(Vec3::new(20_000.0, 0.0, -2_000.0), 400.0, INNER, OUTER)
    }

    fn star_on_screen(camera: &BeltCamera) -> math::Vec2 {
        let viewport = Viewport::of(camera, WINDOW, 1.0);
        let star = viewport
            .pixel_of(Vec3::ZERO)
            .expect("the star is on screen");
        let focus = viewport
            .pixel_of(camera.shown_focus())
            .expect("the focus is on screen");
        (star - focus).normalize_or_zero()
    }

    fn on_screen(pixel: math::Vec2, window: math::UVec2) -> bool {
        (0.0..=window.x as f32).contains(&pixel.x) && (0.0..=window.y as f32).contains(&pixel.y)
    }

    #[test]
    fn a_quarter_period_turns_the_focus_a_quarter_turn_prograde() {
        let radius = INNER;
        let period = TAU * (radius.powi(3) / GRAVITY.mu()).sqrt();
        let start = Vec3::new(radius, 0.0, 0.0);
        let mut camera = BeltCamera::new(start, 50.0, INNER, OUTER);

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
    fn a_focus_inside_the_belt_turns_no_faster_than_the_inner_edge() {
        let period = TAU * (INNER.powi(3) / GRAVITY.mu()).sqrt();
        let mut camera = BeltCamera::new(Vec3::new(0.1 * INNER, 0.0, 0.0), 50.0, INNER, OUTER);

        camera.advance(period / 4.0, GRAVITY);

        let turned = camera.focus();
        assert!(
            turned.distance(Vec3::new(0.0, 0.0, -0.1 * INNER)) <= 1e-6 * INNER,
            "a focus near the star turned to {turned:?} in a quarter of the inner edge's period"
        );
    }

    #[test]
    fn the_star_holds_its_screen_direction_while_the_focus_rides_a_quarter_turn() {
        let period = TAU * (OUTER.powi(3) / GRAVITY.mu()).sqrt();
        let mut camera = BeltCamera::new(Vec3::new(OUTER, 0.0, 0.0), 2_000.0, INNER, OUTER);
        let opening = star_on_screen(&camera);
        assert!(opening.y < -0.9, "the star stands up-screen, not {opening}");

        for _ in 0..8 {
            camera.advance(period / 32.0, GRAVITY);
            let now = star_on_screen(&camera);
            assert!(
                (now - opening).length() < 0.01,
                "the star swung from {opening} to {now}"
            );
        }
    }

    #[test]
    fn no_pan_puts_the_focus_on_the_star() {
        let mut camera = over_the_belt();
        let floor = FOCUS_FLOOR_SHARE_OF_INNER_RADIUS * INNER;

        for _ in 0..64 {
            camera.pan(Vec3::new(-1e6, 0.0, 0.0));
            camera.pan(Vec3::new(-1e3, 0.0, 700.0));
            let radius = camera.focus().x.hypot(camera.focus().z);
            assert!(radius >= floor, "a pan reached {radius} of the star");
        }
    }

    #[test]
    fn no_pan_carries_the_focus_past_the_belts_outer_margin() {
        let mut camera = over_the_belt();
        let ceiling = OUTER * (1.0 + FOCUS_MARGIN_SHARE_OF_OUTER_RADIUS);

        for _ in 0..64 {
            camera.pan(Vec3::new(1e6, 0.0, 0.0));
            let radius = camera.focus().x.hypot(camera.focus().z);
            assert!(
                radius <= ceiling * (1.0 + 1e-9),
                "a pan reached {radius} past {ceiling}"
            );
        }
    }

    #[test]
    fn a_new_focus_and_zoom_are_shown_only_as_the_camera_settles_toward_them() {
        let mut camera = over_the_belt();
        let start = camera.focus();
        let far = start + Vec3::new(1000.0, 0.0, 0.0);

        camera.set_focus(far);
        camera.zoom(2.0);
        assert_eq!(camera.focus(), far, "the target is taken at once");
        assert_eq!(camera.shown_focus(), start, "and shown only as it settles");

        camera.settle(Span::Slow.seconds() / 2.0);
        assert!(
            camera
                .shown_focus()
                .distance(start + Vec3::new(500.0, 0.0, 0.0))
                < 1e-6,
            "half a span shows half the way"
        );
        assert!((camera.shown_distance - 600.0).abs() < 1e-6);

        camera.settle(Span::Slow.seconds());
        assert_eq!(camera.shown_focus(), far);
        assert_eq!(camera.shown_distance, camera.distance());
    }

    #[test]
    fn pan_shifts_the_focus_by_the_delta() {
        let mut camera = over_the_belt();
        let before = camera.focus();

        camera.pan(Vec3::new(5.0, 0.0, -7.0));

        assert!(
            camera.focus().distance(before + Vec3::new(5.0, 0.0, -7.0)) < 1e-9,
            "{:?} is not {:?} panned",
            camera.focus(),
            before
        );
    }

    #[test]
    fn a_pointer_drag_keeps_the_belt_under_the_pointer() {
        let held = over_the_belt().focus();
        let delta = math::Vec2::new(-60.0, 25.0);

        for zoom in [1.0, 8.0] {
            let mut camera = over_the_belt();
            camera.zoom(zoom);
            camera.settle(Span::Slow.seconds());
            let was = Viewport::of(&camera, WINDOW, 1.0)
                .pixel_of(held)
                .expect("the point is on screen");

            camera.pan_by_pointer(was, was + delta, WINDOW);
            camera.settle(Span::Slow.seconds());

            let now = Viewport::of(&camera, WINDOW, 1.0)
                .pixel_of(held)
                .expect("the point stays on screen");
            assert!(
                (now - was - delta).length() <= 0.02 * delta.length(),
                "at {zoom} the point moved from {was} to {now}, not by {delta}"
            );
        }
    }

    #[test]
    fn zoom_clamps_to_its_range() {
        let mut camera = over_the_belt();

        camera.zoom(1e12);
        assert_eq!(camera.distance(), camera.farthest_zoom());

        camera.zoom(0.0);
        assert_eq!(camera.distance(), BeltCamera::NEAREST_ZOOM);
    }

    #[test]
    fn the_widest_zoom_frames_every_asteroid_of_every_seed() {
        let camera = BeltCamera::framing(Belt::inner_radius_meters(), Belt::OUTER_RADIUS_METERS);

        for window in [WINDOW, SQUARE_WINDOW] {
            let viewport = Viewport::of(&camera, window, 1.0);
            for seed in 0..8 {
                for asteroid in Belt::from_seed(seed) {
                    let at = asteroid
                        .orbit()
                        .at(neumannarch_sim::Time::ZERO, Belt::GRAVITY)
                        .pos;
                    let pixel = viewport.pixel_of(at).expect("the asteroid is drawn");
                    assert!(
                        on_screen(pixel, window),
                        "seed {seed} draws an asteroid at {pixel} outside {window}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_widest_zoom_clips_past_the_far_edge_of_the_belt() {
        let camera = BeltCamera::framing(INNER, OUTER);
        let far = f64::from(camera.engine_camera().projection().far());

        assert!(
            far > camera.distance() + 2.0 * OUTER,
            "the far clip {far} does not reach past the belt"
        );
    }
}
