use neumannarch_sim::roster::CONSTRUCTOR;
use neumannarch_sim::state::{Command, State, view::View};
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{AsteroidId, Materials, Post, SeatId, Setup};

use neumannarch_sim::roster::Roster;

use crate::Agent;
use crate::bots::scripted::Scripted;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::roles::Roles;
use crate::harness::fixture::Fixture;
use crate::harness::played_match::{free_for_all, minutes};

const SEEDS: u64 = 8;

const OPENING_SECONDS: u64 = 8;

const SETTLED_SECONDS: u64 = 240;

fn scripted(personality: Personality) -> Scripted {
    Scripted::new(personality, Roster::shipped())
}

fn opening(state: &State, seat: SeatId) -> Vec<Command> {
    scripted(Personality::turtle()).decide(&View::of(state, seat, &Shots::default()))
}

#[test]
fn an_agent_asks_for_one_reserve_row_at_a_free_asteroid_only_once_its_window_is_open() {
    let fixture = Fixture::seating([None, None]);
    let state = fixture.state();
    let picking = state.draft().stages()[0].seat;
    let waiting = state.draft().stages()[1].seat;

    let placing = opening(state, picking);

    assert_eq!(placing.len(), 1, "one structure, not the whole reserve");
    let Command::Want {
        asteroid,
        row,
        count,
    } = placing[0];
    assert_eq!(count, 1);
    assert!(
        state[picking].reserved(row) > 0,
        "a row it holds in reserve"
    );
    assert!(!state.is_taken(asteroid), "an asteroid no seat has taken");
    assert_eq!(
        opening(state, waiting),
        Vec::new(),
        "the seat whose window is shut asks nothing"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn two_agents_draft_four_asteroids_and_no_seat_takes_a_asteroid_another_took() {
    let fixture = Fixture::drafted([Some(Personality::turtle()), Some(Personality::expand())]);

    let mine = fixture.standing(0);
    let theirs = fixture.standing(1);

    assert_eq!(mine.len(), 2, "a seat drafts one asteroid per reserve row");
    assert_eq!(theirs.len(), 2);
    assert!(
        mine.iter().all(|asteroid| !theirs.contains(asteroid)),
        "{mine:?} and {theirs:?} share an asteroid"
    );
    assert_eq!(
        fixture.state().draft().ended(),
        Some(fixture.state().tick().back(1))
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_bot_keeps_the_constructor_it_drafted_where_it_stands_and_builds_there() {
    let mut fixture = Fixture::drafted([Some(Personality::expand()), None]);
    fixture.run(OPENING_SECONDS);

    let state = fixture.state();
    let asteroid = state
        .entities()
        .find(|entity| entity.seat() == SeatId(0) && entity.row() == CONSTRUCTOR)
        .map(|entity| entity.home())
        .expect("it drafted its constructor somewhere");
    let post = Post {
        asteroid,
        seat: SeatId(0),
    };

    assert_eq!(
        state.count(post, CONSTRUCTOR),
        1,
        "it stayed where it landed"
    );
    assert!(
        state.frames_at(post).count() > 0,
        "nothing built at {asteroid:?} in the opening {OPENING_SECONDS} seconds"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn an_agent_holds_more_than_the_asteroid_it_opened_on_by_mid_match() {
    let mut fixture = Fixture::drafted([Some(Personality::turtle()), None]);
    fixture.run(SETTLED_SECONDS);

    let state = fixture.state();
    let team = state.standings().teams()[0];
    assert!(team.asteroids >= 2, "it held {} asteroids", team.asteroids);
    assert!(team.value > 0.0);
    assert!(
        state
            .entities()
            .filter(|entity| entity.seat() == SeatId(0))
            .count()
            >= 8,
        "an economy of one asteroid is more than a shipyard and a constructor"
    );
    assert!(
        state.seats()[0].stockpile().stock().total() > 0.0,
        "it never ran itself dry"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn an_agent_with_nothing_of_its_own_on_the_map_asks_for_nothing() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);

    let asking = scripted(Personality::expand()).decide(&fixture.view(1));

    assert!(
        fixture.standing(1).is_empty(),
        "the seat that let its stages lapse stands somewhere"
    );
    assert_eq!(asking, Vec::new());
}

#[test]
fn a_bots_first_draft_pick_is_the_asteroid_richest_in_the_mix_it_means_to_build() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let shares = personality.weights(&roster, &Roles::of(&roster), 0.0, 0.0);
    let mix = shares
        .iter()
        .map(|(row, share)| roster[*row].cost * *share)
        .fold(Materials::ZERO, |mix, cost| mix + cost);

    for seed in 0..SEEDS {
        let setup = Setup::new(free_for_all(2), seed, minutes(7)).expect("a match");
        let state = State::start(&setup);
        let picking = state.draft().stages()[0].seat;
        let view = View::of(&state, picking, &Shots::default());
        let fit = |at: AsteroidId| {
            view.terrain_of(at).map_or(0.0, |terrain| {
                terrain
                    .caps
                    .amounts()
                    .map(|(material, cap)| cap * mix[material])
                    .sum::<f64>()
            })
        };
        let richest = view
            .terrain
            .iter()
            .map(|terrain| terrain.asteroid)
            .max_by(|one, other| fit(*one).total_cmp(&fit(*other)).then(other.cmp(one)))
            .expect("a belt of asteroids");

        let picked = scripted(personality.clone()).decide(&view);

        let Command::Want { asteroid, .. } = *picked.first().expect("a pick");
        assert_eq!(
            asteroid,
            richest,
            "seed {seed}: it took {asteroid:?} yielding {} where {richest:?} yields {}",
            fit(asteroid),
            fit(richest)
        );
    }
}
