use neumannarch_sim::roster::{FRIGATE, Roster, SHIPYARD};

use std::collections::BTreeMap;

use neumannarch_sim::state::MAX_WANT;

use crate::bots::scripted::funding::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::{Proposal, Reason};
use crate::harness::fixture::{Fixture, surveyed};

#[test]
fn a_proposal_is_funded_whole_or_not_at_all_until_the_budget_is_spent() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);
    let view = fixture.view(0);
    let roster = Roster::shipped();
    let survey = surveyed(&view, &roster);
    let yard = survey.held().first().copied().expect("a drafted shipyard");
    let asking = |count| Proposal {
        posting: survey.posting(yard, FRIGATE),
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
fn a_proposal_is_refused_where_no_builder_of_the_seat_stands() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);
    let view = fixture.view(0);
    let roster = Roster::shipped();
    let survey = surveyed(&view, &roster);
    let free = fixture.free(1)[0];

    let wants = Funding::over(
        &survey,
        &[Proposal {
            posting: survey.posting(free, FRIGATE),
            count: 1,
            reason: Reason::Offence,
        }],
    );

    assert_eq!(
        wants.get(&survey.posting(free, FRIGATE)),
        None,
        "nothing is built at an asteroid the seat does not build at"
    );
}

#[test]
fn a_want_nothing_stands_behind_and_no_proposal_asks_for_is_lowered_to_what_it_can_justify() {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(1)[0];
    fixture.want(0, free, SHIPYARD, 1);
    fixture.want(0, free, FRIGATE, 3);
    fixture.run(2);
    let view = fixture.view(0);
    let roster = Roster::shipped();
    let survey = surveyed(&view, &roster);
    let posting = survey.posting(free, FRIGATE);

    let wants = Funding::over(&survey, &[]);

    assert_eq!(view.want_of(posting), 3, "the seat wanted three frigates");
    assert_eq!(
        wants.get(&posting).copied(),
        Some(justified(&survey, posting)),
        "the want stood above what stands there and what a builder is building"
    );
    assert!(wants[&posting] < 3);
}
