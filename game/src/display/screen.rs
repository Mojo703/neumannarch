//! One frame's projection: the engine camera built once, the surface it
//! draws over, and the two scales a fixed-screen-size draw needs.

use mirage_engine::Camera;
use mirage_engine::egui::{self, Pos2};
use mirage_engine::math::{self, UVec2, Vec2};
use probe_sim::Vec3;

use crate::display::camera::BeltCamera;

/// Where world points land this frame. Built once per frame, since every
/// projection would otherwise rebuild the camera.
#[derive(Clone, Debug)]
pub struct Screen {
    camera: Camera,
    focus: Vec3,
    window: UVec2,
    points_per_pixel: f32,
}

impl Screen {
    /// The projection `camera` draws with over a `window`-physical-pixel
    /// surface. `points_per_pixel` is the painter's own, the inverse of
    /// `egui::Context::pixels_per_point`.
    pub fn of(camera: &BeltCamera, window: UVec2, points_per_pixel: f32) -> Screen {
        Screen {
            camera: camera.engine_camera(),
            focus: camera.focus(),
            window,
            points_per_pixel,
        }
    }

    /// The engine camera this frame draws with.
    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// The surface, in physical pixels.
    pub fn window(&self) -> UVec2 {
        self.window
    }

    /// `world`, in sim meters, relative to this frame's focus, as the
    /// engine's `f32` vector.
    ///
    /// The engine never sees an absolute coordinate: every draw and every
    /// projection goes through this, so a point far from the origin loses
    /// none of `f32`'s resolution to the distance already crossed to reach
    /// it.
    pub(crate) fn local(&self, world: Vec3) -> math::Vec3 {
        let relative = world - self.focus;
        math::Vec3::new(relative.x as f32, relative.y as f32, relative.z as f32)
    }

    /// The physical pixel `world`, in meters, lands on; `None` behind the
    /// eye.
    pub fn pixel_of(&self, world: Vec3) -> Option<Vec2> {
        self.camera.pixel_of(self.local(world), self.window)
    }

    /// `pixel`, in physical pixels, in the painter's own measure: what the
    /// pointer's position is compared against a painted shape as.
    pub fn point_at(&self, pixel: Vec2) -> Pos2 {
        egui::pos2(pixel.x, pixel.y) * self.points_per_pixel
    }

    /// The point `world`, in meters, lands on, in the painter's own
    /// measure; `None` behind the eye.
    pub fn point_of(&self, world: Vec3) -> Option<Pos2> {
        self.pixel_of(world).map(|pixel| self.point_at(pixel))
    }

    /// The physical pixels one meter covers at `world`'s depth; `None`
    /// behind the eye.
    pub fn pixels_per_meter(&self, world: Vec3) -> Option<f32> {
        self.camera.pixels_per_meter(self.local(world), self.window)
    }

