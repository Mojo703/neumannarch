use mirage_engine::math::{UVec2, Vec2};
use mirage_engine::prelude::{FrameCtx, Game};

use crate::controls::{Axis, Axis2, Button, Controls};
use crate::display::camera::BeltCamera;

const KEY_PAN: f32 = 900.0;

const ZOOM_STEP: f64 = 1.25;

pub struct Panning {
    pointer: Vec2,
}

impl Panning {
    pub fn still() -> Panning {
        Panning {
            pointer: Vec2::ZERO,
        }
    }

    pub fn drag<G: Game<Actions = Controls>>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        camera: &mut BeltCamera,
        window: UVec2,
        zooming: bool,
    ) -> f32 {
        let pointer = ctx.pointer();
        let was = self.pointer;
        self.pointer = pointer;

        if ctx.down(Button::Pan) {
            camera.pan_by_pointer(was, pointer, window);
        }
        let keys = ctx.axis2(Axis2::Pan);
        if keys != Vec2::ZERO {
            let dt = ctx.dt().as_secs_f32();
            let middle = window.as_vec2() / 2.0;
            camera.pan_by_pointer(
                middle,
                middle + Vec2::new(-keys.x, keys.y) * KEY_PAN * dt,
                window,
            );
        }

        let notches = ctx.axis(Axis::Zoom);
        if zooming && notches != 0.0 {
            camera.zoom(ZOOM_STEP.powf(f64::from(-notches)));
            return 0.0;
        }
        notches
    }
}
