use std::fs;
use std::path::Path;

use mirage_engine::headless::Session;
use mirage_engine::math::UVec2;
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use neumannarch_game::display::camera::BeltCamera;
use neumannarch_game::display::fights::Fights;
use neumannarch_game::display::glyph;
use neumannarch_game::display::glyph::Glyph;
use neumannarch_game::display::glyph_quad::GlyphQuad;
use neumannarch_game::display::scene::{
    Arc, AsteroidView, ButtonAt, Client, EntityView, Entry, FlightLine, RowView, Scene, SectorView,
    Shown, StockpileBarView, WheelButton, WheelGesture, WheelView,
};
use neumannarch_game::display::stockpile_bar::StockpileBar;
use neumannarch_game::display::viewport::Viewport;
use neumannarch_game::display::wheels::{Aim, Still, Wheels};
use neumannarch_game::display::{belt, hud};
use neumannarch_game::screens::control::Controls;
use neumannarch_game::screens::lobby::seat_names;
use neumannarch_game::screens::order::Order;
use neumannarch_game::screens::panel::Panel;
use neumannarch_protocol::{Lobby, LobbyEdit, PlayerId};
use neumannarch_sim::Session as Match;
use neumannarch_sim::belt::Belt;
use neumannarch_sim::roster::{
    ENERGY_EXTRACTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER, Roster, SHIPYARD,
};
use neumannarch_sim::state::view::{Building, View};
use neumannarch_sim::state::{Command, STAGE_SPAN, State};
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{
    AsteroidId, Material, Materials, Posting, Retention, RowId, SeatId, Sequence, Stockpile, Time,
    Vec3,
};

meshes! { enum Shape { Sphere, GlyphQuad } }

const WINDOW: UVec2 = UVec2::new(1280, 720);

const ZONE: f64 = neumannarch_sim::belt::Belt::ZONE_RADIUS_METERS;

const YOU: SeatId = SeatId(0);

const TAKEN: AsteroidId = AsteroidId(0);

const HOME: Vec3 = Vec3::new(20_000.0, 0.0, 0.0);

const DRAFT_ZOOM_PER_METER_APART: f64 = 2.4;

const DRAFT_FOCUS_TOWARD_TAKEN: f64 = 0.3;

fn glyph_of(row: RowId) -> Glyph {
    Glyph::of(&Roster::shipped()[row])
}

fn main() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("look");
    fs::create_dir_all(&out).expect("game/look is writable");

    let watched = Watched::wanting();
    let belt = belt_scene();
    let framing_the_belt = BeltCamera::framing(belt.belt_inner_radius, belt.belt_outer_radius);
    for (name, scene, camera) in [
        (
            "region",
            on_the_ring(region_scene(&watched)),
            region_camera(),
        ),
        ("fight", on_the_ring(fight_scene()), fight_camera()),
        (
            "stockpile",
            on_the_ring(stockpile_scene()),
            stockpile_camera(),
        ),
        ("belt", belt, framing_the_belt),
    ] {
        let pixels = render(scene, camera, None, watched.clone());
        save(&out.join(format!("{name}.png")), &pixels);
    }
    let (scene, drafting, watched) = draft_scene();
    let camera = draft_camera(&scene, drafting.bare);
    let pixels = render(scene, camera, Some(drafting), watched);
    save(&out.join("draft.png"), &pixels);
}

#[derive(Clone)]
struct Watched {
    view: View,
    state: State,
}

impl Watched {
    fn of(session: &Match) -> Watched {
        Watched {
            view: View::of(session.state(), YOU, &Shots::default()),
            state: session.state().clone(),
        }
    }

    fn wanting() -> Watched {
        let (mut session, _) = skirmish_where_you_go_first();
        let mut sequence = Sequence::new(YOU);
        for row in [FRIGATE, RAIDER] {
            let stamped = sequence.stamp(
                session.state().tick(),
                Command::Want {
                    asteroid: AsteroidId(0),
                    row,
                    count: 1,
                },
            );
            session.insert(stamped).expect("the want stands");
        }
        session.advance();
        Watched::of(&session)
    }

    fn previewing(&self, at: ButtonAt) -> WheelGesture {
        let edit = at.edit(self.view.want_of(at.posting));
        let preview = self
            .state
            .preview(YOU, &[edit])
            .expect("the button the pointer rests on stands");
        WheelGesture::Button(at, preview)
    }
}

