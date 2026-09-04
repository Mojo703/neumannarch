use std::collections::BTreeMap;

use super::State;
use super::radar::Radar;
use super::schedule::Flight;
use super::sight::Sight;
use crate::belt::Belt;
use crate::ids::{EntityId, RockId, RowId, SeatId, TeamId};
use crate::materials::{Material, Materials, Stockpile};
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::post::Post;
use crate::roster::MassClass;
use crate::state::standings::Standings;
use crate::step::fire::{Exchange, Shots};
use crate::time::Tick;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub entity: EntityId,
    pub seat: SeatId,
    pub row: RowId,
    pub body: Body,
    pub hp: f64,
    pub flying: bool,
    pub home: Option<RockId>,
    pub from: Option<RockId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    pub body: Body,
    pub mass: MassClass,
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
    pub seen: Vec<Seen>,
    pub blips: Vec<Blip>,
    pub exchanges: Vec<Exchange>,
    pub terrain: Vec<Terrain>,
    pub zone: f64,
    pub standings: Option<Standings>,
}

impl View {
    pub fn of(state: &State, seat: SeatId, shots: &Shots) -> View {
        let sweep = state.sweep();
        let sight = Sight::of(state, seat, &sweep);
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
            seen: seen(state, &sight, state.seat(seat).map(|seat| seat.team())),
            blips: blips(state, &Radar::beyond(state, seat, &sweep, &sight)),
            exchanges: shots.exchanges(state, &sight),
            terrain: terrain(state),
            zone: Belt::ZONE_RADIUS_METERS,
            standings: Some(state.standings()).filter(Standings::over),
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
    let held = state.holding(post, row);
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

fn seen(state: &State, sight: &Sight, team: Option<TeamId>) -> Vec<Seen> {
    sight
        .iter()
        .filter_map(|id| state.entity(id))
        .map(|entity| {
            let own = team == Some(state[entity.seat()].team());
            Seen {
                entity: entity.id(),
                seat: entity.seat(),
                row: entity.row(),
                body: state.body_of(entity),
                hp: entity.hp(),
                flying: entity.is_flying(state.tick()),
                home: (!entity.is_flying(state.tick()) || own).then(|| entity.home()),
                from: entity.flight().filter(|_| own).map(Flight::source),
            }
        })
        .collect()
}

fn blips(state: &State, radar: &Radar) -> Vec<Blip> {
    radar
        .iter()
        .filter_map(|contact| state.entity(contact))
        .map(|entity| Blip {
            body: state.body_of(entity),
            mass: state[entity.row()].mass_class(),
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
    use crate::ids::TeamId;
    use crate::roster::{FRIGATE, SHIPYARD, STORAGE};
    use crate::setup::Setup;
    use crate::state::{Batch, Command, Issued, Motion, Send};
    use crate::step::fire::Fire;

    fn quiet(state: &State, seat: SeatId) -> View {
        View::of(state, seat, &Shots::default())
    }

    fn started(clock: Tick) -> State {
        let setup = Setup::new(vec![TeamId(0), TeamId(1)], 0, clock).expect("two seats");
        State::start(&setup)
    }

    fn state() -> State {
        started(Tick(1_000))
    }

    fn tick(state: &State, issued: &[Issued]) -> State {
        let mut batch = Batch::new();
        for issued in issued {
            assert_eq!(batch.insert(*issued), Ok(()));
        }
        let (next, outcome) = state.step(&batch);
        assert_eq!(outcome.rejected, Vec::new(), "the commands were rejected");
        next
    }

    fn want(seat: u8, seq: u32, rock: RockId, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { rock, row, count },
        }
    }

    fn rock(at: u32) -> RockId {
        RockId(at)
    }

    #[test]
    fn a_view_holds_the_seats_own_posts_and_no_others() {
        let mut state = state();
        state.spawn(SeatId(0), SHIPYARD, rock(0), Motion::Fixed);
        let state = tick(
            &state,
            &[
                want(0, 0, rock(0), FRIGATE, 2),
                want(1, 0, rock(5), FRIGATE, 3),
            ],
        );

        let view = quiet(&state, SeatId(0));

        assert_eq!(view.compositions.len(), 1);
        assert_eq!(view.compositions[0].rock, rock(0));
        assert_eq!(view.compositions[0].rows[0].row, FRIGATE);
        assert_eq!(view.compositions[0].rows[0].want, 2);
        assert_eq!(view.compositions[0].rows[0].present, 0);
        assert_eq!(view.reserve[&SHIPYARD], 1);
        assert_eq!(view.terrain.len(), state.rocks().len());
    }

    #[test]
    fn a_view_holds_what_the_seat_sees_and_leaves_out_what_it_does_not() {
        let mut state = state();
        let mine = state.spawn(SeatId(0), SHIPYARD, rock(0), Motion::Fixed);
        let near = state.spawn(SeatId(1), STORAGE, rock(0), Motion::Fixed);
        let far = state.spawn(SeatId(1), STORAGE, rock(9), Motion::Fixed);

        let view = quiet(&state, SeatId(0));
        let seen: Vec<EntityId> = view.seen.iter().map(|seen| seen.entity).collect();

        assert!(seen.contains(&mine));
        assert!(seen.contains(&near), "an enemy at its own rock is seen");
        assert!(!seen.contains(&far), "a rock away is beyond every sensor");
        assert!(
            view.blips.is_empty(),
            "nothing is in radar but out of sight"
        );
        let hp = view
            .seen
            .iter()
            .find(|seen| seen.entity == near)
            .expect("the enemy")
            .hp;
        assert_eq!(hp, state[STORAGE].hp.0);
    }

    #[test]
    fn a_radar_contact_carries_a_mass_class_and_no_row() {
        let mut state = state();

        let watcher = state.spawn(SeatId(0), crate::roster::SCOUT, rock(0), Motion::Fixed);
        let body = state.body_of(&state[watcher]);
        let heavy = Motion::Free {
            body: Body::new(body.pos + crate::Vec3::new(30.0, 0.0, 0.0), body.vel),
            flight: None,
        };
        state.spawn(SeatId(1), FRIGATE, rock(0), heavy);

        let view = quiet(&state, SeatId(0));

        assert_eq!(view.seen.len(), 1, "only the scout itself is seen");
        assert_eq!(view.blips.len(), 1);
        assert_eq!(view.blips[0].mass, MassClass::Medium);
    }

    #[test]
    fn the_standings_are_hidden_until_the_clock_runs_out() {
        let state = state();
        assert_eq!(quiet(&state, SeatId(0)).standings, None);

        let over = started(Tick::ZERO);

        let view = quiet(&over, SeatId(0));

        assert_eq!(view.standings.as_ref().map(Standings::over), Some(true));
        assert_eq!(
            view.standings.expect("the clock has run out").teams().len(),
            2
        );
    }

    #[test]
    fn a_seat_the_match_lacks_sees_only_the_terrain() {
        let view = quiet(&state(), SeatId(9));
        assert!(view.seen.is_empty());
        assert!(view.blips.is_empty());
        assert!(view.compositions.is_empty());
        assert!(view.reserve.is_empty());
        assert_eq!(view.terrain.len(), 21);
    }

    #[test]
    fn a_view_reads_its_terrain_at_the_gravity_it_carries() {
        let state = state();

        let view = quiet(&state, SeatId(0));

        assert_eq!(view.gravity, state.gravity());
        let rock = view.terrain[3];
        assert_eq!(
            rock.orbit.at(view.tick, view.gravity),
            state.rock_body(rock.rock)
        );
    }

    #[test]
    fn a_view_reads_a_rock_by_id() {
        let state = state();
        let at = rock(4);

        let view = quiet(&state, SeatId(0));

        assert_eq!(view.terrain_of(at).map(|rock| rock.rock), Some(at));
        assert_eq!(view.rock_body(at), Some(state.rock_body(at)));
        assert_eq!(view.terrain_of(RockId(99)), None);
        assert_eq!(view.rock_body(RockId(99)), None);
    }

    #[test]
    fn a_flying_units_home_is_its_destination_and_an_enemys_is_hidden() {
        let mut state = state();
        state.spawn(SeatId(0), SHIPYARD, rock(0), Motion::Fixed);

        let mine = state.spawn(SeatId(0), FRIGATE, rock(0), holding(&state, rock(0)));
        let theirs = state.spawn(SeatId(1), FRIGATE, rock(0), holding(&state, rock(0)));
        let state = sent(state, &[(0, mine), (1, theirs)]);

        let view = quiet(&state, SeatId(0));
        let of = |id| view.seen.iter().find(|seen| seen.entity == id).copied();

        let mine = of(mine).expect("a seat sees its own unit");
        assert!(mine.flying);
        assert_eq!(mine.home, Some(rock(1)), "its own send names where to");
        let theirs = of(theirs).expect("the enemy is at the same rock");
        assert!(theirs.flying);
        assert_eq!(
            theirs.home, None,
            "sight does not give a send's destination"
        );
    }

    #[test]
    fn a_holding_entity_names_its_rock_whoever_owns_it() {
        let mut state = state();
        let mine = state.spawn(SeatId(0), SHIPYARD, rock(0), Motion::Fixed);
        let theirs = state.spawn(SeatId(1), STORAGE, rock(0), Motion::Fixed);

        let view = quiet(&state, SeatId(0));

        for id in [mine, theirs] {
            let seen = view
                .seen
                .iter()
                .find(|seen| seen.entity == id)
                .expect("both are at the seat's own rock");
            assert_eq!(seen.home, Some(rock(0)));
        }
    }

    #[test]
    fn an_exchange_names_the_rock_the_shooter_fired_from_and_the_target_was_hit_at() {
        let mut state = state();
        let shooter = state.spawn(SeatId(0), FRIGATE, rock(0), holding(&state, rock(0)));
        let target = state.spawn(SeatId(1), STORAGE, rock(0), Motion::Fixed);
        let shots = Fire::of(&state).run();
        assert!(
            shots
                .hits
                .iter()
                .any(|hit| hit.shooter == shooter && hit.target == target),
            "the frigate fired on the storage"
        );

        let view = View::of(&state, SeatId(0), &shots);

        assert_eq!(
            view.exchanges,
            vec![
                Exchange {
                    rock: rock(0),
                    seat: SeatId(0),
                    fired: true,
                    landed: false,
                },
                Exchange {
                    rock: rock(0),
                    seat: SeatId(1),
                    fired: false,
                    landed: true,
                },
            ]
        );
    }

    #[test]
    fn a_seat_that_sees_neither_side_of_a_fight_is_told_nothing_of_it() {
        let setup =
            Setup::new(vec![TeamId(0), TeamId(1), TeamId(2)], 0, Tick(1_000)).expect("three seats");
        let mut state = State::start(&setup);
        state.spawn(SeatId(0), FRIGATE, rock(0), holding(&state, rock(0)));
        state.spawn(SeatId(1), STORAGE, rock(0), Motion::Fixed);
        state.spawn(SeatId(2), STORAGE, rock(9), Motion::Fixed);
        let shots = Fire::of(&state).run();
        assert!(!shots.hits.is_empty(), "the fight happened");

        let view = View::of(&state, SeatId(2), &shots);

        assert!(view.exchanges.is_empty());
    }

    fn holding(state: &State, at: RockId) -> Motion {
        Motion::Free {
            body: state.rock_body(at),
            flight: None,
        }
    }

    fn sent(state: State, units: &[(u8, EntityId)]) -> State {
        let issued: Vec<Issued> = units
            .iter()
            .flat_map(|(seat, entity)| {
                let row = state[*entity].row();
                [
                    want(*seat, 0, rock(0), row, 0),
                    want(*seat, 1, rock(1), row, 1),
                ]
            })
            .collect();
        let mut state = tick(&state, &issued);
        for _ in 0..=Send::FORMING_TICKS {
            state = tick(&state, &[]);
        }
        state
    }
}
