//! Probe Game on the Mirage engine: the engine runs at the sim's tick rate
//! and draws the world's origin under a fixed camera.

use mirage_engine::prelude::*;

/// Where the camera stands, in meters from the origin.
const EYE: Vec3 = Vec3::new(0.0, 40.0, 60.0);

/// The camera's vertical field of view, in degrees.
const FOV: f32 = 50.0;

fn main() {
    run(
        Config::new("Probe Game").with_tick_interval(probe_sim::TICK),
        |_| Ok(Probe { ticks: 0 }),
    );
}

meshes! { enum Shape { Cube } }

struct Probe {
    /// Steps the engine's accumulator has run at `probe_sim::TICK`.
    ticks: u64,
}

impl Game for Probe {
    type Meshes = Shape;
    type Sound = NoSound;
    type Sources = NoSources;
    type Actions = NoActions;
    type Styles = ();

    fn tick(&mut self, _: &mut TickCtx<'_, Probe>) {
        self.ticks += 1;
    }

    fn frame(&mut self, ctx: &mut FrameCtx<'_, Probe>) {
        ctx.set_camera(Camera::new(
            View::look_at(EYE, Vec3::ZERO),
            Projection::perspective(FOV),
        ));
        ctx.draw(Cube.at(Vec3::ZERO));
        let ticks = self.ticks;
        ctx.ui(|ui| {
            ui.label(format!("tick {ticks}"));
        });
    }
}
