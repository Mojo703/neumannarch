use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::pattern::EntityPattern as P;
use neumannarch_sim::{AsteroidId, Material};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::offence::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::Proposal;
use crate::harness::fixture::{Fixture, surveyed};

const RAIDERS: u32 = 6;

fn yields_on_completion() -> Vec<EntityPattern> {
    vec![
        P::Shipyard,
        P::Constructor,
        P::Storage,
        P::Extractor(Material::Metals),
        P::Extractor(Material::Volatiles),
        P::Extractor(Material::Energy),
    ]
}

fn raiders_against_a_shipyard() -> (Fixture, AsteroidId, AsteroidId) {
    let mut fixture = Fixture::drafted([None, None]);
    let free = fixture.free(2);
    let (mine, theirs) = (free[0], free[1]);
    fixture.want(0, mine, P::Shipyard, 1);
    fixture.want(1, theirs, P::Shipyard, 1);
    fixture.want(0, mine, P::Extractor(Material::Volatiles), 1);
    fixture.want(0, mine, P::Raider, RAIDERS);
    fixture.until(|fixture| {
        let view = fixture.view(0);
        surveyed(&view).standing(mine, P::Raider) == RAIDERS
    });
    (fixture, mine, theirs)
}

fn sent_to(proposals: &[Proposal], fixture: &Fixture, target: AsteroidId) -> u32 {
    let view = fixture.view(0);
    let survey = surveyed(&view);
    proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == target)
        .map(|proposal| {
            proposal
                .count
                .saturating_sub(survey.count(target, proposal.posting.pattern()))
        })
        .sum()
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn one_more_unit_that_does_damage_is_asked_for_at_every_asteroid_where_it_builds() {
    let personality = Personality::expand();
    let fixture = Fixture::drafted([Some(personality.clone()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view);
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
fn no_unit_that_does_damage_is_asked_for_at_an_asteroid_short_of_a_pattern_that_pays_on_completion()
{
    let personality = Personality::expand();
    let mut fixture = Fixture::drafted([None, None]);
    let mine = fixture.free(1)[0];
    fixture.want(0, mine, P::Shipyard, 1);
    fixture.want(0, mine, P::Extractor(Material::Metals), 1);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    assert!(
        survey.short_of(mine, &yields_on_completion()),
        "nothing at {mine:?} is short of a pattern that pays on completion"
    );

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == mine)
    {
        let pattern = proposal.posting.pattern();
        assert!(
            proposal.count <= survey.count(mine, pattern),
            "it asked for {} {} at {mine:?}, over the {} standing, while an extractor is unbuilt there",
            proposal.count,
            pattern.name(),
            survey.count(mine, pattern)
        );
    }
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn every_standing_unit_beyond_an_asteroids_garrison_is_homed_at_the_target() {
    let personality = Personality::expand();
    let (fixture, mine, theirs) = raiders_against_a_shipyard();
    let view = fixture.view(0);
    let survey = surveyed(&view);

    let proposals = Offence::proposals(&survey, &personality, &mut Commitments::default());

    let unit_cost = Personality::damage_unit_cost(&personality.shares(&survey));
    let garrison = personality.garrison(survey.threat_at(mine), unit_cost);
    let count = |at: AsteroidId, pattern: EntityPattern| {
        proposals
            .iter()
            .find(|proposal| proposal.posting == survey.posting(at, pattern))
            .map_or_else(|| survey.count(at, pattern), |proposal| proposal.count)
    };
    let mut kept = 0.0;
    let mut flown = 0;
    for (pattern, _) in personality.shares(&survey) {
        let flying = count(theirs, pattern) - survey.count(theirs, pattern);
        let held = count(mine, pattern).min(survey.standing(mine, pattern));
        assert_eq!(
            held + flying,
            survey.standing(mine, pattern),
            "the wave neither lost nor invented a {}",
            pattern.name()
        );
        flown += flying;
        kept += f64::from(held) * pattern.cost().total();
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
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = raiders_against_a_shipyard();
    fixture.want(0, theirs, P::Raider, 1);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    assert_eq!(
        survey.arriving(theirs, P::Raider),
        0,
        "the want was filled from a surplus, so nothing stands short there"
    );
    assert!(
        survey.want(theirs, P::Raider) > survey.count(theirs, P::Raider),
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
    let personality = Personality::expand();
    let (mut fixture, mine, theirs) = raiders_against_a_shipyard();
    fixture.want(0, theirs, P::Raider, 1);
    fixture.want(0, mine, P::Raider, RAIDERS - 1);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    assert_eq!(
        survey.arriving(theirs, P::Raider),
        1,
        "the surplus raider is not on its way to {theirs:?}"
    );
    let unit_cost = Personality::damage_unit_cost(&personality.shares(&survey));
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
