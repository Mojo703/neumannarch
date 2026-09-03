//! Probe Game's fixed scenes, rendered headlessly to screenshots.
//!
//! Behind the `look` feature (`cargo run -p probe-game --features look
//! --bin look`). DISPLAY.md's "Judging" section names three fixed
//! scenes; this binary builds each by hand, renders it through the
//! engine's offscreen `Session` with the `ui` feature, and writes the
//! pixels to `game/look/<scene>.png`, a directory `.gitignore`d since
//! screenshots are the judgement's input, never committed. The sim is
//! not involved: positions are chosen to make each scene legible at its
//! camera.

use std::fs;
use std::path::Path;

use mirage_engine::headless::Session;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use probe_game::camera::BeltCamera;
use probe_game::glyph::Glyph;
use probe_game::glyph_quad::GlyphQuad;
use probe_game::scene::{Arc, EntityView, Fill, FlightLine, Mark, RingView, RockView, Run, Scene};
use probe_game::screen::Screen;
use probe_game::{belt, hud};
use probe_sim::roster::{FRIGATE, LANCER, RAIDER, Roster, SHIPYARD};
use probe_sim::{Band, Place, RockId, RowId, SeatId, Vec3};

meshes! { enum Shape { Sphere, GlyphQuad } }

/// The offscreen target's size, in physical pixels.
const WINDOW: UVec2 = UVec2::new(1280, 720);

/// `row`'s glyph, by the three rules, over the shipped roster.
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

/// A scene and the camera it is viewed from, drawn once.
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
        // Read before drawing: `points_per_pixel` sizes the belt's ship
        // glyphs to match the HUD's, and the painter that reads it is only
        // reachable through `ctx.ui`.
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let screen = Screen::of(&self.camera, ctx.window_size(), points_per_pixel);

        belt::draw(&self.scene, &screen, ctx);

        let scene = &self.scene;
        ctx.ui(|ui| hud::paint(scene, &screen, None, ui.painter()));
    }
}

/// Renders `scene` from `camera` through an offscreen session and reads
/// the pixels back.
fn render(scene: Scene, camera: BeltCamera) -> Vec<u8> {
    let mut session = Session::<Looker>::new(
        Config::new("probe-look").with_tick_interval(probe_sim::TICK),
        WINDOW,
        |_ctx| Ok(Looker { scene, camera }),
    )
    .expect("the offscreen session starts");

    // The UI layer reads its own claims from the step before; two steps
    // settle the one full-screen layer this binary ever draws.
    session.step();
    session.step();
    session.pixels().expect("the target reads back")
}

/// Writes `pixels`, `WINDOW`-sized `RGBA8`, to `path` as a PNG.
fn save(path: &Path, pixels: &[u8]) {
    image::RgbaImage::from_raw(WINDOW.x, WINDOW.y, pixels.to_vec())
        .expect("the pixels match the window's size")
        .save(path)
        .expect("the PNG writes");
}

/// A rock at `pos`, `radius` meters across, at `id`.
fn rock(id: u32, pos: Vec3, radius: f64) -> RockView {
    RockView {
        id: RockId(id),
        pos,
        radius,
    }
}

/// One unit of `row`, `seat`'s, at `pos`.
fn ship(seat: u8, row: RowId, pos: Vec3) -> EntityView {
    EntityView {
        seat: SeatId(seat),
        glyph: glyph_of(row),
        pos,
    }
}

/// `seat`'s run of `marks`, all present.
fn present(seat: u8, rows: &[RowId]) -> Run {
    Run {
        seat: SeatId(seat),
        marks: rows
            .iter()
            .map(|&row| Mark {
                glyph: glyph_of(row),
                fill: Fill::Solid,
                dim: false,
            })
            .collect(),
    }
}

/// A region with three rocks, mixed seats, a fight and a flight.
fn region_scene() -> Scene {
    let rocks = vec![
        rock(0, Vec3::new(0.0, 0.0, 0.0), 6.0),
        rock(1, Vec3::new(60.0, 0.0, -20.0), 5.0),
        rock(2, Vec3::new(-50.0, 0.0, 40.0), 4.0),
    ];

    let inner_a = Place {
        rock: RockId(0),
        band: Band::Inner,
    };
    let inner_b = Place {
        rock: RockId(1),
        band: Band::Inner,
    };

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
        RingView {
            place: inner_a,
            runs: vec![
                present(0, &[FRIGATE, RAIDER]),
                present(1, &[LANCER, RAIDER]),
            ],
            arcs: vec![
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
        },
        RingView {
            place: inner_b,
            runs: vec![present(0, &[SHIPYARD, FRIGATE])],
            arcs: vec![],
        },
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
        blips: Vec::new(),
        selection: None,
        hover: None,
    }
}

/// The camera over [`region_scene`].
fn region_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(3.0, 0.0, 7.0), 140.0)
}

/// A fight at one rock, two seats engaged.
fn fight_scene() -> Scene {
    let rocks = vec![rock(0, Vec3::ZERO, 6.0)];
    let inner = Place {
        rock: RockId(0),
        band: Band::Inner,
    };

    let entities = vec![
        ship(0, FRIGATE, Vec3::new(5.0, 0.0, 1.0)),
        ship(0, LANCER, Vec3::new(6.0, 0.0, -2.0)),
        ship(0, RAIDER, Vec3::new(4.0, 0.0, 4.0)),
        ship(1, FRIGATE, Vec3::new(-5.0, 0.0, 1.0)),
        ship(1, RAIDER, Vec3::new(-6.0, 0.0, -2.0)),
    ];

    let rings = vec![RingView {
        place: inner,
        runs: vec![
            present(0, &[FRIGATE, LANCER, RAIDER]),
            present(1, &[FRIGATE, RAIDER]),
        ],
        arcs: vec![
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
    }];

    Scene {
        rocks,
        entities,
        rings,
        flights: vec![],
        blips: Vec::new(),
        selection: Some(inner),
        hover: None,
    }
}

/// The camera over [`fight_scene`].
fn fight_camera() -> BeltCamera {
    BeltCamera::new(Vec3::ZERO, 40.0)
}

/// The whole belt: five rocks spread wide, held by mixed seats.
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
        .map(|(index, (_, seat))| RingView {
            place: Place {
                rock: RockId(index as u32),
                band: Band::Inner,
            },
            runs: vec![present(seat, &[FRIGATE])],
            arcs: vec![],
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
        blips: Vec::new(),
        selection: None,
        hover: None,
    }
}

/// The camera over [`belt_scene`].
fn belt_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(0.0, 0.0, -20.0), 420.0)
}
