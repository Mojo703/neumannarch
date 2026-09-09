use neumannarch_sim::roster::Roster;

use neumannarch_sim::{AsteroidId, RowId};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::offence::*;
use crate::bots::scripted::personality::Personality;
use crate::harness::fixture::{Fixture, surveyed};

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
