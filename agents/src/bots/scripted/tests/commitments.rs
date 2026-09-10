use neumannarch_sim::roster::Roster;

use crate::bots::scripted::commitments::*;
use crate::bots::scripted::personality::Personality;
use crate::harness::fixture::{Fixture, surveyed};

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn an_army_counts_as_ahead_only_once_it_passes_the_saving_ratio_and_stays_ahead_to_the_lower_one() {
    let roster = Roster::shipped();
    let ratio = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        match survey.enemy() > 0.0 {
            true => survey.army() / survey.enemy(),
            false => 0.0,
        }
    };
    let mut fixture = Fixture::drafted([Some(Personality::expand()), Some(Personality::expand())]);
    fixture.until(|fixture| ratio(fixture) > AHEAD_BEGINS);
    let mut kept = Commitments::default();
    let ahead = fixture.view(0);
    kept.settle(&surveyed(&ahead, &roster));
    assert!(
        kept.army_ahead,
        "it passed the ratio without counting itself ahead"
    );

    fixture.until(|fixture| {
        let ratio = ratio(fixture);
        ratio > AHEAD_ENDS && ratio < AHEAD_BEGINS
    });
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    kept.settle(&survey);
    let mut fresh = Commitments::default();
    fresh.settle(&survey);

    assert!(
        kept.army_ahead,
        "it gave up a lead of {} against {} above {AHEAD_ENDS}",
        survey.army(),
        survey.enemy()
    );
    assert!(
        !fresh.army_ahead,
        "it counted the same lead as ahead from behind, under {AHEAD_BEGINS}"
    );
}
