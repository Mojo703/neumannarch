use std::collections::{BTreeMap, BTreeSet};

use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::view::View;
use neumannarch_sim::{Posting, RowId, SeatId, Tick, Time};

use crate::personality::Personality;
use crate::played_match::PlayedMatch;
use crate::roles::Roles;

const BRIMMING: f64 = 0.99;

const HOARDING_SECONDS: f64 = 60.0;

const BUILDERS_BUSY_SHARE: f64 = 0.9;

pub struct Guarantees {
    clock: Time,
    seats: Vec<SeatId>,
    unbuilt_frame: Option<String>,
    unbuilt: BTreeMap<SeatId, BTreeSet<Posting>>,
    hoard: Option<String>,
    fielded: BTreeMap<SeatId, Time>,
    traded: Option<Time>,
    full_since: BTreeMap<SeatId, Time>,
}

impl Guarantees {
    pub fn over(personalities: &[Personality], clock: Tick) -> Guarantees {
        let roster = Roster::shipped();
        let army = Roles::of(&roster).army;
        let mut played = PlayedMatch::of(personalities, clock);
        let mut watched = Guarantees {
            clock: Time(clock.0),
            seats: (0..personalities.len())
                .map(|at| SeatId(u8::try_from(at).expect("a seat a personality")))
                .collect(),
            unbuilt_frame: None,
            unbuilt: BTreeMap::new(),
            hoard: None,
            fielded: BTreeMap::new(),
            traded: None,
            full_since: BTreeMap::new(),
        };
        while !played.over() {
            played.advance();
            let now = played.state().time();
            for seat in played.decided().to_vec() {
                watched.read_frames(&played.view(seat), &roster);
            }
            for seat in watched.seats.clone() {
                let view = played.view(seat);
                watched.read_stockpile(&view, &roster, &army);
                if view
                    .present
                    .iter()
                    .any(|it| it.seat == seat && roster[it.row].is_armed())
                {
                    watched.fielded.entry(seat).or_insert(now);
                }
            }
            if watched.traded.is_none() && played.hits() > 0 {
                watched.traded = Some(now);
            }
        }
        watched
    }

    pub fn no_frame_outlives_a_decision_without_a_builder(&self) -> Option<&str> {
        self.unbuilt_frame.as_deref()
    }

    pub fn both_sides_arm_and_trade_shots(&self) -> Option<String> {
        let arming = Time(self.clock.0 / 3);
        let trading = Time(self.clock.0 / 2);
        for seat in &self.seats {
            let at = self.fielded.get(seat).copied();
            if at.is_none_or(|at| at > arming) {
                return Some(format!(
                    "seat {} fielded its first armed unit at {:?}, not inside {:.0}s",
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

    fn read_frames(&mut self, view: &View, roster: &Roster) {
        if self.unbuilt_frame.is_some() {
            return;
        }
        let before = self.unbuilt.remove(&view.seat).unwrap_or_default();
        let building = view
            .plans
            .iter()
            .filter(|(_, plan)| plan.building.is_some())
            .map(|(posting, _)| *posting);
        for posting in building {
            let want = view.want_of(posting);
            let composition = view.compositions.get(&posting.post());
            let arriving = composition.is_some_and(|composition| {
                composition
                    .rows
                    .iter()
                    .any(|(row, held)| held.arriving > 0 && builds(roster, *row))
            });
            if composition.is_some_and(|composition| composition.builder) || arriving {
                continue;
            }
            self.unbuilt.entry(view.seat).or_default().insert(posting);
            if !before.contains(&posting) {
                continue;
            }
            let homed = composition
                .and_then(|composition| composition.rows.get(&posting.row()))
                .map_or(0, |held| held.present + held.arriving);
            self.unbuilt_frame = Some(format!(
                "at {:.0}s seat {} still wants {want} {} at {:?} where {homed} are homed, and no builder of its own stands or arrives there",
                view.time.seconds(),
                view.seat.0,
                named(roster, posting.row()),
                posting.asteroid()
            ));
            return;
        }
    }

    fn read_stockpile(&mut self, view: &View, roster: &Roster, army: &[RowId]) {
        if self.hoard.is_some() {
            return;
        }
        let stock = view.stockpile.stock();
        let capacity = view.stockpile.capacity();
        let affordable = army
            .iter()
            .filter_map(|row| roster.get(*row))
            .any(|row| (stock - row.cost).amounts().all(|(_, left)| left >= 0.0));
        let build_rate: f64 = view
            .compositions
            .iter()
            .filter(|(post, _)| post.seat == view.seat)
            .flat_map(|(_, composition)| composition.rows.iter())
            .filter_map(|(row, held)| {
                roster
                    .get(*row)
                    .map(|stats| stats.builds().sum::<f64>() * f64::from(held.present))
            })
            .sum();
        let builders_busy = view.spend.total() >= BUILDERS_BUSY_SHARE * build_rate;
        if !affordable || builders_busy || stock.total() < BRIMMING * capacity.total() {
            self.full_since.remove(&view.seat);
            return;
        }
        let from = *self.full_since.entry(view.seat).or_insert(view.time);
        if view.time.since(from).seconds() >= HOARDING_SECONDS {
            self.hoard = Some(format!(
                "seat {} sat at its stockpile's capacity from {:.0}s to {:.0}s spending {:.0} a second against builders that could spend {build_rate:.0}, with an armed row it could afford",
                view.seat.0,
                from.seconds(),
                view.time.seconds(),
                view.spend.total()
            ));
        }
    }
}

fn builds(roster: &Roster, row: RowId) -> bool {
    roster
        .get(row)
        .is_some_and(|row| row.builds().next().is_some())
}

fn named(roster: &Roster, row: RowId) -> &str {
    roster.get(row).map_or("an unknown row", |row| row.name)
}
