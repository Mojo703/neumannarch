//! The fogged view: everything one seat may know, and nothing else.

use std::collections::BTreeMap;

use super::State;
use super::sight::Sight;
use crate::ids::{EntityId, RockId, RowId, SeatId, TeamId};
use crate::materials::{Materials, Stockpile};
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::place::{Place, Post};
use crate::state::standings::Standings;
use crate::step::fire::Shots;
use crate::time::Tick;

/// The mass, on the roster's scale, below which radar reports a contact as
/// light. A hypothesis the display confirms or kills.
const LIGHT_MASS: f64 = 30.0;

/// The mass below which radar reports a contact as medium, and at or above
/// which it reports heavy. A hypothesis.
const HEAVY_MASS: f64 = 100.0;

/// How much a radar contact weighs, as much as radar can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassClass {
    Light,
    Medium,
    Heavy,
}

/// One entity the seat sees exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub entity: EntityId,
    pub seat: SeatId,
    pub row: RowId,
    pub body: Body,
    pub hp: f64,
    /// True while it is flying a send, when it neither shoots nor is shot.
    pub flying: bool,
    /// The place it belongs to, which is where a flying one is going.
    /// `None` for a flying entity of another team, whose destination sight
    /// does not give; a holding entity's band its exact position does.
    pub home: Option<Place>,
}

/// One radar contact: inside a sensor's radar range but not its sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    pub body: Body,
    pub mass: MassClass,
}

/// The shots at one place, by or on one seat, in one tick. A fight arc
/// starts and refreshes on these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exchange {
    pub place: Place,
    pub seat: SeatId,
    /// A weapon of the seat's fired from the place.
    pub fired: bool,
    /// A shot landed on one of the seat's at the place.
    pub landed: bool,
}

/// One rock, which every seat always knows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terrain {
    pub rock: RockId,
    pub orbit: Orbit,
    /// The most of each material extractable per second.
    pub caps: Materials,
    /// Visual radius in meters.
    pub radius: f64,
}

/// One row of one of the seat's own compositions.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub row: RowId,
    pub want: u32,
    /// Units at the place, not counting those flying in.
    pub present: u32,
    /// Units flying in to the place.
    pub flying: u32,
    /// Each open frame's work done, as a fraction of its cost.
    pub frames: Vec<f64>,
}

/// One of the seat's own compositions.
#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub place: Place,
    pub rows: Vec<Wanted>,
}

/// What one seat may know at one tick. Built once per tick for a display
/// or an agent; nothing in it refers to anything the seat cannot see.
///
/// The roster is match-constant and travels with the initial state, so it
/// is not repeated here: a client holds it from the session it plays.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub seat: SeatId,
    pub tick: Tick,
    pub clock: Tick,
    /// The central mass's gravitational parameter, in m³/s²: what an
    /// orbit of the terrain is read at a tick with.
    pub gravity: Gravity,
    pub stockpile: Stockpile,
    pub reserve: BTreeMap<RowId, u32>,
    pub compositions: Vec<Composition>,
    pub seen: Vec<Seen>,
    pub blips: Vec<Blip>,
    /// The tick's shots at the places the seat sees, in place then seat
    /// order.
    pub exchanges: Vec<Exchange>,
    /// Every rock, in rock id order, so `terrain[id]` is the rock at `id`.
    pub terrain: Vec<Terrain>,
    /// The score, `Some` only once the clock has run out: DESIGN.md's Fog
    /// section reveals it at the clock and never before.
    pub standings: Option<Standings>,
}

