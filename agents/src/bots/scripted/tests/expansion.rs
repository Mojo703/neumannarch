use neumannarch_sim::roster::Roster;

use neumannarch_sim::AsteroidId;

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::dice::Dice;
use crate::bots::scripted::expansion::*;
use crate::bots::scripted::personality::Personality;
use crate::harness::fixture::{Fixture, surveyed};

#[test]
fn a_free_asteroid_is_claimed_where_the_held_ones_run_short_of_cap_and_only_up_to_the_claims() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let short_of_room = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        survey.home().is_some() && survey.short_of_room(personality.demand(&survey))
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), None]);
    fixture.until(short_of_room);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    let mut commitments = Commitments::default();
    let mut dice = Dice::new(personality.seed);

    let mut claimed: Vec<AsteroidId> = Vec::new();
    for _ in 0..personality.claims + 2 {
        let proposals = Expansion::proposals(&survey, &personality, &mut commitments, &mut dice);
        let asking: Vec<AsteroidId> = proposals
            .iter()
            .map(|proposal| proposal.posting.asteroid())
            .filter(|asteroid| !claimed.contains(asteroid))
            .collect();
        claimed.extend(asking);
    }

    assert_eq!(
        commitments.claims(),
        personality.claims,
        "it claimed {claimed:?}"
    );
    assert!(
        claimed
            .iter()
            .all(|asteroid| !survey.occupied().contains(asteroid)),
        "it claimed an asteroid it already stands at"
    );
    assert!(
        claimed
            .iter()
            .all(|asteroid| !survey.enemy_asteroids.contains(asteroid)),
        "it claimed an asteroid an enemy holds"
    );
}
