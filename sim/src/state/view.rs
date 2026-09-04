use std::collections::BTreeMap;

use super::State;
use super::seat::Seat;
use crate::belt::Belt;
use crate::ids::{EntityId, RockId, RowId, SeatId, TeamId};
use crate::materials::{Material, Materials, Stockpile};
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::state::entity::Entity;
use crate::state::standings::Standings;
use crate::state::{Frame, Held};
use crate::step::fire::{Exchange, Shots};
use crate::time::Tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Berth {
    Standing(RockId),
    Flying { from: RockId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Present {
    pub id: EntityId,
    pub row: RowId,
    pub seat: SeatId,
    pub body: Body,
    pub hp: f64,
    pub home: RockId,
    pub at: Berth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terrain {
    pub rock: RockId,
    pub orbit: Orbit,
    pub caps: Materials,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub progress: f64,
    pub starved_of: Option<Material>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub rock: RockId,
    pub seat: SeatId,
    pub builder: bool,
    pub rows: BTreeMap<RowId, Held>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub rock: RockId,
    pub row: RowId,
    pub want: u32,
    pub building: Option<Building>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub seat: SeatId,
    pub tick: Tick,
    pub clock: Tick,
    pub gravity: Gravity,
    pub stockpile: Stockpile,
    pub reserve: BTreeMap<RowId, u32>,
    pub compositions: Vec<Composition>,
    pub plans: Vec<Plan>,
    pub present: Vec<Present>,
    pub teams: Box<[TeamId]>,
    pub exchanges: Vec<Exchange>,
    pub terrain: Vec<Terrain>,
    pub zone: f64,
    pub standings: Standings,
}

impl View {
    pub fn of(state: &State, seat: SeatId, shots: &Shots) -> View {
        View {
            seat,
            tick: state.tick(),
            clock: state.clock(),
            gravity: state.gravity(),
            stockpile: state
                .seat(seat)
                .map_or_else(Stockpile::default, |seat| *seat.stockpile()),
            reserve: state
                .seat(seat)
                .map_or_else(BTreeMap::new, |seat| seat.reserve().clone()),
            compositions: compositions(state, seat),
            plans: plans(state, seat),
            present: present(state),
            teams: state.seats().iter().map(Seat::team).collect(),
            exchanges: shots.exchanges(state),
            terrain: terrain(state),
            zone: Belt::ZONE_RADIUS_METERS,
            standings: state.standings(),
        }
    }

    pub fn team_of(&self, seat: SeatId) -> Option<TeamId> {
        self.teams.get(usize::from(seat.0)).copied()
    }

    pub fn is_enemy(&self, seat: SeatId) -> bool {
        match (self.team_of(self.seat), self.team_of(seat)) {
            (Some(mine), Some(theirs)) => mine != theirs,
            _ => false,
        }
    }

    pub fn terrain_of(&self, id: RockId) -> Option<&Terrain> {
        self.terrain.get(id.0 as usize)
    }

    pub fn rock_body(&self, id: RockId) -> Option<Body> {
        self.terrain_of(id)
            .map(|terrain| terrain.orbit.at(self.tick, self.gravity))
    }

    pub fn plan_of(&self, rock: RockId, row: RowId) -> Option<&Plan> {
        self.plans
            .iter()
            .find(|plan| plan.rock == rock && plan.row == row)
    }
}

impl Berth {
    fn of(entity: &Entity, now: Tick) -> Berth {
        match entity.flight() {
            Some(flight) if flight.has_departed(now) => Berth::Flying {
                from: flight.source(),
            },
            Some(flight) => Berth::Standing(flight.source()),
            None => Berth::Standing(entity.home()),
        }
    }

    pub fn standing(self) -> Option<RockId> {
        match self {
            Berth::Standing(rock) => Some(rock),
            Berth::Flying { .. } => None,
        }
    }

    pub fn flying_from(self) -> Option<RockId> {
        match self {
            Berth::Standing(_) => None,
            Berth::Flying { from } => Some(from),
        }
    }
}

fn compositions(state: &State, seat: SeatId) -> Vec<Composition> {
    let mut rows: BTreeMap<(RockId, SeatId), BTreeMap<RowId, Held>> = BTreeMap::new();
    for ((rock, at, row), held) in state.holdings() {
        rows.entry((rock, at)).or_default().insert(row, held);
    }
    for (post, _) in state.posts().filter(|(post, _)| post.seat == seat) {
        rows.entry((post.rock, post.seat)).or_default();
    }
    rows.into_iter()
        .map(|((rock, at), rows)| Composition {
            rock,
            seat: at,
            builder: builds_at(state, rock, at),
            rows,
        })
        .collect()
}

fn builds_at(state: &State, rock: RockId, seat: SeatId) -> bool {
    state
        .standing_at(rock)
        .filter(|entity| entity.seat() == seat && entity.flight().is_none())
        .any(|entity| state[entity.row()].builds().next().is_some())
}

fn plans(state: &State, seat: SeatId) -> Vec<Plan> {
    let mut plans: BTreeMap<(RockId, RowId), Plan> = BTreeMap::new();
    for (post, wants) in state.posts().filter(|(post, _)| post.seat == seat) {
        for (row, want) in wants.iter() {
            planned(&mut plans, post.rock, row).want = want;
        }
    }
    for frame in state
        .frames()
        .iter()
        .filter(|frame| frame.post().seat == seat)
    {
        let post = frame.post();
        planned(&mut plans, post.rock, frame.row()).building = Some(building(state, frame));
    }
    plans.into_values().collect()
}

fn planned(plans: &mut BTreeMap<(RockId, RowId), Plan>, rock: RockId, row: RowId) -> &mut Plan {
    plans.entry((rock, row)).or_insert(Plan {
        rock,
        row,
        want: 0,
        building: None,
    })
}

fn building(state: &State, frame: &Frame) -> Building {
    Building {
        progress: frame.fraction(state[frame.row()].cost.total()),
        starved_of: frame.starved_material(state.tick()),
    }
}

fn present(state: &State) -> Vec<Present> {
    state
        .entities()
        .map(|entity| Present {
            id: entity.id(),
            row: entity.row(),
            seat: entity.seat(),
            body: state.body_of(entity),
            hp: entity.hp(),
            home: entity.home(),
            at: Berth::of(entity, state.tick()),
        })
        .collect()
}

fn terrain(state: &State) -> Vec<Terrain> {
    state
        .rocks()
        .iter()
        .enumerate()
        .map(|(at, rock)| Terrain {
            rock: RockId(at as u32),
            orbit: *rock.orbit(),
            caps: rock.caps(),
            radius: rock.radius(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD, STORAGE};
    use crate::state::{Issued, Send};

    const ROCK: RockId = RockId(0);

    const AWAY: RockId = RockId(1);

    fn world() -> World {
        World::started(&[TeamId(0), TeamId(1)])
    }

    fn holding(view: &View, rock: RockId, seat: u8, row: RowId) -> Held {
        view.compositions
            .iter()
            .find(|composition| composition.rock == rock && composition.seat == SeatId(seat))
            .and_then(|composition| composition.rows.get(&row).copied())
            .unwrap_or_default()
    }

    fn sent(seat: u8, from: RockId, to: RockId, row: RowId) -> [Issued; 2] {
        [
            Issued::numbered(seat, 0, from, row, 0),
            Issued::numbered(seat, 1, to, row, 1),
        ]
    }

    #[test]
    fn a_view_carries_every_seats_holdings_and_only_its_own_plans() {
        let mut world = world();
        world.fix(0, SHIPYARD, ROCK);
        world.fix(1, STORAGE, AWAY);
        world.tick(&[
            Issued::want(0, ROCK, FRIGATE, 2),
            Issued::want(1, AWAY, FRIGATE, 3),
        ]);

        let view = world.view(0);

        assert_eq!(holding(&view, ROCK, 0, SHIPYARD).present, 1);
        assert_eq!(
            holding(&view, AWAY, 1, STORAGE).present,
            1,
            "another seat's holdings are visible"
        );
        assert_eq!(
            view.plan_of(ROCK, FRIGATE).map(|plan| plan.want),
            Some(2),
            "the viewer's own want is in the view"
        );
        assert!(
            view.plans.iter().all(|plan| plan.rock == ROCK),
            "another seat's wants are its own to see"
        );
        assert_eq!(view.reserve[&SHIPYARD], 1);
    }

    #[test]
    fn a_composition_says_whether_a_builder_of_its_seat_stands_at_the_rock() {
        let mut world = world();
        world.fix(0, SHIPYARD, ROCK);
        world.fix(0, STORAGE, AWAY);
        world.tick(&[]);

        let view = world.view(0);
        let builder = |rock: RockId| {
            view.compositions
                .iter()
                .find(|composition| composition.rock == rock && composition.seat == SeatId(0))
                .map(|composition| composition.builder)
        };

        assert_eq!(builder(ROCK), Some(true), "a shipyard builds");
        assert_eq!(builder(AWAY), Some(false), "a storage does not");
    }

    #[test]
    fn a_forming_send_is_leaving_its_source_and_arriving_at_its_destination() {
        let mut world = world();
        world.hold(0, FRIGATE, ROCK, 0.0);
        world.tick(&sent(0, ROCK, AWAY, FRIGATE));

        let view = world.view(0);

        let source = holding(&view, ROCK, 0, FRIGATE);
        assert_eq!(source.leaving, 1, "it stands at the rock it is leaving");
        assert_eq!(source.present, 0, "and is no longer counted as held there");
        assert_eq!(holding(&view, AWAY, 0, FRIGATE).arriving, 1);
        assert_eq!(view.present[0].at, Berth::Standing(ROCK));
    }

    #[test]
    fn a_flying_send_leaves_its_source_holding_nothing() {
        let mut world = world();
        world.hold(0, FRIGATE, ROCK, 0.0);
        world.tick(&sent(0, ROCK, AWAY, FRIGATE));
        world.run(Send::FORMING_TICKS + 1);

        let view = world.view(0);

        assert_eq!(holding(&view, ROCK, 0, FRIGATE), Held::default());
        assert_eq!(holding(&view, AWAY, 0, FRIGATE).arriving, 1);
        assert_eq!(view.present[0].at, Berth::Flying { from: ROCK });
        assert_eq!(view.present[0].home, AWAY);
    }

    #[test]
    fn a_plan_carries_the_one_frame_its_row_is_building() {
        let mut world = world();
        world.fix(0, SHIPYARD, ROCK);
        world.tick(&[Issued::want(0, ROCK, FRIGATE, 3)]);
        world.run(60);

        let plan = world
            .view(0)
            .plan_of(ROCK, FRIGATE)
            .copied()
            .expect("the frigate is wanted");

        assert_eq!(plan.want, 3);
        assert_eq!(world.frames(0, ROCK, FRIGATE), 1, "one frame at a time");
        let building = plan.building.expect("the shipyard is building one");
        assert!(building.progress > 0.0 && building.progress < 1.0);
    }

    #[test]
    fn every_seat_holds_every_entity_of_the_match() {
        let mut world = world();
        let mine = world.fix(0, SHIPYARD, ROCK);
        let near = world.fix(1, STORAGE, ROCK);
        let far = world.fix(1, STORAGE, RockId(9));

        for seat in [0, 1, 9] {
            let held: Vec<EntityId> = world.view(seat).present.iter().map(|it| it.id).collect();
            assert_eq!(held, vec![mine, near, far], "seat {seat} was told less");
        }
        let theirs = world.present(0, far).expect("an enemy a belt away");
        assert_eq!(theirs.seat, SeatId(1));
        assert_eq!(theirs.row, STORAGE);
        assert_eq!(theirs.home, RockId(9));
        assert_eq!(theirs.hp, world.state[STORAGE].hp.0);
        assert_eq!(theirs.body, world.state.rock_body(RockId(9)));
        assert_eq!(theirs.at, Berth::Standing(RockId(9)));
    }

    #[test]
    fn a_flying_unit_names_the_rock_it_left_and_the_one_it_flies_to() {
        let mut world = world();
        let mine = world.hold(0, FRIGATE, ROCK, 0.0);
        let theirs = world.hold(1, FRIGATE, ROCK, 0.0);
        world.tick(&[
            Issued::numbered(0, 0, ROCK, FRIGATE, 0),
            Issued::numbered(0, 1, AWAY, FRIGATE, 1),
            Issued::numbered(1, 0, ROCK, FRIGATE, 0),
            Issued::numbered(1, 1, AWAY, FRIGATE, 1),
        ]);
        world.run(Send::FORMING_TICKS + 1);

        for unit in [mine, theirs] {
            let flier = world.present(0, unit).expect("both fliers are in the view");
            assert_eq!(flier.home, AWAY, "a send names where to");
            assert_eq!(flier.at, Berth::Flying { from: ROCK });
            assert_eq!(flier.at.standing(), None);
            assert_eq!(flier.at.flying_from(), Some(ROCK));
        }
    }

    #[test]
    fn the_standings_are_in_every_view_and_say_whether_the_clock_has_run() {
        let mut world = world();
        world.fix(0, SHIPYARD, ROCK);

        let standings = world.view(1).standings;

        assert!(!standings.over());
        assert_eq!(standings.teams().len(), 2);
        assert_eq!(standings.teams()[0].rocks, 1, "team zero holds one rock");
        assert_eq!(standings.teams()[1].rocks, 0);

        let ended = World::timed(&[TeamId(0), TeamId(1)], Tick::ZERO);

        assert!(ended.view(0).standings.over());
    }

    #[test]
    fn a_seat_the_match_lacks_holds_no_composition_of_its_own() {
        let mut world = world();
        world.fix(0, CONSTRUCTOR, ROCK);

        let view = world.view(9);

        assert!(view.plans.is_empty());
        assert!(view.reserve.is_empty());
        assert!(
            view.compositions
                .iter()
                .all(|composition| composition.seat == SeatId(0)),
            "only the seats that hold anything carry a composition"
        );
        assert_eq!(view.terrain.len(), 21);
    }

    #[test]
    fn a_view_reads_its_terrain_at_the_gravity_it_carries() {
        let world = world();

        let view = world.view(0);

        assert_eq!(view.gravity, world.state.gravity());
        let rock = view.terrain[3];
        assert_eq!(
            rock.orbit.at(view.tick, view.gravity),
            world.state.rock_body(rock.rock)
        );
    }

    #[test]
    fn a_view_reads_a_rock_by_id() {
        let world = world();
        let at = RockId(4);

        let view = world.view(0);

        assert_eq!(view.terrain_of(at).map(|rock| rock.rock), Some(at));
        assert_eq!(view.rock_body(at), Some(world.state.rock_body(at)));
        assert_eq!(view.terrain_of(RockId(99)), None);
        assert_eq!(view.rock_body(RockId(99)), None);
    }

    #[test]
    fn an_exchange_names_the_rock_the_shooter_fired_from_and_the_target_was_hit_at() {
        let mut world = world();
        let shooter = world.hold(0, FRIGATE, ROCK, 0.0);
        let target = world.fix(1, STORAGE, ROCK);
        let shots = world.shots();
        assert!(
            shots
                .hits
                .iter()
                .any(|hit| hit.shooter == shooter && hit.target == target),
            "the frigate fired on the storage"
        );

        let view = View::of(&world.state, SeatId(0), &shots);

        assert_eq!(
            view.exchanges,
            vec![
                Exchange {
                    rock: ROCK,
                    seat: SeatId(0),
                    fired: true,
                    landed: false,
                },
                Exchange {
                    rock: ROCK,
                    seat: SeatId(1),
                    fired: false,
                    landed: true,
                },
            ]
        );
    }
}
