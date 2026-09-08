use mirage_engine::Camera;
use mirage_engine::egui::{self, Pos2};
use mirage_engine::math::{self, UVec2, Vec2};
use neumannarch_sim::Vec3;

use crate::display::camera::BeltCamera;

#[derive(Clone, Debug)]
pub struct Viewport {
    camera: Camera,
    focus: Vec3,
    window: UVec2,
    points_per_pixel: f32,
}

impl Viewport {
    pub fn of(camera: &BeltCamera, window: UVec2, points_per_pixel: f32) -> Viewport {
        Viewport {
            camera: camera.engine_camera(),
            focus: camera.shown_focus(),
            window,
            points_per_pixel,
        }
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    pub fn window(&self) -> UVec2 {
        self.window
    }

    pub fn bounds(&self) -> egui::Rect {
        crate::screens::panel::window_of(self.window, self.points_per_pixel)
    }

    pub(crate) fn local(&self, world: Vec3) -> math::Vec3 {
        let relative = world - self.focus;
        math::Vec3::new(relative.x as f32, relative.y as f32, relative.z as f32)
    }

    pub fn pixel_of(&self, world: Vec3) -> Option<Vec2> {
        self.camera.pixel_of(self.local(world), self.window)
    }

    pub fn point_at(&self, pixel: Vec2) -> Pos2 {
        egui::pos2(pixel.x, pixel.y) * self.points_per_pixel
    }

    pub fn point_of(&self, world: Vec3) -> Option<Pos2> {
        self.pixel_of(world).map(|pixel| self.point_at(pixel))
    }

    pub fn pixels_per_meter(&self, world: Vec3) -> Option<f32> {
        self.camera.pixels_per_meter(self.local(world), self.window)
    }

    pub fn meters_per_point(&self, world: Vec3) -> Option<f32> {
        self.pixels_per_meter(world)
            .map(|pixels_per_meter| 1.0 / (self.points_per_pixel * pixels_per_meter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: UVec2 = UVec2::new(1280, 720);

    const FAR: Vec3 = Vec3::new(1e7, 0.0, 0.0);

    const INNER: f64 = 14_000.0;

    const OUTER: f64 = 25_000.0;

    fn over_the_belt() -> BeltCamera {
        BeltCamera::new(Vec3::new(3000.0, 0.0, -2000.0), 400.0, INNER, OUTER)
    }

    fn viewport(camera: &BeltCamera) -> Viewport {
        Viewport::of(camera, WINDOW, 1.0)
    }

    fn centre() -> Vec2 {
        WINDOW.as_vec2() / 2.0
    }

    #[test]
    fn local_at_the_focus_is_zero() {
        let camera = BeltCamera::new(FAR, 400.0, INNER, OUTER);
        let viewport = viewport(&camera);

        assert_eq!(viewport.local(camera.focus()), math::Vec3::ZERO);
    }

    #[test]
    fn a_tenth_of_a_metre_shift_moves_the_projection_by_less_than_a_tenth_of_a_pixel() {
        let apart = Vec3::new(2000.0, 0.0, 0.0);
        let step = Vec3::new(0.1, 0.0, 0.0);
        let camera = BeltCamera::new(FAR, 8000.0, INNER, OUTER);
        let shifted = BeltCamera::new(FAR + step, 8000.0, INNER, OUTER);

        let before = viewport(&camera).pixel_of(FAR + apart).expect("on screen");
        let after = viewport(&shifted)
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
        let far = BeltCamera::new(FAR, 8000.0, INNER, OUTER);
        let near = BeltCamera::new(Vec3::ZERO, 8000.0, INNER, OUTER);

        let from_far = viewport(&far).pixel_of(FAR + apart).expect("on screen");
        let from_near = viewport(&near).pixel_of(apart).expect("on screen");

        assert!(
            from_far.distance(from_near) <= 1.0,
            "{from_far} at 1e7 m vs {from_near} at the origin, same offset from the focus"
        );
    }

    #[test]
    fn the_focus_lands_on_the_window_centre() {
        let camera = over_the_belt();

        let Some(pixel) = viewport(&camera).pixel_of(camera.focus()) else {
            panic!("the focus is in front of the eye");
        };

        assert!(pixel.distance(centre()) <= 1.0, "{pixel}");
    }

    #[test]
    fn a_point_above_the_focus_draws_above_the_centre() {
        let camera = over_the_belt();
        let above = camera.focus() + Vec3::new(0.0, 100.0, 0.0);

        let Some(pixel) = viewport(&camera).pixel_of(above) else {
            panic!("a point above the focus is in front of the eye");
        };

        assert!(pixel.y < centre().y, "{pixel}");
    }

    #[test]
    fn a_point_behind_the_eye_lands_nowhere() {
        let camera = BeltCamera::new(Vec3::ZERO, 100.0, INNER, OUTER);
        let behind = camera.outward() * (10.0 * camera.distance());

        assert_eq!(viewport(&camera).pixel_of(behind), None);
        assert_eq!(viewport(&camera).point_of(behind), None);
        assert_eq!(viewport(&camera).pixels_per_meter(behind), None);
        assert_eq!(viewport(&camera).meters_per_point(behind), None);
    }

    #[test]
    fn the_scale_falls_with_depth() {
        let camera = over_the_belt();
        let viewport = viewport(&camera);
        let Some(near) = viewport.pixels_per_meter(camera.focus()) else {
            panic!("the focus is in front of the eye");
        };
        let farther = camera.focus() - camera.outward() * camera.distance();

        let Some(far) = viewport.pixels_per_meter(farther) else {
            panic!("farther along the plane is still in front of the eye");
        };

        assert!(near > 0.0, "{near}");
        assert!(far < near, "{far} farther away is not under {near}");
    }

    #[test]
    fn a_point_scales_with_the_painters_own_measure() {
        let camera = over_the_belt();
        let half = Viewport::of(&camera, WINDOW, 0.5);

        let pixel = half.pixel_of(camera.focus()).expect("the focus");
        let point = half.point_of(camera.focus()).expect("the focus");

        assert!((point.x - pixel.x * 0.5).abs() <= 1e-3, "{point}");
        assert!((point.y - pixel.y * 0.5).abs() <= 1e-3, "{point}");
    }
}