impl View {
    /// What `seat` may know of `state`, with `shots` the last step
    /// resolved. A seat the match does not have sees only the terrain.
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
            blips: blips(state, seat, &sight, &sweep),
            exchanges: exchanges(state, &sight, shots),
            terrain: terrain(state),
            standings: Some(state.standings()).filter(Standings::over),
        }
    }

    /// The rock at `id`, or `None` when the map lacks it.
    pub fn terrain_of(&self, id: RockId) -> Option<&Terrain> {
        self.terrain.get(id.0 as usize)
    }

    /// Where the rock at `id` is this tick, or `None` when the map lacks
    /// it.
    pub fn rock_body(&self, id: RockId) -> Option<Body> {
        self.terrain_of(id)
            .map(|terrain| terrain.orbit.at(self.tick, self.gravity))
    }

    /// The orbit `place`'s anchor follows, or `None` when the map lacks its
    /// rock. Every seat at a place shares its anchor.
    pub fn anchor(&self, place: Place) -> Option<Orbit> {
        self.terrain_of(place.rock)
            .map(|terrain| terrain.orbit.shifted(place.band.amplitude()))
    }
}

impl MassClass {
    /// The class radar reports for `mass`, on the roster's scale.
    fn of(mass: f64) -> MassClass {
        if mass < LIGHT_MASS {
            MassClass::Light
        } else if mass < HEAVY_MASS {
            MassClass::Medium
        } else {
            MassClass::Heavy
        }
    }
}

/// The seat's own compositions, in place order: the wants, what is there,
/// what is flying in, and the frames open.
fn compositions(state: &State, seat: SeatId) -> Vec<Composition> {
    state
        .posts()
        .filter(|(post, _)| post.seat == seat)
        .map(|(post, wants)| Composition {
            place: post.place,
            rows: wants
                .iter()
                .map(|(row, want)| wanted(state, post, row, want))
                .collect(),
        })
        .collect()
}

