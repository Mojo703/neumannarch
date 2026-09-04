use std::fs;
use std::path::Path;

use mirage_engine::headless::Session;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use neumannarch_game::display::camera::BeltCamera;
use neumannarch_game::display::glyph::Glyph;
use neumannarch_game::display::glyph_quad::GlyphQuad;
use neumannarch_game::display::scene::{
    Arc, EntityView, Entry, FlightLine, Hover, RockView, RowView, Scene, SectorView, Shown,
    WheelBand, WheelView,
};
use neumannarch_game::display::viewport::Viewport;
use neumannarch_game::display::wheels::{Aim, Still, Wheels};
use neumannarch_game::display::{belt, hud};
use neumannarch_sim::roster::{FRIGATE, LANCER, RAIDER, Roster, SHIPYARD, STORAGE};
use neumannarch_sim::state::view::Building;
use neumannarch_sim::{Material, Materials, RockId, RowId, SeatId, Vec3};

meshes! { enum Shape { Sphere, GlyphQuad } }

const WINDOW: UVec2 = UVec2::new(1280, 720);

const ZONE: f64 = neumannarch_sim::belt::Belt::ZONE_RADIUS_METERS;

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

        let roster = Roster::shipped();
        let wheels = Wheels::over(
            &self.scene,
            &roster,
            &viewport,
            &aim(&self.scene),
            &mut Still,
        );
        let scene = &self.scene;
        ctx.ui(|ui| {
            hud::paint(scene, &viewport, ui.painter());
            wheels.paint(ui.painter(), scene.hover.as_ref());
        });
    }
}

fn aim(scene: &Scene) -> Aim {
    Aim {
        viewer: scene.seat,
        pointer: None,
        hovered: None,
        step: 1,
        wants: [((RockId(0), FRIGATE), 1), ((RockId(0), RAIDER), 1)]
            .into_iter()
            .collect(),
    }
}

fn render(scene: Scene, camera: BeltCamera) -> Vec<u8> {
    let mut session = Session::<Looker>::new(
        Config::new("neumannarch-look").with_tick_interval(neumannarch_sim::TICK),
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

fn ship(seat: u8, row: RowId, pos: Vec3) -> EntityView {
    EntityView {
        seat: SeatId(seat),
        glyph: glyph_of(row),
        pos,
        range: Roster::shipped()[row]
            .is_armed()
            .then(|| Roster::shipped()[row].max_damage_range()),
    }
}

fn flier(seat: u8, row: RowId, pos: Vec3) -> EntityView {
    EntityView {
        range: None,
        ..ship(seat, row, pos)
    }
}

fn shown(entry: Entry) -> Shown {
    Shown {
        entry,
        previewed: false,
    }
}

fn row(row: RowId, entries: Vec<Entry>) -> RowView {
    RowView {
        row,
        entries: entries.into_iter().map(shown).collect(),
    }
}

fn sector(seat: u8, rows: Vec<RowView>, arc: Option<Arc>) -> SectorView {
    SectorView {
        seat: SeatId(seat),
        rows,
        arc,
    }
}

fn wheel(rock: u32, sectors: Vec<SectorView>) -> WheelView {
    WheelView {
        rock: RockId(rock),
        sectors,
    }
}

fn arc(seat: u8, fraction: f32, trailing: f32) -> Option<Arc> {
    Some(Arc {
        seat: SeatId(seat),
        fraction,
        trailing,
    })
}

fn region_scene() -> Scene {
    let rocks = vec![
        rock(0, Vec3::new(0.0, 0.0, 0.0), 6.0),
        rock(1, Vec3::new(320.0, 0.0, -110.0), 5.0),
        rock(2, Vec3::new(-280.0, 0.0, 220.0), 4.0),
    ];

    let entities = vec![
        ship(0, RAIDER, Vec3::new(4.0, 0.0, 2.0)),
        ship(0, FRIGATE, Vec3::new(-4.0, 0.0, 3.0)),
        ship(1, RAIDER, Vec3::new(2.0, 0.0, -4.0)),
        ship(1, LANCER, Vec3::new(-2.0, 0.0, -5.0)),
        ship(0, SHIPYARD, Vec3::new(320.0, 0.0, -104.0)),
        ship(0, FRIGATE, Vec3::new(325.0, 0.0, -112.0)),
        flier(1, RAIDER, Vec3::new(-90.0, 0.0, 80.0)),
    ];

    let wheels = vec![
        wheel(
            0,
            vec![
                sector(
                    0,
                    vec![
                        row(FRIGATE, vec![Entry::Present(1)]),
                        row(RAIDER, vec![Entry::Present(1)]),
                    ],
                    arc(0, 0.7, 0.85),
                ),
                sector(
                    1,
                    Roster::shipped()
                        .iter()
                        .map(|(id, _)| row(id, vec![Entry::Present(1)]))
                        .collect(),
                    arc(1, 0.4, 0.4),
                ),
            ],
        ),
        wheel(
            1,
            vec![sector(
                0,
                vec![
                    row(SHIPYARD, vec![Entry::Present(1), Entry::Surplus(1)]),
                    row(
                        FRIGATE,
                        vec![
                            Entry::Present(1),
                            Entry::Building(Building {
                                progress: 0.45,
                                starved_of: Some(Material::Metals),
                            }),
                            Entry::Wanted {
                                count: 1,
                                dashed: false,
                            },
                        ],
                    ),
                ],
                None,
            )],
        ),
        wheel(
            2,
            vec![sector(
                1,
                vec![row(
                    RAIDER,
                    vec![Entry::Arriving {
                        count: 1,
                        from: RockId(0),
                    }],
                )],
                None,
            )],
        ),
    ];

    Scene {
        rocks,
        entities,
        wheels,
        flights: vec![FlightLine {
            from: Vec3::new(-90.0, 0.0, 80.0),
            to: RockId(2),
        }],
        zone: ZONE,
        seat: SeatId(0),
        selection: Some(RockId(0)),
        hover: Some(Hover::Wheel {
            rock: RockId(0),
            row: RAIDER,
            band: WheelBand::Plus(1),
        }),
    }
}

fn region_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(10.0, 0.0, 30.0), 620.0)
}

