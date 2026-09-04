use std::collections::BTreeMap;

use probe_sim::state::view::View;
use probe_sim::{RockId, SeatId, TICKS_PER_SECOND, Tick};

use crate::display::scene::Arc;

const RECENT: u64 = 3 * TICKS_PER_SECOND as u64 / 2;

const FORGET: u64 = 10 * TICKS_PER_SECOND as u64;

#[derive(Clone, Debug, Default)]
pub struct Fights {
    fights: BTreeMap<(RockId, SeatId), Fight>,
}

#[derive(Clone, Debug)]
struct Fight {
    started: f64,
    hp: f64,
    recent: Vec<(Tick, f64)>,
    last_shot: Tick,
}

impl Fights {
    pub fn observe(&mut self, view: &View) {
        let totals = totals(view);
        let hp_at = |key: &(RockId, SeatId)| totals.get(key).copied().unwrap_or(0.0);
        for exchange in &view.exchanges {
            let key = (exchange.rock, exchange.seat);
            self.fights
                .entry(key)
                .or_insert_with(|| Fight::new(hp_at(&key), view.tick))
                .last_shot = view.tick;
        }
        for (key, fight) in &mut self.fights {
            fight.drain(hp_at(key), view.tick);
        }
        self.fights
            .retain(|_, fight| view.tick.0.saturating_sub(fight.last_shot.0) <= FORGET);
    }

    pub fn arcs(&self) -> impl Iterator<Item = (RockId, Arc)> + '_ {
        self.fights
            .iter()
            .filter_map(|((rock, seat), fight)| Some((*rock, fight.arc(*seat)?)))
    }
}

impl Fight {
    fn new(hp: f64, tick: Tick) -> Fight {
        Fight {
            started: hp,
            hp,
            recent: Vec::new(),
            last_shot: tick,
        }
    }

    fn arc(&self, seat: SeatId) -> Option<Arc> {
        (self.started > 0.0).then(|| {
            let share = |hp: f64| (hp / self.started).clamp(0.0, 1.0) as f32;
            Arc {
                seat,
                fraction: share(self.hp),
                trailing: share(self.hp + self.recent.iter().map(|(_, hit)| hit).sum::<f64>()),
            }
        })
    }

    fn drain(&mut self, hp: f64, tick: Tick) {
        let damage = self.hp - hp;
        if damage > 0.0 {
            self.recent.push((tick, damage));
        }
        self.recent
            .retain(|(at, _)| tick.0.saturating_sub(at.0) <= RECENT);
        self.hp = hp;
        self.started = self.started.max(hp);
    }
}

fn totals(view: &View) -> BTreeMap<(RockId, SeatId), f64> {
    let mut totals: BTreeMap<(RockId, SeatId), f64> = BTreeMap::new();
    for held in &view.present {
        let Some(rock) = held.at.standing() else {
            continue;
        };
        *totals.entry((rock, held.seat)).or_insert(0.0) += held.hp;
    }
    totals
}

#[cfg(test)]
mod tests {
    use probe_sim::belt::Belt;
    use probe_sim::orbit::Body;
    use probe_sim::state::Standings;
    use probe_sim::state::view::{Berth, Present};
    use probe_sim::step::fire::Exchange;
    use probe_sim::{EntityId, Materials, RockId, RowId, Stockpile, TeamId, Vec3};

    use super::*;

    const ROCK: RockId = RockId(0);

    const SEAT: SeatId = SeatId(0);

    fn view(tick: u64, hp: f64, shooting: bool) -> View {
        View {
            seat: SEAT,
            tick: Tick(tick),
            clock: Tick(u64::MAX),
            gravity: Belt::GRAVITY,
            stockpile: Stockpile::new(Materials::ZERO, Materials::ZERO),
            reserve: BTreeMap::new(),
            compositions: Vec::new(),
            plans: Vec::new(),
            present: vec![Present {
                id: EntityId(0),
                row: RowId(0),
                seat: SEAT,
                body: Body::new(Vec3::ZERO, Vec3::ZERO),
                hp,
                home: ROCK,
                at: Berth::Standing(ROCK),
            }],
            teams: Box::new([TeamId(0)]),
            exchanges: match shooting {
                true => vec![Exchange {
                    rock: ROCK,
                    seat: SEAT,
                    fired: false,
                    landed: true,
                }],
                false => Vec::new(),
            },
            terrain: Vec::new(),
            zone: Belt::ZONE_RADIUS_METERS,
            standings: Standings::new(Vec::new(), false),
        }
    }

    fn arc(fights: &Fights) -> Option<Arc> {
        fights.arcs().next().map(|(_, arc)| arc)
    }

    #[test]
    fn no_shots_draw_no_arc() {
        let mut fights = Fights::default();
        fights.observe(&view(0, 100.0, false));
        assert_eq!(arc(&fights), None);
    }

    #[test]
    fn an_arc_is_full_at_the_first_shot_and_drains_with_the_hp() {
        let mut fights = Fights::default();
        fights.observe(&view(0, 100.0, true));
        assert_eq!(arc(&fights).expect("the fight started").fraction, 1.0);

        fights.observe(&view(1, 40.0, true));

        assert_eq!(arc(&fights).expect("the fight goes on").fraction, 0.4);
    }

    #[test]
    fn damage_trails_the_drain_for_a_second_and_a_half() {
        let mut fights = Fights::default();
        fights.observe(&view(0, 100.0, true));
        fights.observe(&view(1, 70.0, true));

        let hit = arc(&fights).expect("the fight goes on");
        assert_eq!(hit.fraction, 0.7);
        assert_eq!(hit.trailing, 1.0, "the last hit still trails");

        let after = 1 + RECENT + 1;
        fights.observe(&view(after, 70.0, false));

        let caught = arc(&fights).expect("the arc stands for ten seconds");
        assert_eq!(caught.trailing, caught.fraction, "the trail caught up");
    }

    #[test]
    fn an_arc_is_forgotten_ten_seconds_after_the_last_shot() {
        let mut fights = Fights::default();
        fights.observe(&view(0, 100.0, true));

        fights.observe(&view(FORGET, 100.0, false));
        assert!(arc(&fights).is_some(), "ten seconds is still drawn");

        fights.observe(&view(FORGET + 1, 100.0, false));
        assert_eq!(arc(&fights), None);
    }

    #[test]
    fn a_seat_wiped_out_at_the_rock_drains_to_nothing_and_still_draws() {
        let mut fights = Fights::default();
        fights.observe(&view(0, 100.0, true));

        let mut gone = view(1, 0.0, true);
        gone.present.clear();
        fights.observe(&gone);

        assert_eq!(arc(&fights).expect("the loss is readable").fraction, 0.0);
    }
}