    /// The meters one painted point covers at `world`'s depth: what a draw
    /// of a fixed screen size scales by; `None` behind the eye.
    pub fn meters_per_point(&self, world: Vec3) -> Option<f32> {
        self.pixels_per_meter(world)
            .map(|pixels_per_meter| 1.0 / (self.points_per_pixel * pixels_per_meter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window every pixel is taken over, in physical pixels.
    const WINDOW: UVec2 = UVec2::new(1280, 720);

    /// A point 1e7 meters from the origin: where the belt's rocks orbit,
    /// far enough out that `f32` alone loses a meter of resolution.
    const FAR: Vec3 = Vec3::new(1e7, 0.0, 0.0);

    fn over_the_belt() -> BeltCamera {
        BeltCamera::new(Vec3::new(3000.0, 0.0, -2000.0), 400.0)
    }

    fn screen(camera: &BeltCamera) -> Screen {
        Screen::of(camera, WINDOW, 1.0)
    }

    fn centre() -> Vec2 {
        WINDOW.as_vec2() / 2.0
    }

    #[test]
    fn local_at_the_focus_is_zero() {
        let camera = BeltCamera::new(FAR, 400.0);
        let screen = screen(&camera);

        assert_eq!(screen.local(camera.focus()), math::Vec3::ZERO);
    }

    #[test]
    fn a_tenth_of_a_metre_shift_moves_the_projection_by_less_than_a_tenth_of_a_pixel() {
        let apart = Vec3::new(2000.0, 0.0, 0.0);
        let step = Vec3::new(0.1, 0.0, 0.0);
        let camera = BeltCamera::new(FAR, 8000.0);
        let shifted = BeltCamera::new(FAR + step, 8000.0);

        let before = screen(&camera).pixel_of(FAR + apart).expect("on screen");
        let after = screen(&shifted)
            .pixel_of(FAR + apart + step)
            .expect("on screen");

        assert!(
            before.distance(after) < 0.1,
            "{before} moved to {after} for a {step:?} shared shift"
        );
    }

    #[test]
    fn absolute_position_does_not_move_the_projection() {
        let apart = Vec3::new(2000.0, 0.0, 0.0);
        let far = BeltCamera::new(FAR, 8000.0);
        let near = BeltCamera::new(Vec3::ZERO, 8000.0);

        let from_far = screen(&far).pixel_of(FAR + apart).expect("on screen");
        let from_near = screen(&near).pixel_of(apart).expect("on screen");

        assert!(
            from_far.distance(from_near) <= 1.0,
            "{from_far} at 1e7 m vs {from_near} at the origin, same offset from the focus"
        );
    }

    #[test]
    fn the_focus_lands_on_the_window_centre() {
        let camera = over_the_belt();

        let Some(pixel) = screen(&camera).pixel_of(camera.focus()) else {
            panic!("the focus is in front of the eye");
        };

        assert!(pixel.distance(centre()) <= 1.0, "{pixel}");
    }

    #[test]
    fn a_point_above_the_focus_draws_above_the_centre() {
        let camera = over_the_belt();
        let above = camera.focus() + Vec3::new(0.0, 100.0, 0.0);

        let Some(pixel) = screen(&camera).pixel_of(above) else {
            panic!("a point above the focus is in front of the eye");
        };

        assert!(pixel.y < centre().y, "{pixel}");
    }

    #[test]
    fn a_point_behind_the_eye_lands_nowhere() {
        let camera = BeltCamera::new(Vec3::ZERO, 100.0);
        let behind = Vec3::new(0.0, 0.0, 10.0 * camera.distance());

        assert_eq!(screen(&camera).pixel_of(behind), None);
        assert_eq!(screen(&camera).point_of(behind), None);
        assert_eq!(screen(&camera).pixels_per_meter(behind), None);
        assert_eq!(screen(&camera).meters_per_point(behind), None);
    }

    #[test]
    fn the_scale_falls_with_depth() {
        let camera = over_the_belt();
        let screen = screen(&camera);
        let Some(near) = screen.pixels_per_meter(camera.focus()) else {
            panic!("the focus is in front of the eye");
        };
        let farther = camera.focus() + Vec3::new(0.0, 0.0, -camera.distance());

        let Some(far) = screen.pixels_per_meter(farther) else {
            panic!("farther along the plane is still in front of the eye");
        };

        assert!(near > 0.0, "{near}");
        assert!(far < near, "{far} farther away is not under {near}");
    }

    #[test]
    fn a_point_scales_with_the_painters_own_measure() {
        let camera = over_the_belt();
        let half = Screen::of(&camera, WINDOW, 0.5);

        let pixel = half.pixel_of(camera.focus()).expect("the focus");
        let point = half.point_of(camera.focus()).expect("the focus");

        assert!((point.x - pixel.x * 0.5).abs() <= 1e-3, "{point}");
        assert!((point.y - pixel.y * 0.5).abs() <= 1e-3, "{point}");
    }
}
