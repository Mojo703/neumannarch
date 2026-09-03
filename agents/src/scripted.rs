//! The opponent the game ships.
//!
//! Good RTS and 4X opponents are not one policy; they are a few
//! independent layers that each state what they want, over a memory of a
//! map they cannot see all of. This one is built that way, and because the
//! only lever in this game is a want per row per place, every layer's
//! output is the same shape: a target composition at a place. The
//! decision is the difference between that and what the view says it has.
//!
//! The layers, in the order they take the stockpile:
//!
//! - **Opening.** The reserve placed at the richest rock its seat's index
//!   picks, which is all a seat can do before it holds anything.
//! - **Economy.** At every rock it has: extractors up to what the rock's
//!   richest material feeds, a fast builder at the first rocks, stores at
//!   home as the stockpile fills. It expands by claiming a near rich rock
//!   and moving a mobile builder there, and a claim that never arrives
//!   lapses, since the sim tells nobody that a send could not be planned.
//! - **Scouting.** One cheap unit per scout it keeps, walking to the rock
//!   it watched least recently, reassigned the moment that rock is in
//!   sight.
//! - **Army.** A value target at a ratio to the enemy value it estimates
//!   from sight, radar and memory, never below a floor, so an unscouted
//!   enemy is still respected. A share of it is posted at each of its own
//!   rocks where an enemy force was just seen, and the rest stages in the
//!   target rock's outer band until it outweighs what is at the rock, then
//!   moves into the inner band.
//! - **Counters** are not a layer but the army's mix: each armed row is
//!   rated by its damage through the plating the enemy has shown, per cost
//!   unit, skewed by the personality's taste for reach past the enemy's
//!   and for durability. A row that shoots farther than it sees gets a
//!   spotter, because fire needs sight.
//!
//! Every number is a constant of a [`Personality`], or of the module that
//! uses it, with its unit and the word hypothesis. Nothing here reads the
//! state, a clock, or anything the seat cannot see.

use probe_sim::roster::Roster;
use probe_sim::state::Command;
use probe_sim::state::view::View;

use crate::memory::Memory;
use crate::personality::Personality;
use crate::plan::Plan;
use crate::roles::Roles;
use crate::survey::Survey;
use crate::{Agent, Dice};

/// The shipped opponent: one personality, the roster it plays, and what it
/// remembers of a map it cannot see all of.
pub struct Scripted {
    personality: Personality,
    roster: Roster,
    roles: Roles,
    memory: Memory,
    dice: Dice,
}

impl Scripted {
    /// An agent playing `personality` over `roster`, seeded by the
    /// personality. The roster is match-constant and no view carries it, so
    /// an agent is built with the one its match runs.
    pub fn new(personality: Personality, roster: Roster) -> Scripted {
        Scripted {
            roles: Roles::of(&roster),
            dice: Dice::new(personality.seed),
            personality,
            roster,
            memory: Memory::default(),
        }
    }

    /// The personality it plays.
    pub fn personality(&self) -> &Personality {
        &self.personality
    }
}

