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
    use neumannarch_sim::Post;
    use neumannarch_sim::belt::Belt;
    use neumannarch_sim::roster::{CONSTRUCTOR, FRIGATE, LANCER, SHIPYARD};
    use neumannarch_sim::state::{Batch, Command, Seat, State};
    use neumannarch_sim::state::{Rock, view::View};
    use neumannarch_sim::step::fire::Shots;
    use neumannarch_sim::{
        Material, Materials, RockId, SeatId, Sequence, TICKS_PER_SECOND, TeamId, Time,
    };

    use crate::{DECISION_INTERVAL, MAX_COMMANDS_PER_DECISION, Mix};

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
            Time(clock * TICKS_PER_SECOND as u64),
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
        playing(state, agents, |state| state.standings().over())
    }

    fn drafted(agents: Vec<(SeatId, Box<Scripted>)>) -> State {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        playing(state, agents, |state| !state.drafting()).0
    }

    fn playing(
        state: State,
        agents: Vec<(SeatId, Box<Scripted>)>,
        done: impl Fn(&State) -> bool,
    ) -> (State, f64) {
        let mut state = state;
        let mut playing: Vec<(Sequence, Box<Scripted>)> = agents
            .into_iter()
            .map(|(seat, agent)| (Sequence::new(seat), agent))
            .collect();
        let mut shots = Shots::default();
        let mut damage = 0.0;
        while !done(&state) {
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
    fn an_agent_asks_for_one_reserve_row_at_a_free_rock_only_once_its_window_is_open() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let picking = state.draft().stages()[0].seat;
        let mut agent = scripted(Personality::turtle());

        let opening = agent.decide(&View::of(&state, picking, &Shots::default()));

        assert_eq!(opening.len(), 1, "one structure, not the whole reserve");
        let Command::Want { rock, row, count } = opening[0];
        assert_eq!(count, 1);
        assert!(
            state[picking].reserved(row) > 0,
            "a row it holds in reserve"
        );
        assert_eq!(state.draft().took(rock), None, "a rock no seat has taken");

        let waiting = state.draft().stages()[1].seat;
        let shut =
            scripted(Personality::turtle()).decide(&View::of(&state, waiting, &Shots::default()));

        assert_eq!(
            shut,
            Vec::new(),
            "the seat whose window is shut asks nothing"
        );
    }

    #[test]
    fn two_agents_draft_four_rocks_and_no_seat_takes_a_rock_another_took() {
        let state = drafted(vec![
            (SeatId(0), scripted(Personality::turtle())),
            (SeatId(1), scripted(Personality::expand())),
        ]);

        let draft = state.draft();
        let mine: Vec<RockId> = draft.placements(SeatId(0)).map(|(rock, _)| rock).collect();
        let theirs: Vec<RockId> = draft.placements(SeatId(1)).map(|(rock, _)| rock).collect();

        assert_eq!(mine.len(), 2, "a seat drafts one rock per reserve row");
        assert_eq!(theirs.len(), 2);
        assert!(
            mine.iter().all(|rock| !theirs.contains(rock)),
            "{mine:?} and {theirs:?} share a rock"
        );
        assert_eq!(draft.ended(), Some(state.tick().back(1)));
    }

    #[test]
    fn a_bot_keeps_the_constructor_it_drafted_where_it_stands_and_builds_there() {
        let opening = 8 * TICKS_PER_SECOND as u64;
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let (state, _) = playing(
            state,
            vec![(SeatId(0), scripted(Personality::expand()))],
            |state| state.time().0 >= opening,
        );

        let (rock, _) = state
            .draft()
            .placements(SeatId(0))
            .find(|(_, row)| *row == CONSTRUCTOR)
            .expect("it drafted its constructor somewhere");
        let post = Post {
            rock,
            seat: SeatId(0),
        };

        assert_eq!(
            state.count(post, CONSTRUCTOR),
            1,
            "it stayed where it landed"
        );
        assert!(
            state.frames_at(post).count() > 0,
            "nothing built at {rock:?} in the opening eight seconds"
        );
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
    fn a_bot_drafts_the_rock_richest_in_what_the_mix_it_means_to_build_wants_most() {
        let state = start(Belt::fixed(Belt::GRAVITY), PLAYED);
        let picking = state.draft().stages()[0].seat;
        let view = View::of(&state, picking, &Shots::default());
        let hungry_for = |row| {
            let personality = Personality {
                mix: Mix::Pinned(vec![(row, 1.0)]),
                ..Personality::turtle()
            };
            let opening = scripted(personality).decide(&view);
            let Command::Want { rock, .. } = *opening.first().expect("a pick");
            view.terrain_of(rock).expect("a rock").caps
        };
        let richest = |material| {
            view.terrain
                .iter()
                .map(|terrain| terrain.caps[material])
                .fold(0.0, f64::max)
        };

        let metals = hungry_for(FRIGATE);
        let energy = hungry_for(LANCER);

        assert_eq!(
            metals[Material::Metals],
            richest(Material::Metals),
            "the frigate's cost is metals before all else"
        );
        assert!(
            energy[Material::Energy] >= metals[Material::Energy],
            "the lancer wants energy more than the frigate does"
        );
    }
}
