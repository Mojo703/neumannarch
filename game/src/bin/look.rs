use std::fs;
use std::path::Path;

use mirage_engine::headless::Session;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use probe_game::display::camera::BeltCamera;
use probe_game::display::glyph::Glyph;
use probe_game::display::glyph_quad::GlyphQuad;
use probe_game::display::scene::{
    Arc, EntityView, Fill, FlightLine, Mark, Reason, RingView, RockView, Run, Scene,
};
use probe_game::display::viewport::Viewport;
use probe_game::display::{belt, hud};
use probe_sim::roster::{FRIGATE, LANCER, RAIDER, Roster, SHIPYARD};
use probe_sim::{Materials, RockId, RowId, SeatId, Vec3};

meshes! { enum Shape { Sphere, GlyphQuad } }

const WINDOW: UVec2 = UVec2::new(1280, 720);

fn glyph_of(row: RowId) -> Glyph {
    Glyph::of(&Roster::shipped()[row])
}

fn main() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("look");
    fs::create_dir_all(&out).expect("game/look is writable");

    for (name, scene, camera) in [
        ("region", region_scene(), region_camera()),
        ("fight", fight_scene(), fight_camera()),
        ("belt", belt_scene(), belt_camera()),
    ] {
        let pixels = render(scene, camera);
        save(&out.join(format!("{name}.png")), &pixels);
    }
}

struct Looker {
    scene: Scene,
    camera: BeltCamera,
}

impl Game for Looker {
    type Actions = NoActions;
    type Meshes = Shape;
    type Sound = NoSound;
    type Sources = NoSources;
    type Styles = ();

    fn tick(&mut self, _ctx: &mut TickCtx<'_, Self>) {}

    fn frame(&mut self, ctx: &mut FrameCtx<'_, Self>) {
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let viewport = Viewport::of(&self.camera, ctx.window_size(), points_per_pixel);

        belt::draw(&self.scene, &viewport, ctx);

        let scene = &self.scene;
        ctx.ui(|ui| hud::paint(scene, &viewport, None, ui.painter()));
    }
}

fn render(scene: Scene, camera: BeltCamera) -> Vec<u8> {
    let mut session = Session::<Looker>::new(
        Config::new("probe-look").with_tick_interval(probe_sim::TICK),
        WINDOW,
        |_ctx| Ok(Looker { scene, camera }),
    )
    .expect("the offscreen session starts");

    session.step();
    session.step();
    session.pixels().expect("the target reads back")
}

fn save(path: &Path, pixels: &[u8]) {
    image::RgbaImage::from_raw(WINDOW.x, WINDOW.y, pixels.to_vec())
        .expect("the pixels match the window's size")
        .save(path)
        .expect("the PNG writes");
}

fn rock(id: u32, pos: Vec3, radius: f64) -> RockView {
    RockView {
        id: RockId(id),
        pos,
        radius,
        caps: caps_of(id),
    }
}

fn caps_of(id: u32) -> Materials {
    let rich = 8.0;
    let poor = 1.0;
    match id % 3 {
        0 => Materials::new(rich, poor, poor),
        1 => Materials::new(poor, rich, poor),
        _ => Materials::new(poor, poor, rich),
    }
}

fn ring(rock: RockId, runs: Vec<Run>, arcs: Vec<Arc>) -> RingView {
    RingView {
        rock,
        caps: caps_of(rock.0),
        runs,
        arcs,
    }
}

fn ship(seat: u8, row: RowId, pos: Vec3) -> EntityView {
    EntityView {
        seat: SeatId(seat),
        glyph: glyph_of(row),
        pos,
    }
}

fn present(seat: u8, rows: &[RowId]) -> Run {
    Run {
        seat: SeatId(seat),
        marks: rows
            .iter()
            .map(|&row| Mark {
                glyph: glyph_of(row),
                fill: Fill::Solid,
                dim: false,
                reason: Reason::Here(row),
            })
            .collect(),
    }
}

fn region_scene() -> Scene {
    let rocks = vec![
        rock(0, Vec3::new(0.0, 0.0, 0.0), 6.0),
        rock(1, Vec3::new(60.0, 0.0, -20.0), 5.0),
        rock(2, Vec3::new(-50.0, 0.0, 40.0), 4.0),
    ];

    let inner_a = RockId(0);
    let inner_b = RockId(1);
    let inner_c = RockId(2);

    let entities = vec![
        ship(0, RAIDER, Vec3::new(4.0, 0.0, 2.0)),
        ship(0, FRIGATE, Vec3::new(-4.0, 0.0, 3.0)),
        ship(1, RAIDER, Vec3::new(2.0, 0.0, -4.0)),
        ship(1, LANCER, Vec3::new(-2.0, 0.0, -5.0)),
        ship(0, SHIPYARD, Vec3::new(60.0, 0.0, -14.0)),
        ship(0, FRIGATE, Vec3::new(65.0, 0.0, -22.0)),
        ship(1, RAIDER, Vec3::new(-15.0, 0.0, 15.0)),
    ];

    let rings = vec![
        ring(
            inner_a,
            vec![
                present(0, &[FRIGATE, RAIDER]),
                present(1, &[LANCER, RAIDER]),
            ],
            vec![
                Arc {
                    seat: SeatId(0),
                    fraction: 0.7,
                    trailing: 0.85,
                },
                Arc {
                    seat: SeatId(1),
                    fraction: 0.4,
                    trailing: 0.4,
                },
            ],
        ),
        ring(inner_b, vec![present(0, &[SHIPYARD, FRIGATE])], vec![]),
        ring(inner_c, vec![], vec![]),
    ];

    let flights = vec![FlightLine {
        from: Vec3::new(-15.0, 0.0, 15.0),
        to: RockId(2),
    }];

    Scene {
        rocks,
        entities,
        rings,
        flights,
        selection: None,
        hover: None,
    }
}

fn region_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(3.0, 0.0, 7.0), 140.0)
}