fn fight_scene() -> Scene {
    let rocks = vec![rock(0, Vec3::ZERO, 6.0)];

    let entities = vec![
        ship(0, FRIGATE, Vec3::new(5.0, 0.0, 1.0)),
        ship(0, LANCER, Vec3::new(6.0, 0.0, -2.0)),
        ship(0, RAIDER, Vec3::new(4.0, 0.0, 4.0)),
        ship(1, FRIGATE, Vec3::new(-5.0, 0.0, 1.0)),
        ship(1, RAIDER, Vec3::new(-6.0, 0.0, -2.0)),
    ];

    let wheels = vec![wheel(
        0,
        vec![
            sector(
                0,
                vec![
                    row(FRIGATE, vec![Entry::Present(1)]),
                    row(LANCER, vec![Entry::Present(1)]),
                    row(
                        RAIDER,
                        vec![
                            Entry::Present(1),
                            Entry::Leaving {
                                count: 2,
                                to: RockId(1),
                            },
                        ],
                    ),
                ],
                arc(0, 0.85, 0.9),
            ),
            sector(
                1,
                vec![
                    row(FRIGATE, vec![Entry::Present(1)]),
                    row(RAIDER, vec![Entry::Present(1)]),
                ],
                arc(1, 0.3, 0.55),
            ),
        ],
    )];

    Scene {
        rocks,
        entities,
        wheels,
        flights: Vec::new(),
        zone: ZONE,
        seat: SeatId(0),
        selection: Some(RockId(0)),
        hover: None,
    }
}

fn fight_camera() -> BeltCamera {
    BeltCamera::new(Vec3::ZERO, 60.0)
}

fn belt_scene() -> Scene {
    let positions = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1_400.0, 0.0, 400.0),
        Vec3::new(-1_200.0, 0.0, 800.0),
        Vec3::new(600.0, 0.0, -1_500.0),
        Vec3::new(-900.0, 0.0, -1_000.0),
    ];
    let rocks: Vec<RockView> = positions
        .iter()
        .enumerate()
        .map(|(index, &pos)| rock(index as u32, pos, 6.0))
        .collect();

    let seats = [0, 1, 0, 1, 0];
    let flight_from = Vec3::new(700.0, 0.0, 200.0);
    let mut entities: Vec<EntityView> = positions
        .iter()
        .zip(seats)
        .map(|(&pos, seat)| ship(seat, FRIGATE, pos + Vec3::new(4.0, 0.0, 0.0)))
        .collect();
    entities.push(flier(1, RAIDER, flight_from));

    let wheels = positions
        .iter()
        .zip(seats)
        .enumerate()
        .map(|(index, (_, seat))| {
            wheel(
                index as u32,
                vec![sector(
                    seat,
                    vec![
                        row(STORAGE, vec![Entry::Present(1)]),
                        row(FRIGATE, vec![Entry::Present(1)]),
                    ],
                    None,
                )],
            )
        })
        .collect();

    Scene {
        rocks,
        entities,
        wheels,
        flights: vec![FlightLine {
            from: flight_from,
            to: RockId(1),
        }],
        zone: ZONE,
        seat: SeatId(0),
        selection: None,
        hover: None,
    }
}

fn belt_camera() -> BeltCamera {
    BeltCamera::new(Vec3::new(0.0, 0.0, -200.0), 4_200.0)
}
