//! The fire rule: every ready weapon shoots in ready order at the enemy
//! that threatens its owner most, against the tick's opening state plus the
//! damage already assigned.

use std::collections::BTreeMap;

use crate::ids::{EntityId, SeatId};
use crate::place::Place;
use crate::roster::Weapon;
use crate::state::sweep::Sweep;
use crate::state::{Entity, Ready, Sight, State};
use crate::time::Moment;

/// One shot that landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub shooter: EntityId,
    /// The index of the weapon in the shooter's row.
    pub weapon: u8,
    pub target: EntityId,
    /// Hit points, after falloff and the target's plating.
    pub damage: f64,
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

/// Every shot of one tick, in the order they resolved, and when the
/// weapons that fired are next ready. A weapon with nothing to shoot at
/// keeps the moment it had, so it fires the instant a target arrives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shots {
    pub hits: Vec<Hit>,
    pub ready: Vec<Ready>,
}

/// The fire phase over one tick's snapshot.
pub struct Fire<'a> {
    state: &'a State,
    sweep: Sweep,
    sights: BTreeMap<SeatId, Sight>,
}

impl<'a> Fire<'a> {
    /// Reads `state`, building the sweep and one sight per seat.
    pub fn of(state: &'a State) -> Fire<'a> {
        let sweep = state.sweep();
        let sights = (0..state.seats().len())
            .map(|seat| SeatId(seat as u8))
            .map(|seat| (seat, Sight::of(state, seat, &sweep)))
            .collect();
        Fire {
            state,
            sweep,
            sights,
        }
    }

    /// The tick's shots, resolved in ready order then shooter id then
    /// weapon.
    pub fn run(self) -> Shots {
        let mut shots = Shots::default();
        let mut assigned: BTreeMap<EntityId, f64> = BTreeMap::new();
        for ready in self.ready() {
            let Some(shooter) = self.state.entity(ready.entity()) else {
                continue;
            };
            let Some(Weapon::Damage {
                range,
                rate,
                damage,
                falloff,
            }) = self.state[shooter.row()]
                .weapons
                .get(usize::from(ready.weapon()))
                .copied()
            else {
                continue;
            };
            let Some((target, distance)) = self.target(shooter, range.0, &assigned) else {
                continue;
            };
            let plating = self.state[self.state[target].row()].plating.0;
            let dealt = (damage.0 * (1.0 - falloff.0 * distance / range.0) - plating).max(0.0);
            *assigned.entry(target).or_default() += dealt;
            shots.hits.push(Hit {
                shooter: shooter.id(),
                weapon: ready.weapon(),
                target,
                damage: dealt,
            });
            shots.ready.push(Ready::new(
                ready.entity(),
                ready.weapon(),
                self.next_ready(ready.at(), rate.0),
            ));
        }
        shots
    }

    /// Every weapon that may fire this tick, in ready order then shooter id
    /// then weapon. A weapon of a flying entity holds its fire.
    fn ready(&self) -> Vec<Ready> {
        let now = Moment::at(self.state.tick().next());
        let mut ready: Vec<Ready> = self
            .state
            .ready()
            .iter()
            .filter(|ready| ready.at() < now)
            .filter(|ready| {
                self.state
                    .entity(ready.entity())
                    .is_some_and(|entity| !entity.is_flying())
            })
            .cloned()
            .collect();
        ready.sort_by(|a, b| {
            a.at()
                .cmp(&b.at())
                .then(a.entity().cmp(&b.entity()))
                .then(a.weapon().cmp(&b.weapon()))
        });
        ready
    }

    /// When a weapon of `rate` shots per second that fired at `at` is next
    /// ready: one interval on, never before this tick.
    fn next_ready(&self, at: Moment, rate: f64) -> Moment {
        let interval = if rate > 0.0 { 1.0 / rate } else { f64::MAX };
        at.after(interval).max(Moment::at(self.state.tick()))
    }

    /// The enemy `shooter` fires at and how far off it is, in meters: the
    /// highest damage per second through the shooter's plating per point of
    /// the target's hit points, ties by nearest then lowest id. Targets
    /// whose assigned damage is already lethal are skipped.
    fn target(
        &self,
        shooter: &Entity,
        range: f64,
        assigned: &BTreeMap<EntityId, f64>,
    ) -> Option<(EntityId, f64)> {
        let sight = self.sights.get(&shooter.seat())?;
        let team = self.state[shooter.seat()].team();
        let from = self.state.body_of(shooter).pos;
        let plating = self.state[shooter.row()].plating.0;
        self.sweep
            .within(from, range)
            .filter_map(|id| self.state.entity(id))
            .filter(|target| {
                self.state[target.seat()].team() != team
                    && !target.is_flying()
                    && target.home().rock == shooter.home().rock
                    && sight.sees(target.id())
                    && target.hp() > assigned.get(&target.id()).copied().unwrap_or(0.0)
            })
            .map(|target| {
                let threat = self.state[target.row()].dps_through(plating) / target.hp();
                let distance = self.state.body_of(target).pos.distance(from);
                (target.id(), threat, distance)
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(b.2.total_cmp(&a.2))
                    .then(b.0.cmp(&a.0))
            })
            .map(|(id, _, distance)| (id, distance))
    }
}

impl Shots {
    /// The damage every target took this tick, in id order.
    pub fn damage(&self) -> BTreeMap<EntityId, f64> {
        let mut damage: BTreeMap<EntityId, f64> = BTreeMap::new();
        for hit in &self.hits {
            *damage.entry(hit.target).or_default() += hit.damage;
        }
        damage
    }

    /// Where this tick's shots were fired and landed, as `sight` gives
    /// them: one entry per place and seat, in place then seat order,
    /// counting only shots by or on an entity that sight shows.
    pub fn exchanges(&self, state: &State, sight: &Sight) -> Vec<Exchange> {
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
        for hit in &self.hits {
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
}
