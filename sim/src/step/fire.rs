use std::collections::BTreeMap;

use crate::ids::{EntityId, RockId, SeatId};
use crate::roster::Weapon;
use crate::state::sweep::Sweep;
use crate::state::{Entity, Ready, State};
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
    pub rock: RockId,
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
                    .is_some_and(|entity| !entity.is_flying(self.state.tick()))
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
        at.after(interval).max(Moment::at(self.state.tick()))
    }

    fn target(
        &self,
        shooter: &Entity,
        range: f64,
        assigned: &BTreeMap<EntityId, f64>,
    ) -> Option<(EntityId, f64)> {
        let team = self.state[shooter.seat()].team();
        let from = self.state.body_of(shooter).pos;
        let plating = self.state[shooter.row()].plating.0;
        let now = self.state.tick();
        let here = shooter.standing(now)?;
        self.sweep
            .within(from, range)
            .filter_map(|id| self.state.entity(id))
            .filter(|target| {
                self.state[target.seat()].team() != team
                    && target.standing(now) == Some(here)
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
    pub fn damage(&self) -> BTreeMap<EntityId, f64> {
        let mut damage: BTreeMap<EntityId, f64> = BTreeMap::new();
        for hit in &self.hits {
            *damage.entry(hit.target).or_default() += hit.damage;
        }
        damage
    }

    pub fn exchanges(&self, state: &State) -> Vec<Exchange> {
        let mut found: BTreeMap<(RockId, SeatId), (bool, bool)> = BTreeMap::new();
        let mut note = |id: EntityId, landed: bool| {
            let Some(entity) = state.entity(id) else {
                return;
            };
            let Some(rock) = entity.standing(state.tick()) else {
                return;
            };
            let at = found.entry((rock, entity.seat())).or_insert((false, false));
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
            .map(|((rock, seat), (fired, landed))| Exchange {
                rock,
                seat,
                fired,
                landed,
            })
            .collect()
    }
}