impl Agent for Scripted {
    fn decide(&mut self, view: &View) -> Vec<Command> {
        self.memory.observe(view, &self.roster);
        let survey = Survey::of(view, &self.roster, &self.roles, &self.memory);
        let plan = Plan::of(&survey, &self.personality, &mut self.memory, &mut self.dice);
        plan.commands(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use probe_sim::belt::Belt;
    use probe_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use probe_sim::state::{Batch, Command, Seat, State};
    use probe_sim::state::{Rock, view::View};
    use probe_sim::step::fire::Shots;
    use probe_sim::{Band, Materials, RockId, SeatId, Sequence, TICKS_PER_SECOND, TeamId, Tick};

    use crate::{DECISION_INTERVAL, MAX_COMMANDS_PER_DECISION};

    /// How far into a match the play tests run, in seconds. Seven minutes:
    /// an agent has placed, saturated its first rock, built the second
    /// builder it takes to spare one for a claim, and landed that claim by
    /// then, measured against the shipped belt; a debug build steps it in
    /// seconds.
    const PLAYED: u64 = 420;

    /// What each seat starts with, in materials, as the shipped belt seats
    /// a match with.
    const STOCK: Materials = Materials::new(300.0, 100.0, 100.0);

    /// The first four rocks of the shipped belt: neighbours, so the two
    /// agents open beside each other and meet. The whole belt spreads them
    /// seven rocks apart, which no send crosses inside a match.
    fn neighbours() -> Vec<Rock> {
        Belt::fixed(Belt::GRAVITY).into_iter().take(4).collect()
    }

    /// A match of two teams over `rocks`, ending at `clock` seconds, each
    /// seat holding the shipped reserve.
    fn start(rocks: Vec<Rock>, clock: u64) -> State {
        let reserve = std::collections::BTreeMap::from([(SHIPYARD, 1), (CONSTRUCTOR, 1)]);
        let seats = [TeamId(0), TeamId(1)]
            .map(|team| Seat::new(team, STOCK, reserve.clone()))
            .to_vec();
        State::new(
            Tick(clock * TICKS_PER_SECOND as u64),
            0,
            Belt::GRAVITY,
            Roster::shipped(),
            rocks,
            seats,
        )
    }

    fn scripted(personality: Personality) -> Box<Scripted> {
        Box::new(Scripted::new(personality, Roster::shipped()))
    }

    /// `agents` playing `state` to its clock, and the damage dealt on the
    /// way, which the state at the clock no longer shows.
    ///
    /// A belt laid by hand is not a [`Setup`](probe_sim::Setup)'s to name,
    /// so this steps the state where a frontend drives a session.
    fn play(state: State, agents: Vec<(SeatId, Box<Scripted>)>) -> (State, f64) {
        let mut state = state;
        let mut playing: Vec<(Sequence, Box<Scripted>)> = agents
            .into_iter()
            .map(|(seat, agent)| (Sequence::new(seat), agent))
            .collect();
        let mut shots = Shots::default();
        let mut damage = 0.0;
        while !state.standings().over() {
            let mut batch = Batch::new();
            for (sequence, agent) in &mut playing {
                let seat = sequence.seat();
                let cadence = DECISION_INTERVAL.0;
                if state.tick().0 % cadence != u64::from(seat.0) % cadence {
                    continue;
                }
                let view = View::of(&state, seat, &shots);
                let decided = agent.decide(&view);
                for command in decided.into_iter().take(MAX_COMMANDS_PER_DECISION) {
                    let stamped = sequence.stamp(state.tick(), command);
                    assert_eq!(batch.insert(stamped.issued), Ok(()));
                }
            }
            let (next, outcome) = state.step(&batch);
            damage += outcome.shots.hits.iter().map(|hit| hit.damage).sum::<f64>();
            shots = outcome.shots;
            state = next;
        }
        (state, damage)
    }
    #[test]
    fn an_agent_opens_by_placing_its_reserve_at_one_rock() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let mut agent = scripted(Personality::turtle());

        let opening = agent.decide(&View::of(&state, SeatId(0), &Shots::default()));

        assert_eq!(
            opening.len(),
            state.seat(SeatId(0)).expect("a seat").reserve().len()
        );
        let places: Vec<_> = opening
            .iter()
            .map(|command| match command {
                Command::Want { place, count, .. } => {
                    assert_eq!(*count, 1);
                    *place
                }
            })
            .collect();
        assert!(places.iter().all(|place| place.band == Band::Inner));
        assert_eq!(places.first(), places.last(), "one rock, not several");
    }

    #[test]
    fn two_agents_open_on_different_rocks() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let opening = |seat| {
            scripted(Personality::turtle())
                .decide(&View::of(&state, seat, &Shots::default()))
                .first()
                .copied()
        };

        assert_ne!(opening(SeatId(0)), opening(SeatId(1)));
    }

    #[test]
    fn an_agent_holds_more_than_the_rock_it_opened_on_by_mid_match() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);

        let (state, _) = play(state, vec![(SeatId(0), scripted(Personality::turtle()))]);

        let team = state.standings().teams()[0];
        assert!(team.rocks >= 2, "it held {} rocks", team.rocks);
        assert!(team.value > 0.0);
        assert!(
            state
                .entities()
                .filter(|entity| entity.seat() == SeatId(0))
                .count()
                >= 8,
            "an economy of one rock is more than a shipyard and a constructor"
        );
        assert!(
            state.seats()[0].stockpile().stock().total() > 0.0,
            "it never ran itself dry"
        );
    }

    #[test]
    fn two_personalities_beside_each_other_fight_before_the_clock() {
        let state = start(neighbours(), PLAYED);

        let (_, damage) = play(
            state,
            vec![
                (SeatId(0), scripted(Personality::turtle())),
                (SeatId(1), scripted(Personality::expand())),
            ],
        );

        assert!(damage > 0.0, "no shot dealt damage in {PLAYED} seconds");
    }

    #[test]
    fn an_agent_that_lost_everything_asks_for_nothing() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let mut agent = scripted(Personality::expand());
        let mut view = View::of(&state, SeatId(0), &Shots::default());
        view.reserve.clear();

        assert_eq!(agent.decide(&view), Vec::new());
    }

    #[test]
    fn the_rock_an_agent_opens_on_is_one_of_the_richest() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let mut agent = scripted(Personality::turtle());
        let view = View::of(&state, SeatId(0), &Shots::default());

        let opening = agent.decide(&view);
        let Command::Want { place, .. } = opening.first().copied().expect("an opening");

        let best = view
            .terrain
            .iter()
            .map(|terrain| terrain.caps.total())
            .fold(0.0, f64::max);
        let opened = view.terrain_of(place.rock).expect("a rock").caps.total();
        assert!(opened >= 0.9 * best, "opened on {opened} against {best}");
        assert_ne!(place.rock, RockId(20), "not the thinnest rock on the belt");
    }
}
