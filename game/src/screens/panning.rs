//! The pan and zoom of the belt, read one way by every screen that draws
//! it.

use mirage_engine::math::{UVec2, Vec2};
use mirage_engine::prelude::{FrameCtx, Game};

use crate::controls::{Axis, Axis2, Button, Controls};
use crate::display::camera::BeltCamera;

/// How fast the pan keys move the belt, in points a second.
const KEY_PAN: f32 = 900.0;

/// What one notch of the zoom axis multiplies the eye-to-focus distance
/// by.
const ZOOM_STEP: f64 = 1.25;

/// The belt under the pointer and the keys: the drag, the pan keys and the
/// zoom axis, held between frames by the pointer they are measured from.
pub struct Panning {
    /// Where the pointer stood last frame, in physical pixels.
    pointer: Vec2,
}

impl Panning {
    /// A belt nobody has dragged yet.
    pub fn still() -> Panning {
        Panning {
            pointer: Vec2::ZERO,
        }
    }

    /// Moves `camera` by this frame's drag, pan keys and zoom notches over
    /// a `window` of physical pixels, and answers the notches it did not
    /// spend, which a send in progress takes instead of a zoom.
    pub fn drag<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        camera: &mut BeltCamera,
        window: UVec2,
        zooming: bool,
    ) -> f32 {
        let pointer = ctx.pointer();
        let moved = pointer - self.pointer;
        self.pointer = pointer;

        if ctx.down(Button::Pan) {
            camera.pan_by_pixels(moved, window);
        }
        let keys = ctx.axis2(Axis2::Pan);
        if keys != Vec2::ZERO {
            let dt = ctx.dt().as_secs_f32();
            camera.pan_by_pixels(Vec2::new(-keys.x, keys.y) * KEY_PAN * dt, window);
        }

        let notches = ctx.axis(Axis::Zoom);
        if zooming && notches != 0.0 {
            camera.zoom(ZOOM_STEP.powf(f64::from(-notches)));
            return 0.0;
        }
        notches
    }
}