struct Drafting {
    names: Vec<String>,
    button: Posting,
    bare: AsteroidId,
}

struct Looker {
    scene: Scene,
    camera: BeltCamera,
    drafting: Option<Drafting>,
    watched: Watched,
}

impl Game for Looker {
    type Actions = NoActions;
    type Meshes = Shape;
    type Sound = NoSound;
    type Sources = NoSources;
    type Styles = ();

    fn tick(&mut self, _ctx: &mut TickCtx<'_, Self>) {}

    fn frame(&mut self, ctx: &mut FrameCtx<'_, Self>) {
        let points_per_pixel = 1.0 / ctx.pixels_per_point();
        let window = ctx.window_size();
        let viewport = Viewport::of(&self.camera, window, points_per_pixel);

        belt::draw(&self.scene, &viewport, ctx);

        let roster = Roster::shipped();
        let scene = &self.scene;
        let over = viewport.bounds();
        let bar = scene
            .stockpile_bar
            .map(|view| StockpileBar::across(over, view));
        let wheels = Wheels::over(
            scene,
            &roster,
            &viewport,
            &aim(scene, &self.watched),
            &mut Still,
        );
        let wheels = match &bar {
            Some(bar) => wheels.clear_of(bar.frame()),
            None => wheels,
        };
        let order = self.drafting.as_ref().map(|drafting| {
            Order::over(
                over,
                &self.watched.view.draft,
                self.watched.view.tick,
                &roster,
                &drafting.names,
                1.0,
            )
        });
        let note = self.drafting.as_ref().and_then(|drafting| {
            let posting = drafting.button;
            let at = wheels
                .iter()
                .find(|wheel| wheel.asteroid() == posting.asteroid())?
                .button(posting.row(), WheelButton::Plus(1))?;
            let (beside, spoken) = wheels.spoken_at(at)?;
            Some((
                egui::Rect::from_center_size(beside, egui::Vec2::splat(2.0 * glyph::HALF)),
                spoken.phrase(&roster),
            ))
        });
        ctx.ui(|ui| {
            hud::paint(scene, &viewport, ui.painter());
            wheels.paint(ui.painter(), scene.gesture.as_ref());
            if let Some(bar) = &bar {
                bar.paint(ui.painter(), None);
            }
            if let Some(order) = &order {
                order.paint(ui.painter());
            }
            if let Some((beside, phrase)) = note {
                let panel = Panel::new(ui.painter(), over, egui::Pos2::ZERO, false);
                let mut controls = Controls::over(&panel);
                if let Some(bar) = &bar {
                    controls.avoid(bar.frame());
                }
                controls.note(beside, phrase);
                controls.finish();
            }
        });
    }
}

fn aim<'a>(scene: &Scene, watched: &'a Watched) -> Aim<'a> {
    Aim {
        viewer: scene.seat,
        pointer: None,
        hovered: None,
        step: 1,
        view: &watched.view,
        state: &watched.state,
    }
}

fn render(
    scene: Scene,
    camera: BeltCamera,
    drafting: Option<Drafting>,
    watched: Watched,
) -> Vec<u8> {
    let mut session = Session::<Looker>::new(
        Config::new("neumannarch-look").with_tick_interval(neumannarch_sim::TICK),
        WINDOW,
        |_ctx| {
            Ok(Looker {
                scene,
                camera,
                drafting,
                watched,
            })
        },
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

fn asteroid(id: u32, pos: Vec3, radius: f64) -> AsteroidView {
    AsteroidView {
        id: AsteroidId(id),
        pos,
        radius,
        caps: caps_of(id),
        pull: Materials::ZERO,
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

fn wheel(asteroid: u32, sectors: Vec<SectorView>) -> WheelView {
    WheelView {
        asteroid: AsteroidId(asteroid),
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

fn region_scene(watched: &Watched) -> Scene {
    let asteroids = vec![
        asteroid(0, Vec3::new(0.0, 0.0, 0.0), 6.0),
        asteroid(1, Vec3::new(320.0, 0.0, -110.0), 5.0),
        asteroid(2, Vec3::new(-280.0, 0.0, 220.0), 4.0),
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
                        from: AsteroidId(0),
                    }],
                )],
                None,
            )],
        ),
    ];

    Scene {
        asteroids,
        entities,
        wheels,
        flights: vec![FlightLine {
            from: Vec3::new(-90.0, 0.0, 80.0),
            to: AsteroidId(2),
            previewed: false,
        }],
        stockpile_bar: None,
        zone: ZONE,
        star_radius: Belt::STAR_RADIUS_METERS,
        star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
        belt_inner_radius: Belt::inner_radius_meters(),
        belt_outer_radius: Belt::OUTER_RADIUS_METERS,
        seat: SeatId(0),
        selection: Some(AsteroidId(0)),
        gesture: Some(watched.previewing(ButtonAt {
            posting: Posting::of(AsteroidId(0), SeatId(0), RAIDER),
            button: WheelButton::Plus(1),
        })),
    }
}

