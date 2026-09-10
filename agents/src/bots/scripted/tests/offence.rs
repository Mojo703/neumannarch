use neumannarch_sim::roster::{METALS_EXTRACTOR, RAIDER, Roster, SHIPYARD, VOLATILES_EXTRACTOR};

use neumannarch_sim::{AsteroidId, RowId};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::offence::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::Proposal;
use crate::harness::fixture::{Fixture, surveyed};

const RAIDERS: u32 = 6;

fn raiders_against_a_shipyard() -> (Fixture, AsteroidId, AsteroidId) {
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
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn one_more_unit_that_does_damage_is_asked_for_at_every_asteroid_where_it_builds() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let fixture = Fixture::drafted([Some(personality.clone()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    let building = survey.building();

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    assert!(
        building.len() > 1,
        "the fixture builds at one asteroid, so nothing tells a spread ask from a single one"
    );
    for asteroid in &building {
        assert_eq!(
            survey.damage_value(*asteroid),
            0.0,
            "the fixture already stands a force that does damage at {asteroid:?}"
        );
        let asked: u32 = proposals
            .iter()
            .filter(|proposal| proposal.posting.asteroid() == *asteroid)
            .map(|proposal| proposal.count)
            .sum();
        assert_eq!(
            asked, 1,
            "it asked for {asked} units that do damage at {asteroid:?}"
        );
    }
    assert!(
        proposals
            .iter()
            .all(|proposal| building.contains(&proposal.posting.asteroid())),
        "it asked for a unit at an asteroid no builder of its own stands at"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn no_unit_that_does_damage_is_asked_for_at_an_asteroid_still_short_of_a_row_that_pays_on_completion()
 {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let mut fixture = Fixture::drafted([None, None]);
    let mine = fixture.free(1)[0];
    fixture.want(0, mine, SHIPYARD, 1);
    fixture.want(0, mine, METALS_EXTRACTOR, 1);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    assert!(
        survey.short_of(mine, &survey.roles.yields_on_completion()),
        "nothing at {mine:?} is short of a row that pays on completion"
    );

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == mine)
    {
        let row = proposal.posting.row();
        assert!(
            proposal.count <= survey.count(mine, row),
            "it asked for {} {} at {mine:?}, over the {} standing, while an extractor is unbuilt there",
            proposal.count,
            roster[row].name,
            survey.count(mine, row)
        );
    }
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn every_standing_unit_beyond_an_asteroids_garrison_is_homed_at_the_target() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let (fixture, mine, theirs) = raiders_against_a_shipyard();
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    let unit_cost = personality.damage_unit_cost(&roster, &personality.shares(&survey));
    let garrison = personality.garrison(survey.threat_at(mine), unit_cost);
    let count = |at: AsteroidId, row: RowId| {
        proposals
            .iter()
            .find(|proposal| proposal.posting == survey.posting(at, row))
            .map_or_else(|| survey.count(at, row), |proposal| proposal.count)
    };
    let mut kept = 0.0;
    let mut flown = 0;
    for (row, _) in personality.shares(&survey) {
        let flying = count(theirs, row) - survey.count(theirs, row);
        let held = count(mine, row).min(survey.standing(mine, row));
        assert_eq!(
            held + flying,
            survey.standing(mine, row),
            "the wave neither lost nor invented a {row:?}"
        );
        flown += flying;
        kept += f64::from(held) * roster[row].cost.total();
    }
    assert!(flown > 0, "the wave sent nothing: {proposals:?}");
    assert!(
        kept <= garrison + unit_cost,
        "it held {kept} back at {mine:?} where its garrison is {garrison}"
    );
    assert!(
        survey.enemy_asteroids.contains(&theirs),
        "the wave flies at {theirs:?}, which no enemy holds"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_target_standing_under_its_want_is_still_reinforced() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = raiders_against_a_shipyard();
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
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_wave_still_in_the_air_is_not_sent_a_second_time() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = raiders_against_a_shipyard();
    fixture.want(0, theirs, RAIDER, 1);
    fixture.want(0, mine, RAIDER, RAIDERS - 1);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    assert_eq!(
        survey.arriving(theirs, RAIDER),
        1,
        "the surplus raider is not on its way to {theirs:?}"
    );
    let unit_cost = personality.damage_unit_cost(survey.roster, &personality.shares(&survey));
    assert!(
        survey.damage_value(mine) > personality.garrison(survey.threat_at(mine), unit_cost),
        "nothing stands at {mine:?} beyond its garrison to send"
    );

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    assert_eq!(
        sent_to(&proposals, &fixture, theirs),
        0,
        "it sent after the wave it already has in the air: {proposals:?}"
    );
}
