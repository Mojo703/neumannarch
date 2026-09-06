use std::collections::BTreeMap;

use neumannarch_sim::belt::Belt;
use neumannarch_sim::orbit::Gravity;
use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::view::{Berth, Building, View};
use neumannarch_sim::state::{Held, MAX_WANT, Rock};
use neumannarch_sim::{Materials, RockId, RowId, SeatId, Stockpile, Time, Vec3};

use crate::display::fights::Fights;
use crate::display::glyph::Glyph;
use crate::display::label;
use crate::display::send::Sending;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    Solid,
    Hollow,
    Filling(f32),
    Dashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelBand {
    Plus(u32),
    Minus(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hover {
    Wheel {
        rock: RockId,
        row: RowId,
        band: WheelBand,
    },
    Send(Sending),
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
    pub to: RockId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RockView {
    pub id: RockId,
    pub pos: Vec3,
    pub radius: f64,
    pub caps: Materials,
    pub pull: Materials,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StripView {
    pub stockpile: Stockpile,
    pub income: Materials,
    pub spend: Materials,
    pub elapsed: Time,
    pub clock: Time,
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
    Leaving { count: u32, to: RockId },
    Building(Building),
    Arriving { count: u32, from: RockId },
    Wanted { count: u32, dashed: bool },
    Placed,
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
    pub rock: RockId,
    pub sectors: Vec<SectorView>,
}

pub struct Client<'a> {
    pub selection: Option<RockId>,
    pub pointed: Option<RockId>,
    pub hover: Option<Hover>,
    pub fights: &'a Fights,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub rocks: Vec<RockView>,
    pub entities: Vec<EntityView>,
    pub wheels: Vec<WheelView>,
    pub flights: Vec<FlightLine>,
    pub strip: Option<StripView>,
    pub zone: f64,
    pub seat: SeatId,
    pub selection: Option<RockId>,
    pub hover: Option<Hover>,
}

impl Scene {
    pub fn of_belt(rocks: &[Rock], gravity: Gravity, now: Time) -> Scene {
        Scene {
            rocks: rocks
                .iter()
                .enumerate()
                .map(|(at, rock)| RockView {
                    id: RockId(at as u32),
                    pos: rock.orbit().at(now, gravity).pos,
                    radius: rock.radius(),
                    caps: rock.caps(),
                    pull: Materials::ZERO,
                })
                .collect(),
            entities: Vec::new(),
            wheels: Vec::new(),
            flights: Vec::new(),
            strip: None,
            zone: Belt::ZONE_RADIUS_METERS,
            seat: SeatId(0),
            selection: None,
            hover: None,
        }
    }

    pub fn centre(&self) -> Vec3 {
        let rocks = self.rocks.len().max(1) as f64;
        self.rocks
            .iter()
            .fold(Vec3::ZERO, |sum, rock| sum + rock.pos)
            * (1.0 / rocks)
    }

    pub fn from_view(view: &View, roster: &Roster, client: Client<'_>) -> Scene {
        Scene {
            rocks: view
                .terrain
                .iter()
                .map(|rock| RockView {
                    id: rock.rock,
                    pos: rock.orbit.at(view.time, view.gravity).pos,
                    radius: rock.radius,
                    caps: rock.caps,
                    pull: rock.pull,
                })
                .collect(),
            entities: view
                .present
                .iter()
                .map(|present| EntityView {
                    seat: present.seat,
                    glyph: Glyph::of(&roster[present.row]),
                    pos: present.body.pos,
                    range: reach(roster, present.row, present.at.standing().is_some()),
                })
                .collect(),
            wheels: Sectors::of(view, client.fights, [client.selection, client.pointed])
                .previewed(view, roster, client.hover.as_ref())
                .drawn(),
            flights: view
                .present
                .iter()
                .filter(|present| present.at.flying_from().is_some())
                .map(|present| FlightLine {
                    from: present.body.pos,
                    to: present.home,
                })
                .collect(),
            strip: Some(StripView {
                stockpile: view.stockpile,
                income: view.income,
                spend: view.spend,
                elapsed: view.time,
                clock: view.length,
            }),
            zone: view.zone,
            seat: view.seat,
            selection: client.selection,
            hover: client.hover,
        }
    }

    pub fn wheel_of(&self, rock: RockId) -> Option<&WheelView> {
        self.wheels.iter().find(|wheel| wheel.rock == rock)
    }
}

impl Entry {
    pub fn fill(self) -> Fill {
        match self {
            Entry::Present(_) | Entry::Leaving { .. } => Fill::Solid,
            Entry::Building(building) => Fill::Filling(building.progress as f32),
            Entry::Surplus(_) | Entry::Arriving { .. } => Fill::Hollow,
            Entry::Wanted { dashed: false, .. } | Entry::Placed => Fill::Hollow,
            Entry::Wanted { dashed: true, .. } => Fill::Dashed,
        }
    }

    pub fn dim(self) -> bool {
        matches!(self, Entry::Leaving { .. } | Entry::Arriving { .. })
    }

    pub fn count(self) -> Option<u32> {
        match self {
            Entry::Present(count)
            | Entry::Surplus(count)
            | Entry::Leaving { count, .. }
            | Entry::Arriving { count, .. }
            | Entry::Wanted { count, .. } => Some(count),
            Entry::Placed => Some(1),
            Entry::Building(_) => None,
        }
    }

    pub fn phrase(self, name: &str) -> String {
        match self {
            Entry::Present(_) => format!("{name} here"),
            Entry::Surplus(_) => format!("{name} surplus"),
            Entry::Leaving { to, .. } => format!("{name} leaving for {}", rock_name(to)),
            Entry::Building(Building {
                starved_of: Some(material),
                ..
            }) => format!(
                "{name} short of {}",
                label::material(material).to_ascii_lowercase()
            ),
            Entry::Building(_) => format!("{name} building"),
            Entry::Arriving { from, .. } => format!("{name} arriving from {}", rock_name(from)),
            Entry::Wanted { dashed: true, .. } => format!("No builder for {name}"),
            Entry::Wanted { dashed: false, .. } => format!("{name} wanted"),
            Entry::Placed => format!("{name} placed"),
        }
    }
}

impl WheelBand {
    pub fn step(self) -> u32 {
        match self {
            WheelBand::Plus(step) | WheelBand::Minus(step) => step,
        }
    }

    pub fn label(self) -> String {
        match self {
            WheelBand::Plus(1) => "+".to_string(),
            WheelBand::Minus(1) => "-".to_string(),
            WheelBand::Plus(step) => format!("+{step}"),
            WheelBand::Minus(step) => format!("-{step}"),
        }
    }

    pub fn wanted(self, from: u32) -> u32 {
        match self {
            WheelBand::Plus(step) => from.saturating_add(step).min(MAX_WANT),
            WheelBand::Minus(step) => from.saturating_sub(step),
        }
    }
}

pub fn rock_name(rock: RockId) -> String {
    format!("Rock {}", u64::from(rock.0) + 1)
}

fn reach(roster: &Roster, row: RowId, standing: bool) -> Option<f64> {
    let armed = roster[row].is_armed();
    (standing && armed).then(|| roster[row].max_damage_range())
}

type Rows = BTreeMap<RowId, Vec<Shown>>;

struct Sectors {
    rows: BTreeMap<(RockId, SeatId), Rows>,
    arcs: BTreeMap<(RockId, SeatId), Arc>,
}

impl Sectors {
    fn of(view: &View, fights: &Fights, full: [Option<RockId>; 2]) -> Sectors {
        let mut rows: BTreeMap<(RockId, SeatId), Rows> = BTreeMap::new();
        for composition in &view.compositions {
            let sector = rows
                .entry((composition.rock, composition.seat))
                .or_default();
            for (row, held) in &composition.rows {
                let entries = entries_of(view, composition.rock, composition.seat, *row, *held);
                if !entries.is_empty() {
                    sector.insert(*row, entries);
                }
            }
        }
        let placements: Vec<(RockId, SeatId, RowId)> = match view.draft.ended() {
            None => view
                .draft
                .stages()
                .iter()
                .filter_map(|stage| Some((stage.placed?, stage.seat, stage.row)))
                .collect(),
            Some(_) => Vec::new(),
        };
        for plan in &view.plans {
            let entries = rows
                .entry((plan.rock, view.seat))
                .or_default()
                .entry(plan.row)
                .or_default();
            if let Some(building) = plan.building {
                entries.push(shown(Entry::Building(building)));
            }
            let short = plan
                .want
                .saturating_sub(covered(view, plan.rock, plan.row) + wanted_frames(plan.building));
            if short > 0 && !placements.contains(&(plan.rock, view.seat, plan.row)) {
                entries.push(shown(Entry::Wanted {
                    count: short,
                    dashed: !builds_at(view, plan.rock, view.seat),
                }));
            }
            entries.sort_by_key(order);
        }
        for (rock, seat, row) in placements {
            rows.entry((rock, seat))
                .or_default()
                .entry(row)
                .or_default()
                .push(shown(Entry::Placed));
        }
        let arcs: BTreeMap<(RockId, SeatId), Arc> = fights
            .arcs()
            .map(|(rock, arc)| ((rock, arc.seat), arc))
            .collect();
        for (rock, seat) in arcs.keys() {
            rows.entry((*rock, *seat)).or_default();
        }
        for rock in full.into_iter().flatten() {
            rows.entry((rock, view.seat)).or_default();
        }
        Sectors { rows, arcs }
    }

    fn previewed(mut self, view: &View, roster: &Roster, hover: Option<&Hover>) -> Sectors {
        match hover {
            None | Some(Hover::Wheel { .. }) => {}
            Some(Hover::Send(sending)) => {
                for (row, count) in sending.rows(view, roster) {
                    self.moving(view.seat, *sending, row, count);
                }
            }
        }
        self
    }

    fn entries(&mut self, rock: RockId, seat: SeatId, row: RowId) -> &mut Vec<Shown> {
        self.rows
            .entry((rock, seat))
            .or_default()
            .entry(row)
            .or_default()
    }

    fn moving(&mut self, seat: SeatId, sending: Sending, row: RowId, count: u32) {
        for shown in self
            .entries(sending.from, seat, row)
            .iter_mut()
            .filter(|shown| matches!(shown.entry, Entry::Present(_)))
        {
            shown.previewed = true;
        }
        let arriving = self.entries(sending.to, seat, row);
        arriving.push(Shown {
            entry: Entry::Arriving {
                count,
                from: sending.from,
            },
            previewed: true,
        });
        arriving.sort_by_key(order);
    }

    fn drawn(self) -> Vec<WheelView> {
        let mut wheels: BTreeMap<RockId, WheelView> = BTreeMap::new();
        for ((rock, seat), rows) in self.rows {
            wheels
                .entry(rock)
                .or_insert_with(|| WheelView {
                    rock,
                    sectors: Vec::new(),
                })
                .sectors
                .push(SectorView {
                    seat,
                    rows: rows
                        .into_iter()
                        .filter(|(_, entries)| !entries.is_empty())
                        .map(|(row, entries)| RowView { row, entries })
                        .collect(),
                    arc: self.arcs.get(&(rock, seat)).copied(),
                });
        }
        wheels.into_values().collect()
    }
}

fn order(shown: &Shown) -> u8 {
    match shown.entry {
        Entry::Present(_) => 0,
        Entry::Surplus(_) => 1,
        Entry::Leaving { .. } => 2,
        Entry::Building(_) => 3,
        Entry::Arriving { .. } => 4,
        Entry::Wanted { .. } | Entry::Placed => 5,
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

fn entries_of(view: &View, rock: RockId, seat: SeatId, row: RowId, held: Held) -> Vec<Shown> {
    let mut entries = Vec::new();
    if held.present > 0 {
        entries.push(shown(Entry::Present(held.present)));
    }
    let surplus = match seat == view.seat {
        true => (held.present + held.arriving).saturating_sub(want_at(view, rock, row)),
        false => 0,
    };
    if surplus > 0 {
        entries.push(shown(Entry::Surplus(surplus)));
    }
    if let Some(to) = leaving_for(view, rock, seat, row)
        && held.leaving > 0
    {
        entries.push(shown(Entry::Leaving {
            count: held.leaving,
            to,
        }));
    }
    if let Some(from) = arriving_from(view, rock, seat, row)
        && held.arriving > 0
    {
        entries.push(shown(Entry::Arriving {
            count: held.arriving,
            from,
        }));
    }
    entries
}

fn leaving_for(view: &View, rock: RockId, seat: SeatId, row: RowId) -> Option<RockId> {
    view.present
        .iter()
        .filter(|unit| unit.seat == seat && unit.row == row)
        .find(|unit| unit.at.standing() == Some(rock) && unit.home != rock)
        .map(|unit| unit.home)
}

fn arriving_from(view: &View, rock: RockId, seat: SeatId, row: RowId) -> Option<RockId> {
    view.present
        .iter()
        .filter(|unit| unit.seat == seat && unit.row == row && unit.home == rock)
        .find_map(|unit| match unit.at {
            Berth::Standing(from) if from != rock => Some(from),
            Berth::Flying { from } => Some(from),
            Berth::Standing(_) => None,
        })
}

fn covered(view: &View, rock: RockId, row: RowId) -> u32 {
    view.compositions
        .iter()
        .find(|composition| composition.rock == rock && composition.seat == view.seat)
        .and_then(|composition| composition.rows.get(&row))
        .map_or(0, |held| held.present + held.arriving)
}

fn want_at(view: &View, rock: RockId, row: RowId) -> u32 {
    view.plan_of(rock, row).map_or(0, |plan| plan.want)
}

fn builds_at(view: &View, rock: RockId, seat: SeatId) -> bool {
    view.compositions
        .iter()
        .find(|composition| composition.rock == rock && composition.seat == seat)
        .is_some_and(|composition| composition.builder)
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD, STORAGE};
    use neumannarch_sim::state::Send;

    use super::*;
    use crate::display::local::{Local, PLAYER, RIVAL};

    fn at(rock: u32) -> RockId {
        RockId(rock)
    }

    fn scene(local: &Local) -> Scene {
        drawn(local, &Fights::default(), None, None)
    }

    fn drawn(
        local: &Local,
        fights: &Fights,
        selection: Option<RockId>,
        hover: Option<Hover>,
    ) -> Scene {
        Scene::from_view(
            &local.view(),
            local.session().state().roster(),
            Client {
                selection,
                pointed: None,
                hover,
                fights,
            },
        )
    }

    fn entries(scene: &Scene, rock: RockId, seat: SeatId, row: RowId) -> Vec<Shown> {
        scene
            .wheel_of(rock)
            .into_iter()
            .flat_map(|wheel| &wheel.sectors)
            .filter(|sector| sector.seat == seat)
            .flat_map(|sector| &sector.rows)
            .filter(|shown| shown.row == row)
            .flat_map(|shown| shown.entries.clone())
            .collect()
    }

    #[test]
    fn a_rock_holding_no_composition_carries_no_wheel() {
        let mut local = Local::start(2);

        assert!(scene(&local).wheels.is_empty(), "a fresh belt draws none");

        local.want(&[(at(0), SHIPYARD, 1)]);
        let scene = scene(&local);

        assert_eq!(scene.wheels.len(), 1, "one wheel, at the rock it wants at");
        assert_eq!(scene.wheels[0].rock, at(0));
        assert!(scene.wheel_of(at(5)).is_none());
    }

    #[test]
    fn a_draft_placement_stands_on_its_rock_as_a_placed_line_until_the_clock_starts() {
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
        for (rock, (seat, stage)) in [(at(0), first), (at(1), second)] {
            assert_eq!(
                entries(&drafting, rock, seat, stage.row),
                vec![shown(Entry::Placed)],
                "the placement is the one line at its rock"
            );
        }
        assert_eq!(Entry::Placed.count(), Some(1));
        assert_eq!(Entry::Placed.fill(), Fill::Hollow);
        assert_eq!(Entry::Placed.phrase("Shipyard"), "Shipyard placed");

        local.start_the_clock();
        local.run(1);
        let started = scene(&local);
        assert!(
            started.wheels.iter().all(|wheel| {
                wheel.sectors.iter().all(|sector| {
                    sector
                        .rows
                        .iter()
                        .all(|row| row.entries.iter().all(|shown| shown.entry != Entry::Placed))
                })
            }),
            "once the clock runs the structure stands and the line is gone"
        );
    }

    #[test]
    fn a_selected_rock_carries_the_seats_own_wheel_though_it_holds_nothing() {
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
                pointed: Some(at(5)),
                hover: None,
                fights: &Fights::default(),
            },
        );
        assert_eq!(
            pointed.wheel_of(at(5)),
            Some(wheel),
            "the rock under the pointer carries the same wheel"
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
            "no builder stands at the far rock"
        );
        assert_eq!(unbuilt[1].entry.fill(), Fill::Dashed);
        assert_eq!(unbuilt[1].entry.phrase("Frigate"), "No builder for Frigate");
    }

    #[test]
    fn a_forming_send_is_leaving_its_rock_and_arriving_at_the_other() {
        let mut local = Local::start(2);
        local.want(&[(at(0), CONSTRUCTOR, 1)]);
        local.want(&[(at(0), CONSTRUCTOR, 0), (at(1), CONSTRUCTOR, 1)]);
        let scene = scene(&local);

        let leaving = entries(&scene, at(0), PLAYER, CONSTRUCTOR);
        assert_eq!(
            leaving[0].entry,
            Entry::Leaving {
                count: 1,
                to: at(1)
            }
        );
        assert!(leaving[0].entry.dim(), "a unit on its way out is dimmed");
        assert_eq!(
            leaving[0].entry.phrase("Constructor"),
            "Constructor leaving for Rock 2"
        );

        let arriving = entries(&scene, at(1), PLAYER, CONSTRUCTOR);
        assert_eq!(
            arriving[0].entry,
            Entry::Arriving {
                count: 1,
                from: at(0)
            }
        );
        assert_eq!(
            arriving[0].entry.phrase("Constructor"),
            "Constructor arriving from Rock 1"
        );
    }

    #[test]
    fn a_flying_unit_leaves_its_rocks_wheel_and_flies_on_the_belt() {
        let mut local = Local::start(2);
        local.want(&[(at(0), CONSTRUCTOR, 1)]);
        local.want(&[(at(0), CONSTRUCTOR, 0), (at(1), CONSTRUCTOR, 1)]);
        local.run(Send::FORMING_TICKS + 1);
        let scene = scene(&local);

        assert_eq!(scene.flights.len(), 1);
        assert_eq!(scene.flights[0].to, at(1));
        assert!(
            entries(&scene, at(0), PLAYER, CONSTRUCTOR).is_empty(),
            "the wheel forgets a ship in flight"
        );
        assert_eq!(
            entries(&scene, at(1), PLAYER, CONSTRUCTOR)[0].entry,
            Entry::Arriving {
                count: 1,
                from: at(0)
            }
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
    fn a_hovered_band_changes_no_entry() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        local.want(&[(at(0), SHIPYARD, 3)]);
        let hover = |row, band| {
            let scene = drawn(
                &local,
                &Fights::default(),
                Some(at(0)),
                Some(Hover::Wheel {
                    rock: at(0),
                    row,
                    band,
                }),
            );
            entries(&scene, at(0), PLAYER, row)
        };
        let still = scene(&local);

        assert_eq!(
            hover(SHIPYARD, WheelBand::Minus(1)),
            entries(&still, at(0), PLAYER, SHIPYARD),
            "minus leaves what stands and what is wanted as they are"
        );
        assert!(
            hover(FRIGATE, WheelBand::Plus(5)).is_empty(),
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

        let scene = drawn(&local, &Fights::default(), None, Some(Hover::Send(sending)));

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
    }

    #[test]
    fn an_armed_ship_at_a_rock_carries_its_range_and_one_in_flight_carries_none() {
        let mut local = Local::start(2);
        local.want(&[(at(0), SHIPYARD, 1)]);
        let roster = local.session().state().roster();

        assert_eq!(
            reach(roster, FRIGATE, true),
            Some(roster[FRIGATE].max_damage_range()),
            "a frigate at a rock draws its longest range"
        );
        assert_eq!(reach(roster, FRIGATE, false), None, "and none in flight");
        assert_eq!(reach(roster, SHIPYARD, true), None, "a shipyard is unarmed");

        let scene = scene(&local);
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].range, None);
        assert_eq!(scene.zone, local.view().zone);
    }

    #[test]
    fn a_match_scene_carries_the_seats_stockpile_and_the_clock_and_a_belt_scene_none() {
        let local = Local::start(2);
        let view = local.view();

        let strip = scene(&local).strip.expect("a match has a strip");
        assert_eq!(strip.stockpile, view.stockpile);
        assert_eq!(strip.income, view.income);
        assert_eq!(strip.spend, view.spend);
        assert_eq!(strip.elapsed, view.time);
        assert_eq!(strip.clock, view.length);
        assert!(
            scene(&local)
                .rocks
                .iter()
                .zip(&view.terrain)
                .all(|(rock, terrain)| rock.pull == terrain.pull),
            "every rock carries its pull"
        );

        let belt = Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Time::ZERO);
        assert_eq!(belt.strip, None);
        assert!(belt.rocks.iter().all(|rock| rock.pull == Materials::ZERO));
    }
}
