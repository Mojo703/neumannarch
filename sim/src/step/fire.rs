use std::collections::BTreeMap;

use crate::ids::{AsteroidId, EntityId, SeatId};
use crate::roster::Weapon;
use crate::state::sweep::Sweep;
use crate::state::{Aim, Assigned, Entity, Ready, State, Threat};
use crate::time::Moment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub shooter: EntityId,
    pub weapon: u8,
    pub target: EntityId,
    pub damage: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exchange {
    pub asteroid: AsteroidId,
    pub seat: SeatId,
    pub fired: bool,
    pub landed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shots {
    pub hits: Vec<Hit>,
    pub ready: Vec<Ready>,
}

pub struct Fire<'a> {
    state: &'a State,
    sweep: &'a Sweep,
}

impl<'a> Fire<'a> {
    pub fn of(state: &'a State, sweep: &'a Sweep) -> Fire<'a> {
        Fire { state, sweep }
    }

    pub fn run(self) -> Shots {
        let mut shots = Shots::default();
        let mut assigned = Assigned::default();
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
            let Some(aim) = self.aim(shooter, range.0, &assigned) else {
                continue;
            };
            let plating = self.state[self.state[aim.target].row()].plating.0;
            let dealt = (damage.0 * (1.0 - falloff.0 * aim.distance / range.0) - plating).max(0.0);
            assigned.take(aim.target, dealt);
            shots.hits.push(Hit {
                shooter: shooter.id(),
                weapon: ready.weapon(),
                target: aim.target,
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

    fn ready(&self) -> Vec<Ready> {
        let now = Moment::at(self.state.time().next());
        let mut ready: Vec<Ready> = self
            .state
            .ready()
            .iter()
            .filter(|ready| ready.at() < now)
            .filter(|ready| {
                self.state
                    .entity(ready.entity())
                    .is_some_and(|entity| !entity.is_flying(self.state.time()))
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

    fn next_ready(&self, at: Moment, rate: f64) -> Moment {
        let interval = if rate > 0.0 { 1.0 / rate } else { f64::MAX };
        at.after(interval).max(Moment::at(self.state.time()))
    }

    fn aim(&self, shooter: &Entity, range: f64, assigned: &Assigned) -> Option<Aim> {
        let from = self.state.body_of(shooter).pos;
        Threat::of(self.state, shooter)?.best(
            self.sweep
                .within(from, range)
                .filter_map(|id| self.state.entity(id)),
            assigned,
        )
    }
}

impl Shots {
    pub fn damage(&self) -> BTreeMap<EntityId, f64> {
        let mut damage: BTreeMap<EntityId, f64> = BTreeMap::new();
        for hit in &self.hits {
            *damage.entry(hit.target).or_default() += hit.damage;
        }
        damage
    }

    pub fn exchanges(&self, state: &State) -> Vec<Exchange> {
        let mut found: BTreeMap<(AsteroidId, SeatId), (bool, bool)> = BTreeMap::new();
        let mut note = |id: EntityId, landed: bool| {
            let Some(entity) = state.entity(id) else {
                return;
            };
            let Some(asteroid) = entity.standing(state.time()) else {
                return;
            };
            let at = found
                .entry((asteroid, entity.seat()))
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
            .map(|((asteroid, seat), (fired, landed))| Exchange {
                asteroid,
                seat,
                fired,
                landed,
            })
            .collect()
    }
}
