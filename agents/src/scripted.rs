use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;

use crate::commitments::Commitments;
use crate::personality::Personality;
use crate::plan::Plan;
use crate::roles::Roles;
use crate::survey::Survey;
use crate::{Agent, Dice};

pub struct Scripted {
    personality: Personality,
    roster: Roster,
    roles: Roles,
    commitments: Commitments,
    dice: Dice,
}

impl Scripted {
    pub fn new(personality: Personality, roster: Roster) -> Scripted {
        Scripted {
            roles: Roles::of(&roster),
            dice: Dice::new(personality.seed),
            personality,
            roster,
            commitments: Commitments::default(),
        }
    }

    pub fn personality(&self) -> &Personality {
        &self.personality
    }
}

impl Agent for Scripted {
    fn decide(&mut self, view: &View) -> Vec<Command> {
        self.commitments.settle(view, &self.roster);
        let survey = Survey::of(view, &self.roster, &self.roles);
        let plan = Plan::of(
            &survey,
            &self.personality,
            &mut self.commitments,
            &mut self.dice,
        );
        plan.commands(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neumannarch_sim::belt::Belt;
    use neumannarch_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use neumannarch_sim::state::{Batch, Command, Seat, State};
    use neumannarch_sim::state::{Rock, view::View};
    use neumannarch_sim::step::fire::Shots;
    use neumannarch_sim::{Materials, RockId, SeatId, Sequence, TICKS_PER_SECOND, TeamId, Tick};

    use crate::{DECISION_INTERVAL, MAX_COMMANDS_PER_DECISION};

    const PLAYED: u64 = 420;

    const STOCK: Materials = Materials::new(300.0, 100.0, 100.0);

    fn neighbours() -> Vec<Rock> {
        Belt::fixed(Belt::GRAVITY).into_iter().take(4).collect()
    }

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
        let rocks: Vec<_> = opening
            .iter()
            .map(|command| match command {
                Command::Want { rock, count, .. } => {
                    assert_eq!(*count, 1);
                    *rock
                }
            })
            .collect();
        assert_eq!(rocks.first(), rocks.last(), "one rock, not several");
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
        let Command::Want { rock, .. } = opening.first().copied().expect("an opening");

        let best = view
            .terrain
            .iter()
            .map(|terrain| terrain.caps.total())
            .fold(0.0, f64::max);
        let opened = view.terrain_of(rock).expect("a rock").caps.total();
        assert!(opened >= 0.9 * best, "opened on {opened} against {best}");
        assert_ne!(rock, RockId(20), "not the thinnest rock on the belt");
    }
}
