use std::collections::{BTreeMap, BTreeSet};

use neumannarch_sim::pattern::{EntityPattern, Kind};
use neumannarch_sim::state::view::View;
use neumannarch_sim::{AsteroidId, Post, Posting, SeatId, Time};

use neumannarch_protocol::Bot;

use super::played_match::PlayedMatch;
use crate::bots::scripted::personality::armed_units;

const BRIMMING: f64 = 0.99;

const HOARDING_SECONDS: f64 = 60.0;

const BUILDERS_BUSY_SHARE: f64 = 0.9;

const WITHIN_REACH_METERS: f64 = 6_000.0;

const FALLING_SECONDS: f64 = 60.0;

const DRAFTED_ASTEROIDS: u32 = 2;

pub struct Guarantees {
    clock: Time,
    seated: Vec<Bot>,
    unbuilt_frame: Option<String>,
    unbuilt: BTreeMap<SeatId, BTreeSet<Posting>>,
    hoard: Option<String>,
    fielded: BTreeMap<SeatId, Time>,
    traded: Option<Time>,
    full_since: BTreeMap<SeatId, Time>,
    extraction: BTreeMap<SeatId, Extraction>,
    income_fell: Option<String>,
    growth: Option<String>,
}

#[derive(Clone, Copy, Default)]
struct Extraction {
    highest_per_second: f64,
    falling_since: Time,
}

impl Extraction {
    fn read(&mut self, pulled: f64, now: Time) -> f64 {
        if pulled >= self.highest_per_second {
            self.highest_per_second = pulled;
            self.falling_since = now;
        }
        now.since(self.falling_since).seconds()
    }

    fn excused(&mut self, now: Time) {
        self.falling_since = now;
    }
}

impl Guarantees {
    pub fn over(seated: &[Bot], clock: Time) -> Guarantees {
        let army: Vec<EntityPattern> = armed_units().collect();
        let mut played = PlayedMatch::of(seated, clock);
        let mut watched = Guarantees {
            clock,
            seated: seated.to_vec(),
            unbuilt_frame: None,
            unbuilt: BTreeMap::new(),
            hoard: None,
            fielded: BTreeMap::new(),
            traded: None,
            full_since: BTreeMap::new(),
            extraction: BTreeMap::new(),
            income_fell: None,
            growth: None,
        };
        while !played.over() {
            played.advance();
            let now = played.state().time();
            for seat in played.decided().to_vec() {
                watched.read_frames(&played.view(seat));
            }
            for seat in watched.seats() {
                let view = played.view(seat);
                watched.read_stockpile(&view, &army);
                watched.read_income(&view);
                if view
                    .present
                    .iter()
                    .any(|it| it.seat == seat && it.pattern.does_damage())
                {
                    watched.fielded.entry(seat).or_insert(now);
                }
            }
            if watched.traded.is_none() && played.hits() > 0 {
                watched.traded = Some(now);
            }
        }
        watched.read_growth(&played);
        watched
    }

    pub fn an_expand_bot_grows_where_a_turtle_sits(&self) -> Option<&str> {
        self.income_fell.as_deref().or(self.growth.as_deref())
    }

    pub fn every_unit_builds_where_a_builder_of_its_seat_stands(&self) -> Option<&str> {
        self.unbuilt_frame.as_deref()
    }

    pub fn both_sides_arm_and_trade_shots(&self) -> Option<String> {
        let arming = Time(self.clock.0 / 3);
        let trading = Time(self.clock.0 / 2);
        for seat in &self.seats() {
            let at = self.fielded.get(seat).copied();
            if at.is_none_or(|at| at > arming) {
                return Some(format!(
                    "seat {} fielded its first unit that does damage at {:?}, not inside {:.0}s",
                    seat.0,
                    at.map(Time::seconds),
                    arming.seconds()
                ));
            }
        }
        match self.traded {
            Some(at) if at <= trading => None,
            _ => Some(format!("no shot landed inside {:.0}s", trading.seconds())),
        }
    }

