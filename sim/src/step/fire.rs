use std::collections::BTreeMap;

use crate::ids::{AsteroidId, EntityId, SeatId};
use crate::post::Post;
use crate::roster::Weapon;
use crate::state::{AssignedDamage, Ready, Rolls, Shooter, State};
use crate::time::Moment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub(crate) shooter: EntityId,
    pub(crate) weapon: u8,
    pub(crate) target: EntityId,
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
    pub(crate) ready: Vec<Ready>,
    pub(crate) exchanges: Vec<Exchange>,
}

pub(crate) struct Fire<'a> {
    state: &'a State,
    rolls: &'a Rolls<'a>,
}

impl<'a> Fire<'a> {
    pub(crate) fn of(state: &'a State, rolls: &'a Rolls<'a>) -> Fire<'a> {
        Fire { state, rolls }
    }

    pub(crate) fn run(self) -> Shots {
        let mut shots = Shots::default();
        let mut assigned = AssignedDamage::default();
        for ready in self.ready() {
            let shooter = self.state.entity(ready.entity());
            let Some(here) = shooter.standing() else {
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
            let roll = &self.rolls[here];
            let Some(aim) = roll.best(
                Shooter {
                    team: self.state[shooter.seat()].team(),
                    plating: self.state[shooter.row()].plating,
                },
                roll.body_of(shooter).pos,
                range.0,
                &assigned,
            ) else {
                continue;
            };
            let plating = self.state[self.state.entity(aim.target).row()].plating.0;
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
        shots.exchanges = Shots::exchanged(&shots.hits, self.state);
        shots
    }

    fn ready(&self) -> Vec<Ready> {
        let now = Moment::at(self.state.time().next());
        let mut ready: Vec<Ready> = self
            .state
            .ready()
            .filter(|ready| ready.at() < now)
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
}

impl Shots {
    pub fn damage(&self) -> BTreeMap<EntityId, f64> {
        let mut damage: BTreeMap<EntityId, f64> = BTreeMap::new();
        for hit in &self.hits {
            *damage.entry(hit.target).or_default() += hit.damage;
        }
        damage
    }

    fn exchanged(hits: &[Hit], state: &State) -> Vec<Exchange> {
        let mut found: BTreeMap<Post, Exchange> = BTreeMap::new();
        let mut note = |id: EntityId, landed: bool| {
            let entity = state.entity(id);
            let Some(asteroid) = entity.standing() else {
                return;
            };
            let post = Post {
                asteroid,
                seat: entity.seat(),
            };
            let at = found.entry(post).or_insert(Exchange {
                asteroid,
                seat: post.seat,
                fired: false,
                landed: false,
            });
            match landed {
                true => at.landed = true,
                false => at.fired = true,
            }
        };
        for hit in hits {
            note(hit.shooter, false);
            note(hit.target, true);
        }
        found.into_values().collect()
    }
}
