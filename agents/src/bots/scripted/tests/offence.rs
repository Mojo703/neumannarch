use neumannarch_sim::roster::{RAIDER, Roster, SHIPYARD, VOLATILES_EXTRACTOR};

use neumannarch_sim::{AsteroidId, RowId};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::offence::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::Proposal;
use crate::harness::fixture::{Fixture, surveyed};

const RAIDERS: u32 = 6;

fn armed_against_a_yard() -> (Fixture, AsteroidId, AsteroidId) {
    let roster = Roster::shipped();
    let mut fixture = Fixture::drafted([None, None]);
    let free = fixture.free(2);
    let (mine, theirs) = (free[0], free[1]);
    fixture.want(0, mine, SHIPYARD, 1);
    fixture.want(1, theirs, SHIPYARD, 1);
    fixture.want(0, mine, VOLATILES_EXTRACTOR, 1);
    fixture.want(0, mine, RAIDER, RAIDERS);
    fixture.until(|fixture| {
        let view = fixture.view(0);
        surveyed(&view, &roster).standing(mine, RAIDER) == RAIDERS
    });
    (fixture, mine, theirs)
}

fn sent_to(proposals: &[Proposal], fixture: &Fixture, target: AsteroidId) -> u32 {
    let roster = Roster::shipped();
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == target)
        .map(|proposal| {
            proposal
                .count
                .saturating_sub(survey.count(target, proposal.posting.row()))
        })
        .sum()
}

#[test]
fn one_more_armed_unit_is_asked_for_at_the_staging_asteroid_before_a_shot_is_worth_firing() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let fixture = Fixture::drafted([Some(personality.clone()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    let staging = survey.staging().expect("it builds somewhere");

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    assert_eq!(
        survey.armed_value(staging),
        0.0,
        "the fixture already stands an armed force"
    );
    assert!(
        proposals
            .iter()
            .all(|proposal| proposal.posting.asteroid() == staging),
        "it armed an asteroid it does not stage at"
    );
    assert_eq!(
        proposals
            .iter()
            .filter(|proposal| proposal.count > 0)
            .count(),
        1,
        "one whole unit a decision: {proposals:?}"
    );
}

#[test]
fn every_armed_unit_beyond_the_garrison_is_homed_at_the_target_when_the_force_is_enough() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let worth_sending = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        let Some(staging) = survey.staging() else {
            return false;
        };
        let unit_cost = personality.armed_unit_cost(&roster, &personality.shares(&survey));
        !survey.enemy_asteroids.is_empty()
            && survey.armed_value(staging)
                > personality.garrison(survey.threat_at(staging), unit_cost) + unit_cost
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), Some(Personality::turtle())]);
    fixture.until(worth_sending);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    let staging = survey.staging().expect("it builds somewhere");

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    let target = proposals
        .iter()
        .map(|proposal| proposal.posting.asteroid())
        .find(|asteroid| *asteroid != staging)
        .expect("a wave names the asteroid it flies at");
    let unit_cost = personality.armed_unit_cost(&roster, &personality.shares(&survey));
    let garrison = personality.garrison(survey.threat_at(staging), unit_cost);
    let count = |at: AsteroidId, row: RowId| {
        proposals
            .iter()
            .find(|proposal| proposal.posting == survey.posting(at, row))
            .map_or_else(|| survey.count(at, row), |proposal| proposal.count)
    };
    let mut kept = 0.0;
    for (row, _) in personality.shares(&survey) {
        let flying = count(target, row) - survey.count(target, row);
        assert_eq!(
            count(staging, row) + flying,
            survey.count(staging, row),
            "the wave neither lost nor invented a {row:?}"
        );
        kept += f64::from(count(staging, row)) * roster[row].cost.total();
    }
    assert!(
        kept <= garrison + unit_cost,
        "it held {kept} back at {staging:?} where its garrison is {garrison}"
    );
    assert!(
        survey.enemy_asteroids.contains(&target),
        "the wave flies at {target:?}, which no enemy holds"
    );
}

#[test]
fn a_target_standing_under_its_want_is_still_reinforced() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = armed_against_a_yard();
    fixture.want(0, theirs, RAIDER, 1);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    assert_eq!(
        survey.arriving(theirs, RAIDER),
        0,
        "the want was filled from a surplus, so nothing stands short there"
    );
    assert!(
        survey.want(theirs, RAIDER) > survey.count(theirs, RAIDER),
        "the target already holds everything the seat wants there"
    );

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    assert!(
        sent_to(&proposals, &fixture, theirs) > 0,
        "it held its force at {mine:?} rather than reinforce {theirs:?}: {proposals:?}"
    );
}

#[test]
fn a_wave_still_in_the_air_is_not_sent_a_second_time() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = armed_against_a_yard();
    fixture.want(0, theirs, RAIDER, 1);
    fixture.want(0, mine, RAIDER, RAIDERS - 1);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    assert_eq!(
        survey.arriving(theirs, RAIDER),
        1,
        "the surplus raider is not on its way to {theirs:?}"
    );
    let unit_cost = personality.armed_unit_cost(survey.roster, &personality.shares(&survey));
    assert!(
        survey.armed_value(mine) > personality.garrison(survey.threat_at(mine), unit_cost),
        "nothing stands at {mine:?} beyond its garrison to send"
    );

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    assert_eq!(
        sent_to(&proposals, &fixture, theirs),
        0,
        "it sent after the wave it already has in the air: {proposals:?}"
    );
}
