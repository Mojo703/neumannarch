use neumannarch_sim::AsteroidId;

use crate::bots::scripted::defence::*;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::proposal::Proposal;
use crate::harness::fixture::{Fixture, surveyed};

fn worth(proposals: &[Proposal], at: AsteroidId) -> f64 {
    proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == at)
        .map(|proposal| proposal.posting.pattern().cost().total() * f64::from(proposal.count))
        .sum()
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_garrison_is_asked_for_where_an_enemy_force_stands_and_nowhere_else() {
    let personality = Personality::turtle();
    let raided = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view);
        survey
            .developed()
            .into_iter()
            .any(|asteroid| survey.threat_at(asteroid) > 0.0)
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), Some(Personality::expand())]);
    fixture.until(raided);
    let view = fixture.view(0);
    let survey = surveyed(&view);

    let proposals = Defence::proposals(&survey, &personality);

    for asteroid in survey.developed() {
        let threat = survey.threat_at(asteroid);
        match threat > 0.0 {
            true => assert!(
                worth(&proposals, asteroid) >= threat,
                "it asked for {} at {asteroid:?} against a force worth {threat}",
                worth(&proposals, asteroid)
            ),
            false => assert_eq!(
                worth(&proposals, asteroid),
                0.0,
                "it garrisoned {asteroid:?}, where no enemy stands"
            ),
        }
    }
}
