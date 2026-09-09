use std::collections::{BTreeMap, BTreeSet};

use neumannarch_sim::belt::Belt;
use neumannarch_sim::orbit::Gravity;
use neumannarch_sim::roster::{Glyph, Kind, Roster};
use neumannarch_sim::state::view::{Building, View};
use neumannarch_sim::state::{Asteroid, Command, Held, Preview};
use neumannarch_sim::{
    AsteroidId, EntityId, Materials, Post, Posting, RowId, SeatId, Stockpile, Time, Vec3,
};

use crate::display::fights::Fights;
use crate::display::label;
use crate::display::send::Sending;

const STRUCTURE_RING_STEP_RADIANS: f64 = core::f64::consts::TAU / 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    Solid,
    Hollow,
    Filling(f32),
    Dashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelButton {
    Plus(u32),
    Minus(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonAt {
    pub posting: Posting,
    pub button: WheelButton,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WheelGesture {
    Button(ButtonAt, Preview),
    Send(Sending, Preview),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub seat: SeatId,
    pub fraction: f32,
    pub trailing: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityView {
    pub seat: SeatId,
    pub glyph: Glyph,
    pub pos: Vec3,
    pub range: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightLine {
    pub from: Vec3,
    pub to: Vec3,
    pub previewed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsteroidView {
    pub id: AsteroidId,
    pub pos: Vec3,
    pub radius: f64,
    pub caps: Materials,
    pub pull: Materials,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BarMark {
    Cost(Materials),
    Refund(Materials),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StockpileBarView {
    pub stockpile: Stockpile,
    pub income: Materials,
    pub spend: Materials,
    pub elapsed: Time,
    pub clock: Time,
    pub marked: Option<BarMark>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub entry: Entry,
    pub previewed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entry {
    Present(u32),
    Surplus(u32),
    Building(Building),
    Arriving { count: u32, from: AsteroidId },
    Wanted { count: u32, dashed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub row: RowId,
    pub entries: Vec<Shown>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SectorView {
    pub seat: SeatId,
    pub rows: Vec<RowView>,
    pub arc: Option<Arc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WheelView {
    pub asteroid: AsteroidId,
    pub sectors: Vec<SectorView>,
}

pub struct Client<'a> {
    pub selection: Option<AsteroidId>,
    pub asked: Vec<AsteroidId>,
    pub gesture: Option<WheelGesture>,
    pub fights: &'a Fights,
}

impl Client<'_> {
    fn asked(&self) -> Vec<AsteroidId> {
        let mut asked: Vec<AsteroidId> = self
            .selection
            .into_iter()
            .chain(self.asked.iter().copied())
            .collect();
        asked.sort_unstable();
        asked.dedup();
        asked
    }
}

fn ringed(view: &View, roster: &Roster) -> BTreeMap<EntityId, Vec3> {
    let mut standing: BTreeMap<AsteroidId, u32> = BTreeMap::new();
    let mut ringed = BTreeMap::new();
    for present in &view.present {
        if roster[present.row].kind() != Kind::Structure {
            continue;
        }
        let Some(terrain) = view.terrain_of(present.home) else {
            continue;
        };
        let rung = standing.entry(present.home).or_default();
        let turn = STRUCTURE_RING_STEP_RADIANS * f64::from(*rung);
        *rung += 1;
        let asteroid = terrain.orbit.at(view.time, view.gravity).pos;
        let radial = asteroid.normalized().unwrap_or(Vec3::new(1.0, 0.0, 0.0));
        let along = Vec3::new(-radial.z, 0.0, radial.x);
        let reach = terrain.radius + Belt::SPACING_METERS;
        let (sin, cos) = turn.sin_cos();
        ringed.insert(present.id, asteroid + (radial * cos + along * sin) * reach);
    }
    ringed
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub asteroids: Vec<AsteroidView>,
    pub entities: Vec<EntityView>,
    pub wheels: Vec<WheelView>,
    pub fights: BTreeMap<AsteroidId, Vec<Arc>>,
    pub flights: Vec<FlightLine>,
    pub stockpile_bar: Option<StockpileBarView>,
    pub zone: f64,
    pub star_radius: f64,
    pub star_light_range: f64,
    pub belt_inner_radius: f64,
    pub belt_outer_radius: f64,
    pub seat: SeatId,
    pub selection: Option<AsteroidId>,
    pub gesture: Option<WheelGesture>,
}

impl Scene {
    pub fn of_belt(asteroids: &[Asteroid], gravity: Gravity, now: Time) -> Scene {
        Scene {
            asteroids: asteroids
                .iter()
                .enumerate()
                .map(|(at, asteroid)| AsteroidView {
                    id: AsteroidId(at as u32),
                    pos: asteroid.orbit().at(now, gravity).pos,
                    radius: asteroid.radius(),
                    caps: asteroid.caps(),
                    pull: Materials::ZERO,
                })
                .collect(),
            entities: Vec::new(),
            wheels: Vec::new(),
            fights: BTreeMap::new(),
            flights: Vec::new(),
            stockpile_bar: None,
            zone: Belt::ZONE_RADIUS_METERS,
            star_radius: Belt::STAR_RADIUS_METERS,
            star_light_range: Belt::STAR_LIGHT_RANGE_METERS,
            belt_inner_radius: Belt::inner_radius_meters(),
            belt_outer_radius: Belt::OUTER_RADIUS_METERS,
            seat: SeatId(0),
            selection: None,
            gesture: None,
        }
    }

    pub fn from_view(view: &View, roster: &Roster, client: Client<'_>) -> Scene {
        let ringed = ringed(view, roster);
        let asked = client.asked();
        let mut fights: BTreeMap<AsteroidId, Vec<Arc>> = BTreeMap::new();
        for (asteroid, arc) in client.fights.arcs() {
            fights.entry(asteroid).or_default().push(arc);
        }
        Scene {
            asteroids: view
                .terrain
                .iter()
                .map(|asteroid| AsteroidView {
                    id: asteroid.asteroid,
                    pos: asteroid.orbit.at(view.time, view.gravity).pos,
                    radius: asteroid.radius,
                    caps: asteroid.caps,
                    pull: asteroid.pull,
                })
                .collect(),
            entities: view
                .present
                .iter()
                .map(|present| EntityView {
                    seat: present.seat,
                    glyph: roster[present.row].glyph(),
                    pos: ringed.get(&present.id).copied().unwrap_or(present.body.pos),
                    range: reach(roster, present.row, present.at.standing().is_some()),
                })
                .collect(),
            wheels: Sectors::of(view, &fights, &asked)
                .previewing(view, client.gesture.as_ref())
                .drawn(),
            fights,
            flights: flight_lines(view, client.gesture.as_ref()),
            stockpile_bar: Some(StockpileBarView {
                stockpile: view.stockpile,
                income: view.income,
                spend: view.spend,
                elapsed: view.time,
                clock: view.length,
                marked: marked_on_bar(roster, client.gesture.as_ref()),
            }),
            zone: view.zone,
            star_radius: view.star_radius,
            star_light_range: view.star_light_range,
            belt_inner_radius: view.belt_inner_radius,
            belt_outer_radius: view.belt_outer_radius,
            seat: view.seat,
            selection: client.selection,
            gesture: client.gesture,
        }
    }

    pub fn wheel_of(&self, asteroid: AsteroidId) -> Option<&WheelView> {
        self.wheels.iter().find(|wheel| wheel.asteroid == asteroid)
    }

    pub fn largest_cap(&self) -> f64 {
        self.asteroids
            .iter()
            .flat_map(|asteroid| asteroid.caps.amounts().map(|(_, cap)| cap))
            .fold(0.0, f64::max)
    }
}

impl Entry {
    pub fn fill(self) -> Fill {
        match self {
            Entry::Present(_) => Fill::Solid,
            Entry::Building(building) => Fill::Filling(building.progress as f32),
            Entry::Surplus(_) | Entry::Arriving { .. } => Fill::Hollow,
            Entry::Wanted { dashed: false, .. } => Fill::Hollow,
            Entry::Wanted { dashed: true, .. } => Fill::Dashed,
        }
    }

    #[cfg(test)]
    pub(crate) fn dim(self) -> bool {
        matches!(self, Entry::Arriving { .. })
    }

    pub fn count(self) -> Option<u32> {
        match self {
            Entry::Present(count)
            | Entry::Surplus(count)
            | Entry::Arriving { count, .. }
            | Entry::Wanted { count, .. } => Some(count),
            Entry::Building(_) => None,
        }
    }

    pub fn phrase(self, name: &str) -> String {
        match self {
            Entry::Present(_) => format!("{name} here"),
            Entry::Surplus(_) => format!("{name} surplus"),
            Entry::Building(Building {
                starved_of: Some(material),
                ..
            }) => format!(
                "{name} short of {}",
                label::material(material).to_ascii_lowercase()
            ),
            Entry::Building(_) => format!("{name} building"),
            Entry::Arriving { from, .. } => format!("{name} arriving from {}", asteroid_name(from)),
            Entry::Wanted { dashed: true, .. } => format!("No builder for {name}"),
            Entry::Wanted { dashed: false, .. } => format!("{name} wanted"),
        }
    }
}

impl WheelButton {
    pub fn step(self) -> u32 {
        match self {
            WheelButton::Plus(step) | WheelButton::Minus(step) => step,
        }
    }

    pub fn label(self) -> String {
        match self {
            WheelButton::Plus(1) => "+".to_string(),
            WheelButton::Minus(1) => "-".to_string(),
            WheelButton::Plus(step) => format!("+{step}"),
            WheelButton::Minus(step) => format!("-{step}"),
        }
    }

    pub fn wanted(self, from: u32) -> u32 {
        match self {
            WheelButton::Plus(step) => from.saturating_add(step),
            WheelButton::Minus(step) => from.saturating_sub(step),
        }
    }
}

impl ButtonAt {
    pub fn edit(self, want: u32) -> Command {
        Command::Want {
            asteroid: self.posting.asteroid(),
            row: self.posting.row(),
            count: self.button.wanted(want),
        }
    }
}

pub fn asteroid_name(asteroid: AsteroidId) -> String {
    format!("Asteroid {}", u64::from(asteroid.0) + 1)
}

fn reach(roster: &Roster, row: RowId, standing: bool) -> Option<f64> {
    let does_damage = roster[row].does_damage();
    (standing && does_damage).then(|| roster[row].max_damage_range())
}

type Rows = BTreeMap<RowId, Vec<Shown>>;

struct Sectors {
    rows: BTreeMap<Post, Rows>,
    arcs: BTreeMap<Post, Arc>,
}

impl Sectors {
    fn of(view: &View, fights: &BTreeMap<AsteroidId, Vec<Arc>>, asked: &[AsteroidId]) -> Sectors {
        let mut rows: BTreeMap<Post, Rows> = BTreeMap::new();
        for (post, composition) in &view.compositions {
            if !asked.contains(&post.asteroid) {
                continue;
            }
            let sector = rows.entry(*post).or_default();
            for (row, held) in &composition.rows {
                let entries = entries_of(view, Posting::new(*post, *row), *held);
                if !entries.is_empty() {
                    sector.insert(*row, entries);
                }
            }
        }
        for (posting, plan) in &view.plans {
            if !asked.contains(&posting.asteroid()) {
                continue;
            }
            let entries = rows
                .entry(posting.post())
                .or_default()
                .entry(posting.row())
                .or_default();
            if let Some(building) = plan.building {
                entries.push(shown(Entry::Building(building)));
            }
            let short = plan
                .want
                .saturating_sub(covered(view, *posting) + wanted_frames(plan.building));
            if short > 0 {
                entries.push(shown(Entry::Wanted {
                    count: short,
                    dashed: !builds_at(view, posting.post()),
                }));
            }
            entries.sort_by_key(order);
        }
        let arcs: BTreeMap<Post, Arc> = fights
            .iter()
            .filter(|(asteroid, _)| asked.contains(asteroid))
            .flat_map(|(asteroid, arcs)| {
                arcs.iter().map(|arc| {
                    (
                        Post {
                            asteroid: *asteroid,
                            seat: arc.seat,
                        },
                        *arc,
                    )
                })
            })
            .collect();
        for post in arcs.keys() {
            rows.entry(*post).or_default();
        }
        for asteroid in asked.iter().copied().filter(|_| view.still_in) {
            rows.entry(Post {
                asteroid,
                seat: view.seat,
            })
            .or_default();
        }
        Sectors { rows, arcs }
    }

    fn previewing(mut self, view: &View, gesture: Option<&WheelGesture>) -> Sectors {
        let Some(WheelGesture::Send(_, sent)) = gesture else {
            return self;
        };
        for (posting, filling) in &sent.shortfalls {
            let arriving = self.entries(*posting);
            for (from, count) in &filling.sent_from {
                arriving.push(Shown {
                    entry: Entry::Arriving {
                        count: *count,
                        from: *from,
                    },
                    previewed: true,
                });
            }
            if filling.to_build > 0 {
                arriving.push(Shown {
                    entry: Entry::Wanted {
                        count: filling.to_build,
                        dashed: !builds_at(view, posting.post()),
                    },
                    previewed: true,
                });
            }
            arriving.sort_by_key(order);
            for from in filling.sent_from.keys() {
                for shown in self
                    .entries(Posting::of(*from, posting.seat(), posting.row()))
                    .iter_mut()
                    .filter(|shown| matches!(shown.entry, Entry::Present(_)))
                {
                    shown.previewed = true;
                }
            }
        }
        self
    }

    fn entries(&mut self, posting: Posting) -> &mut Vec<Shown> {
        self.rows
            .entry(posting.post())
            .or_default()
            .entry(posting.row())
            .or_default()
    }

    fn drawn(self) -> Vec<WheelView> {
        let mut wheels: BTreeMap<AsteroidId, WheelView> = BTreeMap::new();
        for (post, rows) in self.rows {
            wheels
                .entry(post.asteroid)
                .or_insert_with(|| WheelView {
                    asteroid: post.asteroid,
                    sectors: Vec::new(),
                })
                .sectors
                .push(SectorView {
                    seat: post.seat,
                    rows: rows
                        .into_iter()
                        .filter(|(_, entries)| !entries.is_empty())
                        .map(|(row, entries)| RowView { row, entries })
                        .collect(),
                    arc: self.arcs.get(&post).copied(),
                });
        }
        wheels.into_values().collect()
    }
}

fn order(shown: &Shown) -> u8 {
    match shown.entry {
        Entry::Present(_) => 0,
        Entry::Surplus(_) => 1,
        Entry::Building(_) => 2,
        Entry::Arriving { .. } => 3,
        Entry::Wanted { .. } => 4,
    }
}

fn shown(entry: Entry) -> Shown {
    Shown {
        entry,
        previewed: false,
    }
}

fn wanted_frames(building: Option<Building>) -> u32 {
    u32::from(building.is_some())
}

fn entries_of(view: &View, posting: Posting, held: Held) -> Vec<Shown> {
    let mut entries = Vec::new();
    if held.present > 0 {
        entries.push(shown(Entry::Present(held.present)));
    }
    if held.surplus > 0 {
        entries.push(shown(Entry::Surplus(held.surplus)));
    }
    if let Some(from) = arriving_from(view, posting)
        && held.arriving > 0
    {
        entries.push(shown(Entry::Arriving {
            count: held.arriving,
            from,
        }));
    }
    entries
}

fn arriving_from(view: &View, posting: Posting) -> Option<AsteroidId> {
    let asteroid = posting.asteroid();
    view.present
        .iter()
        .filter(|unit| {
            unit.seat == posting.seat() && unit.row == posting.row() && unit.home == asteroid
        })
        .find_map(|unit| unit.at.flying_from())
}

fn covered(view: &View, posting: Posting) -> u32 {
    view.compositions
        .get(&posting.post())
        .and_then(|composition| composition.rows.get(&posting.row()))
        .map_or(0, |held| held.present + held.arriving)
}

fn builds_at(view: &View, post: Post) -> bool {
    view.compositions
        .get(&post)
        .is_some_and(|composition| composition.builder)
}

fn flight_lines(view: &View, gesture: Option<&WheelGesture>) -> Vec<FlightLine> {
    let flying = view
        .present
        .iter()
        .filter(|present| present.at.flying_from().is_some())
        .filter_map(|present| {
            Some(FlightLine {
                from: present.body.pos,
                to: view.asteroid_body(present.home)?.pos,
                previewed: false,
            })
        });
    flying.chain(previewed_lines(view, gesture)).collect()
}

fn previewed_lines(view: &View, gesture: Option<&WheelGesture>) -> Vec<FlightLine> {
    let Some(WheelGesture::Send(_, sent)) = gesture else {
        return Vec::new();
    };
    let apart: BTreeSet<(AsteroidId, AsteroidId)> = sent
        .shortfalls
        .iter()
        .flat_map(|(posting, filling)| {
            filling
                .sent_from
                .keys()
                .map(move |source| (*source, posting.asteroid()))
        })
        .collect();
    apart
        .into_iter()
        .filter_map(|(source, destination)| {
            Some(FlightLine {
                from: view.asteroid_body(source)?.pos,
                to: view.asteroid_body(destination)?.pos,
                previewed: true,
            })
        })
        .collect()
}

fn marked_on_bar(roster: &Roster, gesture: Option<&WheelGesture>) -> Option<BarMark> {
    let WheelGesture::Button(at, preview) = gesture? else {
        return None;
    };
    match at.button {
        WheelButton::Plus(_) => Some(BarMark::Cost(preview.cost_to_build(roster))),
        WheelButton::Minus(_) => Some(BarMark::Refund(preview.refund)),
    }
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD, STORAGE};

    use super::*;
    use crate::display::local::{Local, PLAYER, RIVAL};

    fn at(asteroid: u32) -> AsteroidId {
        AsteroidId(asteroid)
    }

    fn scene(local: &Local) -> Scene {
        drawn(local, &Fights::default(), None, None)
    }

    fn drawn(
        local: &Local,
        fights: &Fights,
        selection: Option<AsteroidId>,
        gesture: Option<WheelGesture>,
    ) -> Scene {
        Scene::from_view(
            &local.view(),
            local.session().state().roster(),
            Client {
                selection,
                asked: local
                    .view()
                    .terrain
                    .iter()
                    .map(|terrain| terrain.asteroid)
                    .collect(),
                gesture,
                fights,
            },
        )
    }

    fn dragging(local: &Local, sending: Sending) -> WheelGesture {
        let edits = sending.commands(&local.view(), local.session().state().roster());
        WheelGesture::Send(sending, previewed(local, &edits))
    }

    fn pressing(local: &Local, at: ButtonAt) -> WheelGesture {
        let edit = at.edit(local.view().want_of(at.posting));
        WheelGesture::Button(at, previewed(local, &[edit]))
    }

    fn previewed(local: &Local, wants: &[Command]) -> Preview {
        local
            .session()
            .state()
            .preview(PLAYER, wants)
            .expect("the wants the pointer would issue stand")
    }

    fn entries(scene: &Scene, asteroid: AsteroidId, seat: SeatId, row: RowId) -> Vec<Shown> {
        scene
            .wheel_of(asteroid)
            .into_iter()
            .flat_map(|wheel| &wheel.sectors)
            .filter(|sector| sector.seat == seat)
            .flat_map(|sector| &sector.rows)
            .filter(|shown| shown.row == row)
            .flat_map(|shown| shown.entries.clone())
            .collect()
    }

    #[test]
    fn only_an_asked_asteroid_carries_a_wheel_whatever_it_holds() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);

        let asking = |asked: Vec<AsteroidId>| {
            Scene::from_view(
                &local.view(),
                local.session().state().roster(),
                Client {
                    selection: None,
                    asked,
                    gesture: None,
                    fights: &Fights::default(),
                },
            )
        };
        assert!(
            asking(Vec::new()).wheels.is_empty(),
            "nothing asked, nothing drawn"
        );

        let asked = asking(vec![at(0), at(5)]);
        assert_eq!(asked.wheels.len(), 2, "one wheel per asteroid asked");
        assert!(asked.wheel_of(at(0)).is_some());
        assert!(asked.wheel_of(at(5)).is_some());
    }

    #[test]
    fn a_draft_pick_stands_on_its_asteroid_as_a_present_structure() {
        let mut local = Local::drafting(2);
        let stages = local.session().state().draft().stages().to_vec();
        let mine = stages
            .iter()
            .find(|stage| stage.seat == PLAYER)
            .expect("the player picks");
        let theirs = stages
            .iter()
            .find(|stage| stage.seat == RIVAL)
            .expect("the rival picks");
        let (first, second) = match stages[0].seat == PLAYER {
            true => ((PLAYER, mine), (RIVAL, theirs)),
            false => ((RIVAL, theirs), (PLAYER, mine)),
        };
        local.want_of(first.0, &[(at(0), first.1.row, 1)]);
        local.want_of(second.0, &[(at(1), second.1.row, 1)]);

        let drafting = scene(&local);
        for (asteroid, (seat, stage)) in [(at(0), first), (at(1), second)] {
            assert_eq!(
                entries(&drafting, asteroid, seat, stage.row),
                vec![shown(Entry::Present(1))],
                "the pick stands at its asteroid while the draft runs"
            );
        }
    }

    #[test]
    fn a_selected_asteroid_carries_the_seats_own_wheel_though_it_holds_nothing() {
        let local = Local::start(2);

        let scene = drawn(&local, &Fights::default(), Some(at(5)), None);

        let wheel = scene
            .wheel_of(at(5))
            .expect("the selection carries a wheel");
        assert_eq!(wheel.sectors.len(), 1);
        assert_eq!(wheel.sectors[0].seat, PLAYER);
        assert!(wheel.sectors[0].rows.is_empty(), "and no entry of its own");

        let pointed = Scene::from_view(
            &local.view(),
            local.session().state().roster(),
            Client {
                selection: None,
                asked: vec![at(5)],
                gesture: None,
                fights: &Fights::default(),
            },
        );
        assert_eq!(
            pointed.wheel_of(at(5)),
            Some(wheel),
            "the asteroid under the pointer carries the same wheel"
        );
    }

    #[test]
    fn a_placed_structure_stands_as_one_present_entry_with_its_count() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);

        let shown = entries(&scene(&local), at(0), PLAYER, SHIPYARD);

        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].entry, Entry::Present(1));
        assert_eq!(shown[0].entry.fill(), Fill::Solid);
        assert_eq!(shown[0].entry.count(), Some(1));
        assert!(!shown[0].previewed);
    }

    #[test]
    fn a_frame_fills_where_a_builder_stands_and_a_want_is_dashed_where_none_does() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        local.want(&[(at(0), STORAGE, 1), (at(5), FRIGATE, 2)]);
        local.run(60);
        let scene = scene(&local);

        let built = entries(&scene, at(0), PLAYER, STORAGE);
        assert_eq!(built.len(), 1, "the frame covers the want");
        assert!(matches!(built[0].entry, Entry::Building(_)));
        assert!(matches!(built[0].entry.fill(), Fill::Filling(progress) if progress > 0.0));
        assert_eq!(built[0].entry.count(), None, "a frame carries no count");

        let unbuilt = entries(&scene, at(5), PLAYER, FRIGATE);
        assert_eq!(
            unbuilt[1].entry,
            Entry::Wanted {
                count: 1,
                dashed: true
            },
            "no builder stands at the far asteroid"
        );
        assert_eq!(unbuilt[1].entry.fill(), Fill::Dashed);
        assert_eq!(unbuilt[1].entry.phrase("Frigate"), "No builder for Frigate");
    }

    #[test]
    fn a_re_homed_unit_leaves_its_asteroids_wheel_at_once_and_flies_on_the_belt() {
        let mut local = Local::start(2);
        local.want(&[(at(0), CONSTRUCTOR, 1)]);
        local.want(&[(at(0), CONSTRUCTOR, 0), (at(1), CONSTRUCTOR, 1)]);
        let scene = scene(&local);

        assert!(
            entries(&scene, at(0), PLAYER, CONSTRUCTOR).is_empty(),
            "the wheel keeps a ship that is already flying"
        );
        let arriving = entries(&scene, at(1), PLAYER, CONSTRUCTOR);
        assert_eq!(
            arriving[0].entry,
            Entry::Arriving {
                count: 1,
                from: at(0)
            }
        );
        assert!(arriving[0].entry.dim(), "a unit on its way is dimmed");
        assert_eq!(
            arriving[0].entry.phrase("Constructor"),
            "Constructor arriving from Asteroid 1"
        );

        assert_eq!(scene.flights.len(), 1);
        let ahead = scene.flights[0].to;
        let destination = local.view().asteroid_body(at(1)).expect("the destination");
        assert_eq!(
            ahead, destination.pos,
            "the line runs to where the asteroid stands now, so the two move together"
        );
    }

    #[test]
    fn another_seats_sector_shows_what_it_holds_and_never_its_wants() {
        let mut local = Local::start(2);
        local.want_of(RIVAL, &[(at(3), SHIPYARD, 1), (at(3), FRIGATE, 2)]);
        local.run(1);

        let scene = scene(&local);

        assert_eq!(
            entries(&scene, at(3), RIVAL, SHIPYARD)[0].entry,
            Entry::Present(1),
            "an enemy structure is on its wheel"
        );
        assert!(
            entries(&scene, at(3), RIVAL, FRIGATE).is_empty(),
            "what it wants is its own to see"
        );
    }

    #[test]
    fn what_stands_above_the_want_is_surplus_and_stays() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        local.run(1);
        assert!(
            !entries(&scene(&local), at(0), PLAYER, SHIPYARD)
                .iter()
                .any(|shown| matches!(shown.entry, Entry::Surplus(_))),
            "a covered want has no surplus"
        );

        local.want(&[(at(0), SHIPYARD, 0)]);
        local.run(1);
        let shown = entries(&scene(&local), at(0), PLAYER, SHIPYARD);

        assert_eq!(shown[0].entry, Entry::Present(1), "nothing is scrapped");
        assert_eq!(shown[1].entry, Entry::Surplus(1));
        assert_eq!(shown[1].entry.fill(), Fill::Hollow);
        assert_eq!(shown[1].entry.phrase("Shipyard"), "Shipyard surplus");
    }

    #[test]
    fn a_viewer_whose_seat_is_out_carries_no_sector_of_its_own_and_so_no_button() {
        let local = Local::start(2);
        let out = local.view_of(SeatId(9));
        assert!(!out.still_in, "a seat the match lacks is not in it");

        let scene = Scene::from_view(
            &out,
            local.session().state().roster(),
            Client {
                selection: Some(at(5)),
                asked: vec![at(5)],
                gesture: None,
                fights: &Fights::default(),
            },
        );

        assert_eq!(
            scene.wheel_of(at(5)),
            None,
            "the selection carries nothing for a seat that is out"
        );
        assert!(local.view().still_in, "a seated player is in the match");
    }

    #[test]
    fn a_wheels_surplus_is_the_count_the_sim_would_send_and_never_the_client_arithmetic() {
        let mut local = Local::start(2);
        local.want(&[(at(0), CONSTRUCTOR, 1)]);
        local.want(&[(at(0), CONSTRUCTOR, 0), (at(1), CONSTRUCTOR, 1)]);
        local.want(&[(at(1), CONSTRUCTOR, 0)]);
        let view = local.view();

        let arriving = view
            .compositions
            .get(&Post {
                asteroid: at(1),
                seat: PLAYER,
            })
            .and_then(|composition| composition.rows.get(&CONSTRUCTOR))
            .copied()
            .expect("the constructor is homed at its destination");

        assert_eq!(arriving.arriving, 1);
        assert_eq!(arriving.present, 0);
        assert_eq!(view.want_of(Posting::of(at(1), PLAYER, CONSTRUCTOR)), 0);
        assert_eq!(
            arriving.surplus, 0,
            "a unit still on its way stands nowhere to be surplus"
        );
        assert!(
            !entries(&scene(&local), at(1), PLAYER, CONSTRUCTOR)
                .iter()
                .any(|shown| matches!(shown.entry, Entry::Surplus(_))),
            "the wheel says what the sim says"
        );
    }

    #[test]
    fn the_stockpile_bar_marks_the_cost_a_plus_would_pay_and_the_refund_a_minus_would_take() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        let roster = local.session().state().roster();
        let button = |button| {
            let at = ButtonAt {
                posting: Posting::of(at(0), PLAYER, FRIGATE),
                button,
            };
            drawn(
                &local,
                &Fights::default(),
                Some(at.posting.asteroid()),
                Some(pressing(&local, at)),
            )
            .stockpile_bar
            .and_then(|bar| bar.marked)
        };

        assert_eq!(
            button(WheelButton::Plus(1)),
            Some(BarMark::Cost(roster[FRIGATE].cost)),
            "one more frigate costs one frigate"
        );
        assert_eq!(
            button(WheelButton::Minus(1)),
            Some(BarMark::Refund(Materials::ZERO)),
            "nothing is building to refund"
        );
    }

    #[test]
    fn a_hovered_button_changes_no_entry() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        local.want(&[(at(0), SHIPYARD, 3)]);
        let hover = |row, button| {
            let at = ButtonAt {
                posting: Posting::of(at(0), PLAYER, row),
                button,
            };
            let scene = drawn(
                &local,
                &Fights::default(),
                Some(at.posting.asteroid()),
                Some(pressing(&local, at)),
            );
            entries(&scene, at.posting.asteroid(), PLAYER, row)
        };
        let still = scene(&local);

        assert_eq!(
            hover(SHIPYARD, WheelButton::Minus(1)),
            entries(&still, at(0), PLAYER, SHIPYARD),
            "minus leaves what stands and what is wanted as they are"
        );
        assert!(
            hover(FRIGATE, WheelButton::Plus(5)).is_empty(),
            "plus adds nothing until it is clicked"
        );
    }

    #[test]
    fn a_send_drag_dims_the_source_and_shows_the_destination_arriving() {
        let mut local = Local::start(2);
        local.want(&[(at(0), CONSTRUCTOR, 1)]);
        let sending = Sending {
            from: at(0),
            to: at(1),
            count: 1,
        };

        let scene = drawn(
            &local,
            &Fights::default(),
            None,
            Some(dragging(&local, sending)),
        );

        let source = entries(&scene, at(0), PLAYER, CONSTRUCTOR);
        assert_eq!(source[0].entry, Entry::Present(1));
        assert!(source[0].previewed, "what would go is dimmed");

        let destination = entries(&scene, at(1), PLAYER, CONSTRUCTOR);
        assert_eq!(
            destination[0].entry,
            Entry::Arriving {
                count: 1,
                from: at(0)
            }
        );
        assert!(destination[0].previewed);
        let line = scene
            .flights
            .iter()
            .find(|flight| flight.previewed)
            .expect("the drag draws its own flight line");
        assert_eq!(
            line.to,
            local
                .view()
                .asteroid_body(at(1))
                .expect("the destination")
                .pos
        );
    }

    #[test]
    fn a_ship_that_does_damage_at_an_asteroid_carries_its_range_and_one_in_flight_carries_none() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        let roster = local.session().state().roster();

        assert_eq!(
            reach(roster, FRIGATE, true),
            Some(roster[FRIGATE].max_damage_range()),
            "a frigate at an asteroid draws its longest range"
        );
        assert_eq!(reach(roster, FRIGATE, false), None, "and none in flight");
        assert_eq!(
            reach(roster, SHIPYARD, true),
            None,
            "a shipyard does no damage"
        );

        let scene = scene(&local);
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].range, None);
        assert_eq!(scene.zone, local.view().zone);
    }

    #[test]
    fn a_match_scene_carries_the_seats_stockpile_and_the_clock_and_a_belt_scene_none() {
        let local = Local::start(2);
        let view = local.view();

        let bar = scene(&local)
            .stockpile_bar
            .expect("a match has a stockpile bar");
        assert_eq!(bar.stockpile, view.stockpile);
        assert_eq!(bar.income, view.income);
        assert_eq!(bar.spend, view.spend);
        assert_eq!(bar.elapsed, view.time);
        assert_eq!(bar.clock, view.length);
        assert_eq!(bar.marked, None, "an idle pointer marks no bar");
        assert!(
            scene(&local)
                .asteroids
                .iter()
                .zip(&view.terrain)
                .all(|(asteroid, terrain)| asteroid.pull == terrain.pull),
            "every asteroid carries its pull"
        );

        let belt = Scene::of_belt(&Belt::from_seed(0), Belt::GRAVITY, Time::ZERO);
        assert_eq!(belt.stockpile_bar, None);
        assert!(
            belt.asteroids
                .iter()
                .all(|asteroid| asteroid.pull == Materials::ZERO)
        );
    }
}