fn region_camera() -> BeltCamera {
    BeltCamera::new(
        HOME + Vec3::new(10.0, 0.0, 30.0),
        620.0,
        Belt::inner_radius_meters(),
        Belt::OUTER_RADIUS_METERS,
    )
}

fn fight_scene() -> Scene {
    let asteroids = vec![asteroid(0, Vec3::ZERO, 6.0)];

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
                                to: AsteroidId(1),
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
        asteroids,
        entities,
        wheels,
        flights: Vec::new(),
        stockpile_bar: None,
        zone: ZONE,
        star_radius: Belt::STAR_RADIUS_METERS,
        star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
        belt_inner_radius: Belt::inner_radius_meters(),
        belt_outer_radius: Belt::OUTER_RADIUS_METERS,
        seat: SeatId(0),
        selection: Some(AsteroidId(0)),
        gesture: None,
    }
}

fn fight_camera() -> BeltCamera {
    BeltCamera::new(
        HOME,
        60.0,
        Belt::inner_radius_meters(),
        Belt::OUTER_RADIUS_METERS,
    )
}

fn stockpile_scene() -> Scene {
    let mut mined = asteroid(0, Vec3::ZERO, 6.0);
    mined.caps = Materials::new(20.0, 8.0, 12.0);
    mined.pull = Materials::new(12.0, 0.0, 12.0);
    let mut barren = asteroid(1, Vec3::new(340.0, 0.0, -120.0), 5.0);
    barren.caps = Materials::new(4.0, 0.0, 16.0);
    barren.pull = Materials::new(0.0, 0.0, 6.0);
    let asteroids = vec![mined, barren];

    let entities = vec![
        ship(0, METALS_EXTRACTOR, Vec3::new(-5.0, 0.0, 3.0)),
        ship(0, ENERGY_EXTRACTOR, Vec3::new(5.0, 0.0, 3.0)),
        ship(0, SHIPYARD, Vec3::new(0.0, 0.0, -6.0)),
        ship(0, FRIGATE, Vec3::new(6.0, 0.0, -3.0)),
        ship(1, ENERGY_EXTRACTOR, Vec3::new(344.0, 0.0, -118.0)),
    ];

    let wheels = vec![wheel(
        0,
        vec![sector(
            0,
            vec![
                row(SHIPYARD, vec![Entry::Present(1)]),
                row(METALS_EXTRACTOR, vec![Entry::Present(1)]),
                row(ENERGY_EXTRACTOR, vec![Entry::Present(1)]),
                row(
                    FRIGATE,
                    vec![
                        Entry::Present(1),
                        Entry::Building(Building {
                            progress: 0.3,
                            starved_of: Some(Material::Volatiles),
                        }),
                        Entry::Wanted {
                            count: 2,
                            dashed: false,
                        },
                    ],
                ),
            ],
            None,
        )],
    )];

    Scene {
        asteroids,
        entities,
        wheels,
        flights: Vec::new(),
        stockpile_bar: Some(StockpileBarView {
            stockpile: Stockpile::new(
                Materials::new(120.0, 0.0, 300.0),
                Materials::new(300.0, 300.0, 300.0),
            ),
            income: Materials::new(12.0, 0.0, 12.0),
            spend: Materials::new(9.0, 4.0, 2.0),
            elapsed: Time(neumannarch_sim::TICKS_PER_SECOND as u64 * 247),
            clock: Time(neumannarch_sim::TICKS_PER_SECOND as u64 * 900),
            marked: None,
        }),
        zone: ZONE,
        star_radius: Belt::STAR_RADIUS_METERS,
        star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
        belt_inner_radius: Belt::inner_radius_meters(),
        belt_outer_radius: Belt::OUTER_RADIUS_METERS,
        seat: SeatId(0),
        selection: Some(AsteroidId(0)),
        gesture: None,
    }
}

