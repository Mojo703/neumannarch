use std::collections::BTreeMap;

use super::State;
use super::schedule::Flight;
use super::seat::Seat;
use crate::belt::Belt;
use crate::ids::{EntityId, RockId, RowId, SeatId, TeamId};
use crate::materials::{Material, Materials, Stockpile};
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::post::Post;
use crate::state::standings::Standings;
use crate::step::fire::{Exchange, Shots};
use crate::time::Tick;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Present {
    pub id: EntityId,
    pub row: RowId,
    pub seat: SeatId,
    pub body: Body,
    pub hp: f64,
    pub home: RockId,
    pub from: Option<RockId>,
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
pub struct Wanted {
    pub row: RowId,
    pub want: u32,
    pub present: u32,
    pub transit: u32,
    pub frames: Vec<Building>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub rock: RockId,
    pub rows: Vec<Wanted>,
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
}

fn compositions(state: &State, seat: SeatId) -> Vec<Composition> {
    state
        .posts()
        .filter(|(post, _)| post.seat == seat)
        .map(|(post, wants)| Composition {
            rock: post.rock,
            rows: wants
                .iter()
                .map(|(row, want)| wanted(state, post, row, want))
                .collect(),
        })
        .collect()
}

fn wanted(state: &State, post: Post, row: RowId, want: u32) -> Wanted {
    let held = state.holdings(post, row);
    Wanted {
        row,
        want,
        present: held.present,
        transit: held.transit,
        frames: state
            .frames_at(post)
            .filter(|frame| frame.row() == row)
            .map(|frame| Building {
                progress: frame.fraction(state[row].cost.total()),
                starved_of: frame.starved_material(state.tick()),
            })
            .collect(),
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
            from: entity
                .flight()
                .filter(|_| entity.is_flying(state.tick()))
                .map(Flight::source),
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
    use crate::roster::{FRIGATE, SHIPYARD, STORAGE};
    use crate::state::{Issued, Send};

    const ROCK: RockId = RockId(0);

    fn world() -> World {
        World::started(&[TeamId(0), TeamId(1)])
    }

    #[test]
    fn a_view_holds_the_seats_own_posts_and_no_others() {
        let mut world = world();
        world.fix(0, SHIPYARD, ROCK);
        world.tick(&[
            Issued::want(0, ROCK, FRIGATE, 2),
            Issued::want(1, RockId(5), FRIGATE, 3),
        ]);

        let view = world.view(0);

        assert_eq!(view.compositions.len(), 1);
        assert_eq!(view.compositions[0].rock, ROCK);
        assert_eq!(view.compositions[0].rows[0].row, FRIGATE);
        assert_eq!(view.compositions[0].rows[0].want, 2);
        assert_eq!(view.compositions[0].rows[0].present, 0);
        assert_eq!(view.reserve[&SHIPYARD], 1);
        assert_eq!(view.terrain.len(), world.state.rocks().len());
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
        assert_eq!(theirs.from, None, "it is not flying");
    }

    #[test]
    fn a_flying_unit_names_the_rock_it_left_and_the_one_it_flies_to() {
        let mut world = world();
        let mine = world.hold(0, FRIGATE, ROCK, 0.0);
        let theirs = world.hold(1, FRIGATE, ROCK, 0.0);
        world.tick(&[
            Issued::numbered(0, 0, ROCK, FRIGATE, 0),
            Issued::numbered(0, 1, RockId(1), FRIGATE, 1),
            Issued::numbered(1, 0, ROCK, FRIGATE, 0),
            Issued::numbered(1, 1, RockId(1), FRIGATE, 1),
        ]);
        world.run(Send::FORMING_TICKS + 1);

        for unit in [mine, theirs] {
            let flier = world.present(0, unit).expect("both fliers are in the view");
            assert_eq!(flier.home, RockId(1), "a send names where to");
            assert_eq!(flier.from, Some(ROCK), "a send names where from");
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
        let view = world().view(9);
        assert!(view.compositions.is_empty());
        assert!(view.reserve.is_empty());
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