fn fight_scene() -> Scene {
    let rocks = vec![rock(0, Vec3::ZERO, 6.0)];
    let inner = RockId(0);

    let entities = vec![
        ship(0, FRIGATE, Vec3::new(5.0, 0.0, 1.0)),
        ship(0, LANCER, Vec3::new(6.0, 0.0, -2.0)),
        ship(0, RAIDER, Vec3::new(4.0, 0.0, 4.0)),
        ship(1, FRIGATE, Vec3::new(-5.0, 0.0, 1.0)),
        ship(1, RAIDER, Vec3::new(-6.0, 0.0, -2.0)),
    ];

    let rings = vec![ring(
        inner,
        vec![
            present(0, &[FRIGATE, LANCER, RAIDER]),
            present(1, &[FRIGATE, RAIDER]),
        ],
        vec![
            Arc {
                seat: SeatId(0),
                fraction: 0.85,
                trailing: 0.9,
            },
            Arc {
                seat: SeatId(1),
                fraction: 0.3,
                trailing: 0.55,
            },
        ],
    )];

    Scene {
        rocks,
        entities,
        rings,
        flights: vec![],
        selection: Some(inner),
        hover: None,
    }
}

fn fight_camera() -> BeltCamera {
    BeltCamera::new(Vec3::ZERO, 40.0)
}

fn belt_scene() -> Scene {
    let positions = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(140.0, 0.0, 40.0),
        Vec3::new(-120.0, 0.0, 80.0),
        Vec3::new(60.0, 0.0, -150.0),
        Vec3::new(-90.0, 0.0, -100.0),
    ];
    let rocks: Vec<RockView> = positions
        .iter()
        .enumerate()
        .map(|(index, &pos)| rock(index as u32, pos, 6.0))
        .collect();

    let seats = [0, 1, 0, 1, 0];
    let flight_from = Vec3::new(70.0, 0.0, 20.0);
    let mut entities: Vec<EntityView> = positions
        .iter()
        .zip(seats)
        .map(|(&pos, seat)| ship(seat, FRIGATE, pos + Vec3::new(4.0, 0.0, 0.0)))
        .collect();
    entities.push(ship(1, RAIDER, flight_from));

    let rings: Vec<RingView> = positions
        .iter()
        .zip(seats)
        .enumerate()
        .map(|(index, (_, seat))| {
            ring(
                RockId(index as u32),
                vec![present(seat, &[FRIGATE])],
                vec![],
            )
        })
        .collect();

    Scene {
        rocks,
        entities,
        rings,
        flights: vec![FlightLine {
            from: flight_from,
            to: RockId(1),
        }],
        selection: None,
        hover: None,
    }
}

fn belt_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(0.0, 0.0, -20.0), 420.0)
}