fn stockpile_camera() -> BeltCamera {
    BeltCamera::new(
        HOME + Vec3::new(120.0, 0.0, -40.0),
        500.0,
        Belt::inner_radius_meters(),
        Belt::OUTER_RADIUS_METERS,
    )
}

fn on_the_ring(mut scene: Scene) -> Scene {
    for asteroid in &mut scene.asteroids {
        asteroid.pos += HOME;
    }
    for entity in &mut scene.entities {
        entity.pos += HOME;
    }
    for flight in &mut scene.flights {
        flight.from += HOME;
    }
    scene
}

fn belt_scene() -> Scene {
    Scene::of_belt(&Belt::from_seed(0), Belt::GRAVITY, Time::ZERO)
}

fn skirmish_where_you_go_first() -> (Match, Vec<String>) {
    (0u64..)
        .find_map(|seed| {
            let mut lobby = Lobby::skirmish(PlayerId::HOST);
            lobby
                .edit(PlayerId::HOST, LobbyEdit::SetSeed(seed))
                .expect("the host sets the seed");
            let started = lobby.freeze().expect("a skirmish starts");
            let names = seat_names(started.seating(), PlayerId::HOST, seed);
            let (setup, _) = started.parts();
            let session =
                Match::new(setup, Retention::shipped(), &[YOU]).expect("the host is seated");
            (session.state().draft().stages()[0].seat == YOU).then_some((session, names))
        })
        .expect("some seed draws the host first")
}

fn draft_scene() -> (Scene, Drafting, Watched) {
    let (mut session, names) = skirmish_where_you_go_first();
    let first = session.state().draft().stages()[0];
    let stamped = Sequence::new(YOU).stamp(
        session.state().tick(),
        Command::Want {
            asteroid: TAKEN,
            row: first.row,
            count: 1,
        },
    );
    session.insert(stamped).expect("the pick is taken");
    for _ in 0..STAGE_SPAN.0 + 1 + STAGE_SPAN.0 / 2 {
        session.advance();
    }
    let watched = Watched::of(&session);
    let view = &watched.view;
    let waiting = view
        .draft
        .stages()
        .iter()
        .find(|stage| stage.seat == YOU && stage.placed.is_none())
        .expect("your second stage waits");
    let bare = nearest_free(&watched.state);
    let button = Posting::of(bare, YOU, waiting.row);
    let scene = Scene::from_view(
        view,
        session.state().roster(),
        Client {
            selection: Some(bare),
            pointed: Some(bare),
            gesture: None,
            fights: &Fights::default(),
        },
    );
    (
        scene,
        Drafting {
            names,
            button,
            bare,
        },
        watched,
    )
}

fn nearest_free(state: &State) -> AsteroidId {
    let taken = state.asteroid_body(TAKEN).pos;
    state
        .asteroids()
        .filter(|(id, _)| *id != TAKEN && state.draft().took(*id).is_none())
        .min_by(|(one, _), (other, _)| {
            let apart = |id: AsteroidId| state.asteroid_body(id).pos.distance(taken);
            apart(*one).total_cmp(&apart(*other))
        })
        .map(|(id, _)| id)
        .expect("a free asteroid beside the one taken")
}

fn draft_camera(scene: &Scene, bare: AsteroidId) -> BeltCamera {
    let at = |id: AsteroidId| {
        scene
            .asteroids
            .iter()
            .find(|asteroid| asteroid.id == id)
            .expect("the asteroid is on the belt")
            .pos
    };
    let (taken, bare) = (at(TAKEN), at(bare));
    let apart = bare.distance(taken);
    BeltCamera::new(
        bare + (taken - bare) * DRAFT_FOCUS_TOWARD_TAKEN,
        apart * DRAFT_ZOOM_PER_METER_APART,
        scene.belt_inner_radius,
        scene.belt_outer_radius,
    )
}
