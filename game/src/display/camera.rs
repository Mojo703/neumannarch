//! The belt camera: a focus point moving at the local circular orbital
//! velocity, seen from a fixed tilt, with pan and zoom.

use core::ops::RangeInclusive;

use mirage_engine::math;
use mirage_engine::{Camera, Projection, View};
use probe_sim::Vec3;
use probe_sim::orbit::Gravity;

/// The camera over the belt: it looks at a focus point from a fixed tilt,
/// and the focus circles the central mass like a body at its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeltCamera {
    focus: Vec3,
    distance: f64,
}

impl BeltCamera {
    /// The vertical field of view, in degrees.
    pub const FOV_DEGREES: f32 = 50.0;
    /// The eye's elevation above the belt plane, in degrees.
    pub const TILT_DEGREES: f64 = 60.0;
    /// Eye-to-focus distances the camera stays within, in meters.
    ///
    /// The near end shows one band whole: a band's anchor sits tens of
    /// meters ahead of its rock and a held force spreads a few meters
    /// around it. The far end shows a region: rocks of the shipped belt lie
    /// two kilometers apart, so this covers about six of those gaps. Both
    /// ends are hypotheses the play confirms or kills.
    pub const ZOOM_RANGE: RangeInclusive<f64> = 40.0..=12_000.0;

    /// Looks at `focus`, in meters, from `distance` meters, clamped to
    /// [`Self::ZOOM_RANGE`].
    pub fn new(focus: Vec3, distance: f64) -> Self {
        Self {
            focus,
            distance: Self::within_zoom_range(distance),
        }
    }

    /// Moves the focus `dt` seconds prograde along the circle of its
    /// current radius, at the circular speed under `gravity`. A focus at
    /// the origin stays.
    pub fn advance(&mut self, dt: f64, gravity: Gravity) {
        let radius = self.focus.x.hypot(self.focus.z);
        let Some(rate) = circular_rate(radius, gravity) else {
            return;
        };
        let (sin, cos) = (rate * dt).sin_cos();
        let Vec3 { x, y, z } = self.focus;
        self.focus = Vec3::new(x * cos + z * sin, y, z * cos - x * sin);
    }

    /// Shifts the focus by `delta`, in meters in the belt plane.
    pub fn pan(&mut self, delta: Vec3) {
        self.focus += delta;
    }

    /// Shifts the focus so the belt plane moves `delta` physical pixels
    /// under the pointer, over a window of `window` physical pixels.
    ///
    /// The plane is foreshortened by the tilt along the screen's vertical,
    /// so a pixel down the screen is farther across the belt than a pixel
    /// across it. Nothing moves while the focus is behind the eye, which no
    /// zoom of [`Self::ZOOM_RANGE`] reaches.
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

    /// Multiplies the distance by `factor`, clamped to
    /// [`Self::ZOOM_RANGE`].
    pub fn zoom(&mut self, factor: f64) {
        self.distance = Self::within_zoom_range(self.distance * factor);
    }

    /// Looks at `focus`, in meters, from the current distance.
    pub fn set_focus(&mut self, focus: Vec3) {
        self.focus = focus;
    }

    /// The point looked at, in meters.
    pub fn focus(&self) -> Vec3 {
        self.focus
    }

    /// Eye to focus, in meters, within [`Self::ZOOM_RANGE`].
    pub fn distance(&self) -> f64 {
        self.distance
    }

    /// The engine camera drawing this view: the eye at the tilt offset from
    /// the focus, looking at it, both relative to the focus. Build it once
    /// per frame; a [`crate::display::screen::Screen`] holds it for everything that
    /// projects, and converts every other point the same way so the engine
    /// never sees an absolute coordinate.
    pub fn engine_camera(&self) -> Camera {
        Camera::new(
            View::look_at(self.local(self.eye()), math::Vec3::ZERO),
            Projection::perspective(Self::FOV_DEGREES),
        )
    }

    /// `point`, in sim meters, relative to the focus, as the engine's
    /// `f32` vector.
    ///
    /// The engine's `f32` has about a meter of resolution at the belt's
    /// 1e7 m scale; converting relative to the focus, which never strays
    /// far in sim meters from what the frame draws, keeps every draw and
    /// projection well inside `f32`'s precision.
    pub(crate) fn local(&self, point: Vec3) -> math::Vec3 {
        let relative = point - self.focus;
        math::Vec3::new(relative.x as f32, relative.y as f32, relative.z as f32)
    }

    /// Where the eye stands: `distance` from the focus, `TILT_DEGREES`
    /// above the belt plane, toward `+Z`.
    fn eye(&self) -> Vec3 {
        let (rise, run) = Self::TILT_DEGREES.to_radians().sin_cos();
        self.focus + Vec3::new(0.0, rise, run) * self.distance
    }

    fn within_zoom_range(distance: f64) -> f64 {
        distance.clamp(*Self::ZOOM_RANGE.start(), *Self::ZOOM_RANGE.end())
    }
}

/// The angular rate of the circular orbit at `radius` meters under
/// `gravity`, in radians per second; `None` at the origin, which no circle
/// passes.
fn circular_rate(radius: f64, gravity: Gravity) -> Option<f64> {
    (radius > 0.0).then(|| (gravity.mu() / radius.powi(3)).sqrt())
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;

    /// The central mass's gravitational parameter, in m³/s².
    const GRAVITY: Gravity = Gravity::new(1e9);

    /// The window every pixel is taken over, in physical pixels.
    const WINDOW: math::UVec2 = math::UVec2::new(1280, 720);

    /// A camera looking at a point well off the origin.
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
        let screen = crate::display::screen::Screen::of(&camera, WINDOW, 1.0);
        // The point under the pointer is the focus: a perspective view
        // keeps the drag exact at one depth, and that is the depth the pan
        // is scaled at.
        let held = camera.focus();
        let was = screen.pixel_of(held).expect("the point is on screen");
        let delta = math::Vec2::new(-60.0, 25.0);

        let mut dragged = camera;
        dragged.pan_by_pixels(delta, WINDOW);

        let now = crate::display::screen::Screen::of(&dragged, WINDOW, 1.0)
            .pixel_of(held)
            .expect("the point stays on screen");
        // The pan is scaled at the focus's depth, and the drag itself
        // moves the plane to a slightly different depth, so a long drag
        // lands a fraction of itself off.
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
    fn the_widest_zoom_shows_two_rocks_of_the_shipped_belt() {
        let rocks = probe_sim::belt::Belt::fixed(probe_sim::belt::Belt::GRAVITY);
        let body = |at: usize| {
            rocks[at]
                .orbit()
                .at(probe_sim::Tick::ZERO, probe_sim::belt::Belt::GRAVITY)
        };
        let (first, second) = (body(0), body(1));
        let camera = BeltCamera::new(first.pos, *BeltCamera::ZOOM_RANGE.end());
        let screen = crate::display::screen::Screen::of(&camera, WINDOW, 1.0);

        let apart = screen
            .pixel_of(second.pos)
            .expect("a neighbour is in front of the eye")
            .distance(screen.pixel_of(first.pos).expect("the focus"));

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
