use std::collections::BTreeMap;

use neumannarch_sim::AsteroidId;
use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::pattern::EntityPattern as P;
use neumannarch_sim::state::MAX_WANT;

use crate::bots::scripted::funding::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::{Proposal, Reason};
use crate::harness::fixture::{Fixture, surveyed};

const FREE_ASTEROIDS: usize = 32;

fn standing(fixture: &Fixture, asteroid: AsteroidId, pattern: EntityPattern) -> u32 {
    let view = fixture.view(0);
    surveyed(&view).standing(asteroid, pattern)
}

fn a_frigate_flown_where_no_builder_of_its_seat_stands() -> (Fixture, AsteroidId, AsteroidId) {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(FREE_ASTEROIDS);
    let home = free[0];
    fixture.want(0, home, P::Shipyard, 1);
    let away = {
        let view = fixture.view(0);
        surveyed(&view)
            .nearest(home, &free[1..])
            .expect("the belt leaves a second asteroid free")
    };
    fixture.want(0, home, P::Frigate, 2);
    fixture.until(|fixture| standing(fixture, home, P::Frigate) == 2);
    fixture.want(0, home, P::Frigate, 1);
    fixture.want(0, away, P::Frigate, 1);
    fixture.until(|fixture| standing(fixture, away, P::Frigate) == 1);
    fixture.want(0, away, P::Frigate, 2);
    (fixture, home, away)
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_proposal_is_funded_whole_or_not_at_all_until_the_budget_is_spent() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let yard = survey.held().first().copied().expect("a drafted shipyard");
    let asking = |count| Proposal {
        posting: survey.posting(yard, P::Frigate),
        count,
        reason: Reason::Offence,
    };
    let mut funding = Funding::new(&survey, &BTreeMap::new());

    assert!(
        funding.affords(&survey, &asking(1)),
        "one frigate is inside the stockpile a match opens with"
    );
    assert!(
        !funding.affords(&survey, &asking(MAX_WANT)),
        "a stockpile worth a handful of frigates bought {MAX_WANT} of them"
    );
    assert!(
        funding.affords(&survey, &asking(1)),
        "the refused proposal spent the budget anyway"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_proposal_is_refused_where_no_builder_of_the_seat_stands() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let free = fixture.free(1)[0];

    let wants = Funding::over(
        &survey,
        &[Proposal {
            posting: survey.posting(free, P::Frigate),
            count: 1,
            reason: Reason::Offence,
        }],
    );

    assert_eq!(
        wants.get(&survey.posting(free, P::Frigate)),
        None,
        "nothing is built at an asteroid the seat does not build at"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_want_nothing_stands_behind_and_no_proposal_asks_for_is_lowered_to_what_it_can_justify() {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(1)[0];
    fixture.want(0, free, P::Shipyard, 1);
    fixture.want(0, free, P::Frigate, 3);
    fixture.run(2);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let posting = survey.posting(free, P::Frigate);

    let wants = Funding::over(&survey, &[]);

    assert_eq!(view.want_of(posting), 3, "the seat wanted three frigates");
    assert_eq!(
        wants.get(&posting).copied(),
        Some(justified(&survey, posting)),
        "the want stood above what stands there and what a builder is building"
    );
    assert!(wants[&posting] < 3);
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_frame_no_builder_of_the_seat_can_fill_funds_nothing_behind_it() {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(1)[0];
    fixture.want(0, free, P::Frigate, 1);
    fixture.run(2);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let posting = survey.posting(free, P::Frigate);
    assert!(
        survey.frame_open(free, P::Frigate),
        "no frame stands at {free:?} to fund"
    );
    assert!(!survey.builds_at(free), "a builder reached {free:?}");

    let wants = Funding::over(
        &survey,
        &[Proposal {
            posting,
            count: 1,
            reason: Reason::Offence,
        }],
    );

    assert_eq!(
        wants.get(&posting),
        None,
        "a frame nothing can build stood in for the unit the want asked for"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_frame_no_builder_of_the_seat_can_fill_leaves_no_want_behind_it() {
    let (fixture, home, away) = a_frigate_flown_where_no_builder_of_its_seat_stands();
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let posting = survey.posting(away, P::Frigate);
    assert!(
        survey.frame_open(away, P::Frigate),
        "no frame stands at {away:?} to leave anything behind"
    );
    assert!(
        !survey.builds_at(away),
        "a builder of the seat reached {away:?}"
    );
    assert_eq!(
        survey.standing(away, P::Frigate),
        1,
        "the flown frigate never stood at {away:?}"
    );

    let wants = Funding::over(
        &survey,
        &[
            Proposal {
                posting: survey.posting(home, P::Frigate),
                count: 0,
                reason: Reason::Offence,
            },
            Proposal {
                posting,
                count: 2,
                reason: Reason::Offence,
            },
        ],
    );

    assert_eq!(
        wants.get(&posting),
        None,
        "it kept wanting at {away:?}, where nothing of its own can build"
    );
}
