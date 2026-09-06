use std::collections::BTreeMap;

use super::State;
use super::seat::Seat;
use crate::belt::Belt;
use crate::ids::{AsteroidId, EntityId, RowId, SeatId, TeamId};
use crate::materials::{Material, Materials, Stockpile};
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::state::draft::Draft;
use crate::state::entity::Entity;
use crate::state::standings::Standings;
use crate::state::{Frame, Held};
use crate::step::fire::{Exchange, Shots};
use crate::time::{Tick, Time};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Berth {
    Standing(AsteroidId),
    Flying { from: AsteroidId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Present {
    pub id: EntityId,
    pub row: RowId,
    pub seat: SeatId,
    pub body: Body,
    pub hp: f64,
    pub home: AsteroidId,
    pub at: Berth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terrain {
    pub asteroid: AsteroidId,
    pub orbit: Orbit,
    pub caps: Materials,
    pub radius: f64,
    pub pull: Materials,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub progress: f64,
    pub starved_of: Option<Material>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub asteroid: AsteroidId,
    pub seat: SeatId,
    pub builder: bool,
    pub rows: BTreeMap<RowId, Held>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub asteroid: AsteroidId,
    pub row: RowId,
    pub want: u32,
    pub building: Option<Building>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub seat: SeatId,
    pub tick: Tick,
    pub time: Time,
    pub length: Time,
    pub gravity: Gravity,
    pub draft: Draft,
    pub stockpile: Stockpile,
    pub income: Materials,
    pub spend: Materials,
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
        let seated = state.seat(seat);
        View {
            seat,
            tick: state.tick(),
            time: state.time(),
            length: state.length(),
            gravity: state.gravity(),
            draft: state.draft().clone(),
            stockpile: seated.map_or_else(Stockpile::default, |seat| *seat.stockpile()),
            income: seated.map_or(Materials::ZERO, Seat::income),
            spend: seated.map_or(Materials::ZERO, Seat::spend),
            reserve: seated.map_or_else(BTreeMap::new, |seat| seat.reserve().clone()),
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

    pub fn terrain_of(&self, id: AsteroidId) -> Option<&Terrain> {
        self.terrain.get(id.0 as usize)
    }

    pub fn asteroid_body(&self, id: AsteroidId) -> Option<Body> {
        self.terrain_of(id)
            .map(|terrain| terrain.orbit.at(self.time, self.gravity))
    }

    pub fn plan_of(&self, asteroid: AsteroidId, row: RowId) -> Option<&Plan> {
        self.plans
            .iter()
            .find(|plan| plan.asteroid == asteroid && plan.row == row)
    }
}

impl Berth {
    fn of(entity: &Entity, now: Time) -> Berth {
        match entity.flight() {
            Some(flight) if flight.has_departed(now) => Berth::Flying {
                from: flight.source(),
            },
            Some(flight) => Berth::Standing(flight.source()),
            None => Berth::Standing(entity.home()),
        }
    }

    pub fn standing(self) -> Option<AsteroidId> {
        match self {
            Berth::Standing(asteroid) => Some(asteroid),
            Berth::Flying { .. } => None,
        }
    }

    pub fn flying_from(self) -> Option<AsteroidId> {
        match self {
            Berth::Standing(_) => None,
            Berth::Flying { from } => Some(from),
        }
    }
}

fn compositions(state: &State, seat: SeatId) -> Vec<Composition> {
    let mut rows: BTreeMap<(AsteroidId, SeatId), BTreeMap<RowId, Held>> = BTreeMap::new();
    for ((asteroid, at, row), held) in state.holdings() {
        rows.entry((asteroid, at)).or_default().insert(row, held);
    }
    for (post, _) in state.posts().filter(|(post, _)| post.seat == seat) {
        rows.entry((post.asteroid, post.seat)).or_default();
    }
    rows.into_iter()
        .map(|((asteroid, at), rows)| Composition {
            asteroid,
            seat: at,
            builder: builds_at(state, asteroid, at),
            rows,
        })
        .collect()
}

fn builds_at(state: &State, asteroid: AsteroidId, seat: SeatId) -> bool {
    state
        .standing_at(asteroid)
        .filter(|entity| entity.seat() == seat && entity.flight().is_none())
        .any(|entity| state[entity.row()].builds().next().is_some())
}

fn plans(state: &State, seat: SeatId) -> Vec<Plan> {
    let mut plans: BTreeMap<(AsteroidId, RowId), Plan> = BTreeMap::new();
    for (post, wants) in state.posts().filter(|(post, _)| post.seat == seat) {
        for (row, want) in wants.iter() {
            planned(&mut plans, post.asteroid, row).want = want;
        }
    }
    for frame in state
        .frames()
        .iter()
        .filter(|frame| frame.post().seat == seat)
    {
        let post = frame.post();
        planned(&mut plans, post.asteroid, frame.row()).building = Some(building(state, frame));
    }
    plans.into_values().collect()
}

fn planned(
    plans: &mut BTreeMap<(AsteroidId, RowId), Plan>,
    asteroid: AsteroidId,
    row: RowId,
) -> &mut Plan {
    plans.entry((asteroid, row)).or_insert(Plan {
        asteroid,
        row,
        want: 0,
        building: None,
    })
}

fn building(state: &State, frame: &Frame) -> Building {
    Building {
        progress: frame.fraction(state[frame.row()].cost.total()),
        starved_of: frame.starved_material(state.time()),
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
            at: Berth::of(entity, state.time()),
        })
        .collect()
}

fn terrain(state: &State) -> Vec<Terrain> {
    state
        .asteroids()
        .map(|(id, asteroid)| Terrain {
            asteroid: id,
            orbit: *asteroid.orbit(),
            caps: asteroid.caps(),
            radius: asteroid.radius(),
            pull: asteroid.pull(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::roster::{
        CONSTRUCTOR, FRIGATE, METALS_EXTRACTOR, SHIPYARD, STORAGE, VOLATILES_EXTRACTOR,
    };
    use crate::state::{Issued, Send};

    const ASTEROID: AsteroidId = AsteroidId(0);

    const AWAY: AsteroidId = AsteroidId(1);

    const SECOND: u64 = TICKS_PER_SECOND as u64;

    fn world() -> World {
        World::started(&[TeamId(0), TeamId(1)])
    }

    fn close(got: Materials, wanted: Materials) -> bool {
        (got - wanted).map(f64::abs).total() < 1e-9
    }

    fn pulled(world: &World, seat: u8) -> Materials {
        world
            .view(seat)
            .terrain_of(ASTEROID)
            .expect("every view")
            .pull
    }

    fn holding(view: &View, asteroid: AsteroidId, seat: u8, row: RowId) -> Held {
        view.compositions
            .iter()
            .find(|composition| {
                composition.asteroid == asteroid && composition.seat == SeatId(seat)
            })
            .and_then(|composition| composition.rows.get(&row).copied())
            .unwrap_or_default()
    }

    fn sent(seat: u8, from: AsteroidId, to: AsteroidId, row: RowId) -> [Issued; 2] {
        [
            Issued::numbered(seat, 0, from, row, 0),
            Issued::numbered(seat, 1, to, row, 1),
        ]
    }

    #[test]
    fn a_view_carries_every_seats_holdings_and_only_its_own_plans() {
        let mut world = world();
        world.fix(0, SHIPYARD, ASTEROID);
        world.fix(1, STORAGE, AWAY);
        world.tick(&[
            Issued::want(0, ASTEROID, FRIGATE, 2),
            Issued::want(1, AWAY, FRIGATE, 3),
        ]);

        let view = world.view(0);

        assert_eq!(holding(&view, ASTEROID, 0, SHIPYARD).present, 1);
        assert_eq!(
            holding(&view, AWAY, 1, STORAGE).present,
            1,
            "another seat's holdings are visible"
        );
        assert_eq!(
            view.plan_of(ASTEROID, FRIGATE).map(|plan| plan.want),
            Some(2),
            "the viewer's own want is in the view"
        );
        assert!(
            view.plans.iter().all(|plan| plan.asteroid == ASTEROID),
            "another seat's wants are its own to see"
        );
        assert_eq!(view.reserve[&SHIPYARD], 1);
    }

    #[test]
    fn a_composition_says_whether_a_builder_of_its_seat_stands_at_the_asteroid() {
        let mut world = world();
        world.fix(0, SHIPYARD, ASTEROID);
        world.fix(0, STORAGE, AWAY);
        world.tick(&[]);

        let view = world.view(0);
        let builder = |asteroid: AsteroidId| {
            view.compositions
                .iter()
                .find(|composition| {
                    composition.asteroid == asteroid && composition.seat == SeatId(0)
                })
                .map(|composition| composition.builder)
        };

        assert_eq!(builder(ASTEROID), Some(true), "a shipyard builds");
        assert_eq!(builder(AWAY), Some(false), "a storage does not");
    }

    #[test]
    fn a_forming_send_is_leaving_its_source_and_arriving_at_its_destination() {
        let mut world = world();
        world.hold(0, FRIGATE, ASTEROID, 0.0);
        world.tick(&sent(0, ASTEROID, AWAY, FRIGATE));

        let view = world.view(0);

        let source = holding(&view, ASTEROID, 0, FRIGATE);
        assert_eq!(source.leaving, 1, "it stands at the asteroid it is leaving");
        assert_eq!(source.present, 0, "and is no longer counted as held there");
        assert_eq!(holding(&view, AWAY, 0, FRIGATE).arriving, 1);
        assert_eq!(view.present[0].at, Berth::Standing(ASTEROID));
    }

    #[test]
    fn a_flying_send_leaves_its_source_holding_nothing() {
        let mut world = world();
        world.hold(0, FRIGATE, ASTEROID, 0.0);
        world.tick(&sent(0, ASTEROID, AWAY, FRIGATE));
        world.run(Send::FORMING_TICKS + 1);

        let view = world.view(0);

        assert_eq!(holding(&view, ASTEROID, 0, FRIGATE), Held::default());
        assert_eq!(holding(&view, AWAY, 0, FRIGATE).arriving, 1);
        assert_eq!(view.present[0].at, Berth::Flying { from: ASTEROID });
        assert_eq!(view.present[0].home, AWAY);
    }

    #[test]
    fn a_plan_carries_the_one_frame_its_row_is_building() {
        let mut world = world();
        world.fix(0, SHIPYARD, ASTEROID);
        world.tick(&[Issued::want(0, ASTEROID, FRIGATE, 3)]);
        world.run(60);

        let plan = world
            .view(0)
            .plan_of(ASTEROID, FRIGATE)
            .copied()
            .expect("the frigate is wanted");

        assert_eq!(plan.want, 3);
        assert_eq!(world.frames(0, ASTEROID, FRIGATE), 1, "one frame at a time");
        let building = plan.building.expect("the shipyard is building one");
        assert!(building.progress > 0.0 && building.progress < 1.0);
    }

    #[test]
    fn every_seat_holds_every_entity_of_the_match() {
        let mut world = world();
        let mine = world.fix(0, SHIPYARD, ASTEROID);
        let near = world.fix(1, STORAGE, ASTEROID);
        let far = world.fix(1, STORAGE, AsteroidId(9));

        for seat in [0, 1, 9] {
            let held: Vec<EntityId> = world.view(seat).present.iter().map(|it| it.id).collect();
            assert_eq!(held, vec![mine, near, far], "seat {seat} was told less");
        }
        let theirs = world.present(0, far).expect("an enemy a belt away");
        assert_eq!(theirs.seat, SeatId(1));
        assert_eq!(theirs.row, STORAGE);
        assert_eq!(theirs.home, AsteroidId(9));
        assert_eq!(theirs.hp, world.state[STORAGE].hp.0);
        assert_eq!(theirs.body, world.state.asteroid_body(AsteroidId(9)));
        assert_eq!(theirs.at, Berth::Standing(AsteroidId(9)));
    }

    #[test]
    fn a_flying_unit_names_the_asteroid_it_left_and_the_one_it_flies_to() {
        let mut world = world();
        let mine = world.hold(0, FRIGATE, ASTEROID, 0.0);
        let theirs = world.hold(1, FRIGATE, ASTEROID, 0.0);
        world.tick(&[
            Issued::numbered(0, 0, ASTEROID, FRIGATE, 0),
            Issued::numbered(0, 1, AWAY, FRIGATE, 1),
            Issued::numbered(1, 0, ASTEROID, FRIGATE, 0),
            Issued::numbered(1, 1, AWAY, FRIGATE, 1),
        ]);
        world.run(Send::FORMING_TICKS + 1);

        for unit in [mine, theirs] {
            let flier = world.present(0, unit).expect("both fliers are in the view");
            assert_eq!(flier.home, AWAY, "a send names where to");
            assert_eq!(flier.at, Berth::Flying { from: ASTEROID });
            assert_eq!(flier.at.standing(), None);
            assert_eq!(flier.at.flying_from(), Some(ASTEROID));
        }
    }

    #[test]
    fn the_standings_are_in_every_view_and_say_whether_the_clock_has_run() {
        let mut world = world();
        world.fix(0, SHIPYARD, ASTEROID);

        let standings = world.view(1).standings;

        assert!(!standings.over());
        assert_eq!(standings.teams().len(), 2);
        assert_eq!(
            standings.teams()[0].asteroids,
            1,
            "team zero holds one asteroid"
        );
        assert_eq!(standings.teams()[1].asteroids, 0);

        let ended = World::timed(&[TeamId(0), TeamId(1)], Time::ZERO);

        assert!(ended.view(0).standings.over());
    }

    #[test]
    fn a_seat_the_match_lacks_holds_no_composition_of_its_own() {
        let mut world = world();
        world.fix(0, CONSTRUCTOR, ASTEROID);

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
        let asteroid = view.terrain[3];
        assert_eq!(
            asteroid.orbit.at(view.time, view.gravity),
            world.state.asteroid_body(asteroid.asteroid)
        );
    }

    #[test]
    fn a_view_reads_a_asteroid_by_id() {
        let world = world();
        let at = AsteroidId(4);

        let view = world.view(0);

        assert_eq!(
            view.terrain_of(at).map(|asteroid| asteroid.asteroid),
            Some(at)
        );
        assert_eq!(view.asteroid_body(at), Some(world.state.asteroid_body(at)));
        assert_eq!(view.terrain_of(AsteroidId(99)), None);
        assert_eq!(view.asteroid_body(AsteroidId(99)), None);
    }

    #[test]
    fn a_seats_income_is_what_its_extractors_pulled_whether_the_store_kept_it_or_not() {
        let mut world = world();
        world.fix(0, METALS_EXTRACTOR, ASTEROID);
        world.fix(0, VOLATILES_EXTRACTOR, ASTEROID);
        let full = world.view(0).stockpile.stock();
        world.run(SECOND);

        let brimming = world.view(0);

        assert_eq!(brimming.stockpile.stock(), full, "nothing was kept");
        assert!(
            close(brimming.income, Materials::new(2.0, 1.0, 0.0)),
            "each extractor pulls its own material, gross"
        );
        assert_eq!(world.view(1).income, Materials::ZERO, "its own extractors");

        world.fix(0, STORAGE, ASTEROID);
        world.run(SECOND);
        let before = world.view(0).stockpile.stock();
        world.run(SECOND);

        let growing = world.view(0);

        assert!(
            growing.stockpile.stock().total() < growing.stockpile.capacity().total(),
            "the store left room to grow"
        );
        assert!(
            close(growing.income, growing.stockpile.stock() - before),
            "all of it kept"
        );
    }

    #[test]
    fn a_seats_spend_over_the_last_second_is_what_its_frames_drained() {
        let mut world = world();
        world.fix(0, CONSTRUCTOR, ASTEROID);
        let before = world.view(0).stockpile.stock();
        world.tick(&[Issued::want(0, ASTEROID, FRIGATE, 1)]);
        world.run(SECOND - 1);

        let view = world.view(0);

        assert!(view.spend.total() > 0.0, "the frame drained something");
        assert!(close(view.spend, before - view.stockpile.stock()));
    }

    #[test]
    fn a_asteroids_pull_over_the_last_second_sums_every_seat_and_stays_inside_its_cap() {
        let mut world = world();
        for seat in [0, 1] {
            world.fix(seat, METALS_EXTRACTOR, ASTEROID);
            world.fix(seat, VOLATILES_EXTRACTOR, ASTEROID);
        }
        world.run(SECOND);

        let pull = pulled(&world, 1);

        assert!(close(pull, Materials::new(4.0, 1.0, 0.0)), "{pull:?}");
        assert_eq!(
            pull.min(world.state[ASTEROID].caps()),
            pull,
            "inside the caps"
        );
        assert_eq!(pull, pulled(&world, 0), "a pull is visible to every seat");
    }

    #[test]
    fn the_view_reports_the_last_completed_second_and_holds_it_until_the_next_closes() {
        let mut world = world();
        world.fix(0, METALS_EXTRACTOR, ASTEROID);

        assert_eq!(world.view(0).income, Materials::ZERO, "none completed");
        world.run(SECOND - 1);
        assert_eq!(world.view(0).income, Materials::ZERO, "still filling");
        world.run(1);
        let first = world.view(0).income;

        world.run(SECOND - 1);

        assert!(first.total() > 0.0);
        assert_eq!(world.view(0).income, first, "it stands to the next");
        assert_eq!(pulled(&world, 0), first, "the asteroid's pull with it");
    }

    #[test]
    fn an_exchange_names_the_asteroid_the_shooter_fired_from_and_the_target_was_hit_at() {
        let mut world = world();
        let shooter = world.hold(0, FRIGATE, ASTEROID, 0.0);
        let target = world.fix(1, STORAGE, ASTEROID);
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
                    asteroid: ASTEROID,
                    seat: SeatId(0),
                    fired: true,
                    landed: false,
                },
                Exchange {
                    asteroid: ASTEROID,
                    seat: SeatId(1),
                    fired: false,
                    landed: true,
                },
            ]
        );
    }
}