/// One wanted row of one composition.
fn wanted(state: &State, post: Post, row: RowId, want: u32) -> Wanted {
    let mine = || {
        state
            .entities_at(post.place)
            .filter(move |entity| entity.seat() == post.seat && entity.row() == row)
    };
    Wanted {
        row,
        want,
        present: mine().filter(|entity| !entity.is_flying()).count() as u32,
        flying: mine().filter(|entity| entity.is_flying()).count() as u32,
        frames: state
            .frames_at(post)
            .filter(|frame| frame.row() == row)
            .map(|frame| {
                let cost = state[row].cost.total();
                if cost > 0.0 {
                    (frame.progress() / cost).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            })
            .collect(),
    }
}

/// Every entity the seat's team sees, in id order. `team` is the viewing
/// seat's, and the match may not have one.
fn seen(state: &State, sight: &Sight, team: Option<TeamId>) -> Vec<Seen> {
    sight
        .iter()
        .filter_map(|id| state.entity(id))
        .map(|entity| Seen {
            entity: entity.id(),
            seat: entity.seat(),
            row: entity.row(),
            body: state.body_of(entity),
            hp: entity.hp(),
            flying: entity.is_flying(),
            home: (!entity.is_flying() || team == Some(state[entity.seat()].team()))
                .then(|| entity.home()),
        })
        .collect()
}

/// The tick's shots at the places the seat sees: one entry per place and
/// seat a seen entity fired from or was hit at.
fn exchanges(state: &State, sight: &Sight, shots: &Shots) -> Vec<Exchange> {
    let mut found: BTreeMap<(Place, SeatId), (bool, bool)> = BTreeMap::new();
    let mut note = |id: EntityId, landed: bool| {
        if !sight.sees(id) {
            return;
        }
        let Some(entity) = state.entity(id) else {
            return;
        };
        let at = found
            .entry((entity.home(), entity.seat()))
            .or_insert((false, false));
        match landed {
            true => at.1 = true,
            false => at.0 = true,
        }
    };
    for hit in &shots.hits {
        note(hit.shooter, false);
        note(hit.target, true);
    }
    found
        .into_iter()
        .map(|((place, seat), (fired, landed))| Exchange {
            place,
            seat,
            fired,
            landed,
        })
        .collect()
}

/// Every entity inside the radar range of one of the seat's team's
/// entities but outside its sight, in id order.
fn blips(
    state: &State,
    seat: SeatId,
    sight: &Sight,
    sweep: &crate::state::sweep::Sweep,
) -> Vec<Blip> {
    let Some(team) = state.seat(seat).map(|seat| seat.team()) else {
        return Vec::new();
    };
    let mut found: Vec<EntityId> = state
        .entities()
        .filter(|sensor| state[sensor.seat()].team() == team)
        .flat_map(|sensor| sweep.within(state.body_of(sensor).pos, state[sensor.row()].radar.0))
        .filter(|id| !sight.sees(*id))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
        .into_iter()
        .filter_map(|id| state.entity(id))
        .map(|entity| Blip {
            body: state.body_of(entity),
            mass: MassClass::of(state[entity.row()].mass.0),
        })
        .collect()
}

/// Every rock with its orbit and caps, which no fog hides.
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
    use crate::place::Band;
    use crate::roster::{FRIGATE, SHIPYARD, STORAGE};
    use crate::setup::Setup;
    use crate::state::{Batch, Command, Issued, Motion};
    use crate::step::fire::Fire;

    /// The view of `seat`, with no shots this tick.
    fn quiet(state: &State, seat: SeatId) -> View {
        View::of(state, seat, &Shots::default())
    }

    /// A match of two teams over the shipped belt, ending at `clock`.
    fn started(clock: Tick) -> State {
        let setup = Setup::new(vec![TeamId(0), TeamId(1)], 0, clock).expect("two seats");
        State::start(&setup)
    }

    /// A match of two teams whose clock no test reaches.
    fn state() -> State {
        started(Tick(1_000))
    }

    /// One tick with `issued`, which must all be accepted.
    fn tick(state: &State, issued: &[Issued]) -> State {
        let mut batch = Batch::new();
        for issued in issued {
            assert_eq!(batch.insert(*issued), Ok(()));
        }
        let (next, outcome) = state.step(&batch);
        assert_eq!(outcome.rejected, Vec::new(), "the commands were rejected");
        next
    }

    /// A want of `count` of `row` at `place`, as `seat`'s `seq`th command.
    fn want(seat: u8, seq: u32, place: Place, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { place, row, count },
        }
    }

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    #[test]
    fn a_view_holds_the_seats_own_posts_and_no_others() {
        let mut state = state();
        state.spawn(SeatId(0), SHIPYARD, inner(0), Motion::Fixed);
        let state = tick(
            &state,
            &[
                want(0, 0, inner(0), FRIGATE, 2),
                want(1, 0, inner(5), FRIGATE, 3),
            ],
        );

        let view = quiet(&state, SeatId(0));

        assert_eq!(view.compositions.len(), 1);
        assert_eq!(view.compositions[0].place, inner(0));
        assert_eq!(view.compositions[0].rows[0].row, FRIGATE);
        assert_eq!(view.compositions[0].rows[0].want, 2);
        assert_eq!(view.compositions[0].rows[0].present, 0);
        assert_eq!(view.reserve[&SHIPYARD], 1);
        assert_eq!(view.terrain.len(), state.rocks().len());
    }

    #[test]
    fn a_view_holds_what_the_seat_sees_and_leaves_out_what_it_does_not() {
        let mut state = state();
        let mine = state.spawn(SeatId(0), SHIPYARD, inner(0), Motion::Fixed);
        let near = state.spawn(SeatId(1), STORAGE, inner(0), Motion::Fixed);
        let far = state.spawn(SeatId(1), STORAGE, inner(9), Motion::Fixed);

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
        // A scout sees two meters and reaches fifty by radar.
        let watcher = state.spawn(SeatId(0), crate::roster::SCOUT, inner(0), Motion::Fixed);
        let body = state.body_of(&state[watcher]);
        let heavy = Motion::Free {
            body: Body::new(body.pos + crate::Vec3::new(30.0, 0.0, 0.0), body.vel),
            flight: None,
        };
        state.spawn(SeatId(1), FRIGATE, inner(0), heavy);

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
    fn a_view_reads_a_rock_and_an_anchor_by_id() {
        let state = state();
        let place = Place {
            rock: RockId(4),
            band: Band::Outer,
        };

        let view = quiet(&state, SeatId(0));

        assert_eq!(
            view.terrain_of(place.rock).map(|rock| rock.rock),
            Some(place.rock)
        );
        assert_eq!(
            view.rock_body(place.rock),
            Some(state.rock_body(place.rock))
        );
        assert_eq!(view.anchor(place), Some(state.anchor(place)));
        assert_eq!(view.terrain_of(RockId(99)), None);
        assert_eq!(view.rock_body(RockId(99)), None);
        assert_eq!(
            view.anchor(Place {
                rock: RockId(99),
                ..place
            }),
            None
        );
    }

    #[test]
    fn a_flying_units_home_is_its_destination_and_an_enemys_is_hidden() {
        let mut state = state();
        state.spawn(SeatId(0), SHIPYARD, inner(0), Motion::Fixed);
        // A frigate is in neither seat's reserve, so the send is the only
        // way rock one's want is filled.
        let mine = state.spawn(SeatId(0), FRIGATE, inner(0), holding(&state, inner(0)));
        let theirs = state.spawn(SeatId(1), FRIGATE, inner(0), holding(&state, inner(0)));
        let state = sent(state, &[(0, mine), (1, theirs)]);

        let view = quiet(&state, SeatId(0));
        let of = |id| view.seen.iter().find(|seen| seen.entity == id).copied();

        let mine = of(mine).expect("a seat sees its own unit");
        assert!(mine.flying);
        assert_eq!(mine.home, Some(inner(1)), "its own send names where to");
        let theirs = of(theirs).expect("the enemy is at the same rock");
        assert!(theirs.flying);
        assert_eq!(
            theirs.home, None,
            "sight does not give a send's destination"
        );
    }

    #[test]
    fn a_holding_entity_names_its_place_whoever_owns_it() {
        let mut state = state();
        let mine = state.spawn(SeatId(0), SHIPYARD, inner(0), Motion::Fixed);
        let theirs = state.spawn(SeatId(1), STORAGE, inner(0), Motion::Fixed);

        let view = quiet(&state, SeatId(0));

        for id in [mine, theirs] {
            let seen = view
                .seen
                .iter()
                .find(|seen| seen.entity == id)
                .expect("both are at the seat's own rock");
            assert_eq!(seen.home, Some(inner(0)));
        }
    }

    #[test]
    fn an_exchange_names_the_place_the_shooter_fired_from_and_the_target_was_hit_at() {
        let mut state = state();
        let shooter = state.spawn(SeatId(0), FRIGATE, inner(0), holding(&state, inner(0)));
        let target = state.spawn(SeatId(1), STORAGE, inner(0), Motion::Fixed);
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
                    place: inner(0),
                    seat: SeatId(0),
                    fired: true,
                    landed: false,
                },
                Exchange {
                    place: inner(0),
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
        state.spawn(SeatId(0), FRIGATE, inner(0), holding(&state, inner(0)));
        state.spawn(SeatId(1), STORAGE, inner(0), Motion::Fixed);
        state.spawn(SeatId(2), STORAGE, inner(9), Motion::Fixed);
        let shots = Fire::of(&state).run();
        assert!(!shots.hits.is_empty(), "the fight happened");

        let view = View::of(&state, SeatId(2), &shots);

        assert!(view.exchanges.is_empty());
    }

    /// Free at `place`'s anchor, as a unit that has arrived is.
    fn holding(state: &State, place: Place) -> Motion {
        Motion::Free {
            body: state.anchor(place).at(state.tick(), state.gravity()),
            flight: None,
        }
    }

    /// One tick that re-homes each `(seat, entity)` from rock zero to rock
    /// one, which is the send that makes them fly.
    fn sent(state: State, units: &[(u8, EntityId)]) -> State {
        let issued: Vec<Issued> = units
            .iter()
            .flat_map(|(seat, entity)| {
                let row = state[*entity].row();
                [
                    want(*seat, 0, inner(0), row, 0),
                    want(*seat, 1, inner(1), row, 1),
                ]
            })
            .collect();
        tick(&state, &issued)
    }
}
