use std::collections::BTreeMap;

use crate::ids::{AsteroidId, EntityId, SeatId};
use crate::post::Post;
use crate::state::{AssignedDamage, Reach, Ready, Rolls, Shooter, State};
use crate::time::Moment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub(crate) shooter: EntityId,
    pub(crate) place: u8,
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
        for ready in self.due() {
            let shooter = self.state.entity(ready.entity());
            let Some(here) = shooter.standing() else {
                shots.lapse(&ready);
                continue;
            };
            let hitscan = ready.place().hitscan;
            let roll = &self.rolls[here];
            let Some(aim) = roll.best(
                Shooter {
                    team: self.state[shooter.seat()].team(),
                    plating: self.state[shooter.row()].plating,
                },
                roll.body_of(shooter).pos,
                Reach::WithinMeters(hitscan.range.0),
                &assigned,
                ready.kept(),
            ) else {
                shots.lapse(&ready);
                continue;
            };
            let plating = self.state[self.state.entity(aim.target).row()].plating.0;
            let dealt = (hitscan.damage.0
                * (1.0 - hitscan.falloff.0 * aim.distance / hitscan.range.0)
                - plating)
                .max(0.0);
            assigned.take(aim.target, dealt);
            shots.hits.push(Hit {
                shooter: shooter.id(),
                place: ready.place().in_row,
                target: aim.target,
                damage: dealt,
            });
            shots.ready.push(Ready::new(
                ready.entity(),
                ready.place(),
                self.next_ready(ready.at(), hitscan.rate.0),
                Some(aim.target),
            ));
        }
        for ready in self.reloading() {
            if !self.keep_stands(ready) {
                shots.lapse(ready);
            }
        }
        shots.exchanges = Shots::exchanged(&shots.hits, self.state);
        shots
    }

    fn due(&self) -> Vec<Ready> {
        let now = Moment::at(self.state.time().next());
        let mut due: Vec<Ready> = self
            .state
            .ready()
            .filter(|ready| ready.at() < now)
            .cloned()
            .collect();
        due.sort_by(|a, b| {
            a.at()
                .cmp(&b.at())
                .then(a.entity().cmp(&b.entity()))
                .then(a.place().in_row.cmp(&b.place().in_row))
        });
        due
    }

    fn reloading(&self) -> impl Iterator<Item = &Ready> {
        let now = Moment::at(self.state.time().next());
        self.state.ready().filter(move |ready| ready.at() >= now)
    }

    fn keep_stands(&self, ready: &Ready) -> bool {
        let Some(kept) = ready.kept() else {
            return true;
        };
        let shooter = self.state.entity(ready.entity());
        let Some(here) = shooter.standing() else {
            return false;
        };
        let target = self.state.entity(kept);
        if target.standing() != Some(here) {
            return false;
        }
        let roll = &self.rolls[here];
        roll.body_of(target).pos.distance(roll.body_of(shooter).pos)
            <= ready.place().hitscan.range.0
    }

    fn next_ready(&self, at: Moment, rate: f64) -> Moment {
        let interval = if rate > 0.0 { 1.0 / rate } else { f64::MAX };
        at.after(interval).max(Moment::at(self.state.time()))
    }
}

impl Shots {
    fn lapse(&mut self, ready: &Ready) {
        if ready.kept().is_some() {
            self.ready
                .push(Ready::new(ready.entity(), ready.place(), ready.at(), None));
        }
    }

    pub(crate) fn hit_by(&self, target: EntityId, shooter: EntityId) -> bool {
        self.hits
            .iter()
            .any(|hit| hit.target == target && hit.shooter == shooter)
    }

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