    pub fn no_stockpile_sits_full_with_builders_idle(&self) -> Option<&str> {
        self.hoard.as_deref()
    }

    fn seats(&self) -> Vec<SeatId> {
        (0..self.seated.len())
            .map(|at| SeatId(u8::try_from(at).expect("a seat a bot")))
            .collect()
    }

    fn plays(&self, seat: SeatId, bot: Bot) -> bool {
        self.seated.get(usize::from(seat.0)) == Some(&bot)
    }

    fn read_income(&mut self, view: &View) {
        if self.income_fell.is_some() || !self.plays(view.seat, Bot::Expand) {
            return;
        }
        let pulled = view.income.total();
        let extraction = self.extraction.entry(view.seat).or_default();
        let falling = extraction.read(pulled, view.time);
        if falling < FALLING_SECONDS {
            return;
        }
        if !free_within_reach(view) {
            extraction.excused(view.time);
            return;
        }
        let highest = extraction.highest_per_second;
        self.income_fell = Some(format!(
            "seat {} pulled {pulled:.1} a second at {:.0}s, under the {highest:.1} it had pulled, for the {falling:.0}s since it last matched that, with a free asteroid within reach at the end of it",
            view.seat.0,
            view.time.seconds()
        ));
    }

    fn read_growth(&mut self, played: &PlayedMatch) {
        let standings = played.state().standings();
        for seat in self.seats() {
            let view = played.view(seat);
            let held = standings
                .teams()
                .get(usize::from(seat.0))
                .map_or(0, |team| team.asteroids);
            let reach = u32::try_from(within_reach(&view).len()).unwrap_or(u32::MAX);
            if self.plays(seat, Bot::Expand) && 2 * held < reach {
                self.growth = Some(format!(
                    "the expand bot at seat {} held {held} asteroids of the {reach} within reach of the two it drafted",
                    seat.0
                ));
            }
            if self.plays(seat, Bot::Turtle) && held > DRAFTED_ASTEROIDS {
                self.growth = Some(format!(
                    "the turtle at seat {} held {held} asteroids, over the {DRAFTED_ASTEROIDS} it drafted",
                    seat.0
                ));
            }
        }
    }

    fn read_frames(&mut self, view: &View) {
        if self.unbuilt_frame.is_some() || !builds_anywhere(view) {
            return;
        }
        let before = self.unbuilt.remove(&view.seat).unwrap_or_default();
        let units = view
            .plans
            .iter()
            .filter(|(posting, _)| posting.pattern().kind() == Kind::Unit)
            .filter_map(|(posting, plan)| Some((*posting, plan.building?.built_at)));
        for (posting, yard) in units {
            if builds_at(view, yard, view.seat) {
                continue;
            }
            self.unbuilt.entry(view.seat).or_default().insert(posting);
            if !before.contains(&posting) {
                continue;
            }
            self.unbuilt_frame = Some(format!(
                "at {:.0}s seat {} builds the {} it wants at {:?} at {yard:?}, where no builder of its own stands, though it builds elsewhere",
                view.time.seconds(),
                view.seat.0,
                posting.pattern().name(),
                posting.asteroid()
            ));
            return;
        }
    }

    fn read_stockpile(&mut self, view: &View, army: &[EntityPattern]) {
        if self.hoard.is_some() {
            return;
        }
        let stock = view.stockpile.stock();
        let capacity = view.stockpile.capacity();
        let affordable = army.iter().any(|pattern| {
            (stock - pattern.cost())
                .amounts()
                .all(|(_, left)| left >= 0.0)
        });
        let build_rate: f64 = view
            .compositions
            .iter()
            .filter(|(post, _)| post.seat == view.seat)
            .flat_map(|(_, composition)| composition.patterns.iter())
            .map(|(pattern, held)| pattern.build_rate() * f64::from(held.present))
            .sum();
        let builders_busy = view.spend.total() >= BUILDERS_BUSY_SHARE * build_rate;
        if !affordable || builders_busy || stock.total() < BRIMMING * capacity.total() {
            self.full_since.remove(&view.seat);
            return;
        }
        let from = *self.full_since.entry(view.seat).or_insert(view.time);
        if view.time.since(from).seconds() >= HOARDING_SECONDS {
            self.hoard = Some(format!(
                "seat {} sat at its stockpile's capacity from {:.0}s to {:.0}s spending {:.0} a second against builders that could spend {build_rate:.0}, with a pattern that does damage it could afford",
                view.seat.0,
                from.seconds(),
                view.time.seconds(),
                view.spend.total()
            ));
        }
    }
}

fn builds_at(view: &View, asteroid: AsteroidId, seat: SeatId) -> bool {
    view.compositions
        .get(&Post { asteroid, seat })
        .is_some_and(|composition| composition.builder)
}

fn builds_anywhere(view: &View) -> bool {
    view.compositions
        .iter()
        .any(|(post, composition)| post.seat == view.seat && composition.builder)
}

fn within_reach(view: &View) -> Vec<AsteroidId> {
    let drafted: Vec<AsteroidId> = view
        .draft
        .stages()
        .iter()
        .filter(|stage| stage.seat == view.seat)
        .filter_map(|stage| stage.placed)
        .collect();
    view.terrain
        .iter()
        .map(|terrain| terrain.asteroid)
        .filter(|asteroid| {
            drafted
                .iter()
                .any(|from| apart(view, *from, *asteroid) <= WITHIN_REACH_METERS)
        })
        .collect()
}

fn free_within_reach(view: &View) -> bool {
    within_reach(view)
        .into_iter()
        .any(|asteroid| !is_taken(view, asteroid))
}

fn is_taken(view: &View, asteroid: AsteroidId) -> bool {
    view.compositions
        .iter()
        .filter(|(post, _)| post.asteroid == asteroid)
        .any(|(_, composition)| {
            composition
                .patterns
                .values()
                .any(|held| held.present + held.arriving > 0)
        })
}

fn apart(view: &View, from: AsteroidId, to: AsteroidId) -> f64 {
    match (view.asteroid_body(from), view.asteroid_body(to)) {
        (Some(from), Some(to)) => from.pos.distance(to.pos),
        _ => f64::INFINITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neumannarch_sim::TICKS_PER_SECOND;

    fn at(seconds: u64) -> Time {
        Time(seconds * u64::from(TICKS_PER_SECOND))
    }

    #[test]
    fn a_fall_is_measured_from_the_last_tick_the_seat_matched_the_most_it_ever_pulled() {
        let mut extraction = Extraction::default();

        assert_eq!(extraction.read(10.0, at(10)), 0.0);
        assert_eq!(
            extraction.read(4.0, at(30)),
            20.0,
            "the fall did not run from the tick the bar was last matched"
        );
        assert_eq!(
            extraction.read(9.0, at(70)),
            60.0,
            "climbing back short of the bar cleared the fall"
        );
        assert_eq!(
            extraction.highest_per_second, 10.0,
            "the bar fell to what the seat pulls now"
        );

        assert_eq!(
            extraction.read(10.0, at(80)),
            0.0,
            "matching the most it ever pulled did not clear the fall"
        );
    }

    #[test]
    fn an_excused_fall_runs_again_from_the_tick_it_was_excused() {
        let mut extraction = Extraction::default();
        extraction.read(10.0, at(10));
        assert_eq!(extraction.read(4.0, at(70)), 60.0);

        extraction.excused(at(70));

        assert_eq!(
            extraction.read(4.0, at(100)),
            30.0,
            "an excused seat is measured from before it was excused"
        );
        assert_eq!(
            extraction.highest_per_second, 10.0,
            "the excuse lowered the bar the seat is held to"
        );
    }
}
