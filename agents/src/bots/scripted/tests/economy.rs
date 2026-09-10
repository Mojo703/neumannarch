use neumannarch_sim::Material;
use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::pattern::EntityPattern as P;

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::economy::*;
use crate::bots::scripted::personality::Personality;
use crate::harness::fixture::{Fixture, surveyed};

fn yields_on_completion() -> Vec<P> {
    vec![
        P::Shipyard,
        P::Constructor,
        P::Storage,
        P::Extractor(Material::Metals),
        P::Extractor(Material::Volatiles),
        P::Extractor(Material::Energy),
    ]
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn an_extractor_is_asked_for_where_the_asteroid_has_cap_to_spare_and_not_where_it_has_none() {
    let personality = Personality::expand();
    let pulled_dry = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view);
        let yielding = yields_on_completion();
        let dry = survey.developed().into_iter().any(|asteroid| {
            Material::EVERY
                .into_iter()
                .any(|material| survey.spare_cap_at(asteroid, material) <= 0.0)
        });
        let extracting = |pattern: EntityPattern| matches!(pattern, P::Extractor(_));
        let asking = Economy::proposals(&survey, &personality, &Commitments::default())
            .iter()
            .any(|proposal| {
                let (asteroid, pattern) = (proposal.posting.asteroid(), proposal.posting.pattern());
                extracting(pattern)
                    && proposal.count > survey.count(asteroid, pattern)
                    && !survey.short_of(asteroid, &yielding)
            });
        dry && asking
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), None]);
    fixture.until(pulled_dry);
    let view = fixture.view(0);
    let survey = surveyed(&view);

    let proposals = Economy::proposals(&survey, &personality, &Commitments::default());

    let asked = |asteroid, pattern| {
        proposals
            .iter()
            .any(|proposal| proposal.posting == survey.posting(asteroid, pattern))
    };
    let yielding = yields_on_completion();
    let mut spared = 0;
    for asteroid in survey.developed() {
        let short = survey.short_of(asteroid, &yielding);
        for material in [Material::Metals, Material::Volatiles, Material::Energy] {
            let pattern = P::Extractor(material);
            match survey.spare_cap_at(asteroid, material) > 0.0 && !short {
                true => spared += usize::from(asked(asteroid, pattern)),
                false => assert!(
                    !asked(asteroid, pattern),
                    "it asked for a {material:?} extractor at {asteroid:?}, where nothing is left to take or a pattern it is already building pays on completion"
                ),
            }
        }
    }
    assert!(
        spared > 0,
        "no asteroid with cap to spare was offered an extractor"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn no_second_pattern_that_pays_on_completion_is_asked_for_where_one_is_still_unbuilt() {
    let personality = Personality::expand();
    let mut fixture = Fixture::drafted([None, None]);
    let asteroid = fixture.free(1)[0];
    fixture.want(0, asteroid, P::Shipyard, 1);
    let alone = {
        let view = fixture.view(0);
        let survey = surveyed(&view);
        Economy::proposals(&survey, &personality, &Commitments::default())
            .iter()
            .filter(|proposal| proposal.posting.asteroid() == asteroid)
            .filter(|proposal| proposal.count > survey.count(asteroid, proposal.posting.pattern()))
            .count()
    };
    assert!(
        alone > 0,
        "the asteroid with nothing building was offered nothing to build"
    );

    fixture.want(0, asteroid, P::Extractor(Material::Metals), 1);

    let view = fixture.view(0);
    let survey = surveyed(&view);
    assert!(
        survey.short_of(asteroid, &yields_on_completion()),
        "the extractor the seat asked for at {asteroid:?} is already built"
    );
    let proposals = Economy::proposals(&survey, &personality, &Commitments::default());
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == asteroid)
    {
        let pattern = proposal.posting.pattern();
        assert!(
            proposal.count <= survey.count(asteroid, pattern),
            "it asked for {} {} at {asteroid:?}, over the {} standing, while an extractor is unbuilt there",
            proposal.count,
            pattern.name(),
            survey.count(asteroid, pattern)
        );
    }
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn one_more_constructor_stands_at_home_only_while_a_claim_is_meant_and_none_is_spare() {
    let personality = Personality::expand();
    let short_of_room = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view);
        let Some(home) = survey.home() else {
            return false;
        };
        survey.short_of_room(personality.demand(&survey))
            && !survey.short_of(home, &yields_on_completion())
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), None]);
    fixture.until(short_of_room);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let mason = P::Constructor;
    let home = survey.home().expect("a builder stands somewhere");
    let count = |commitments: &Commitments| {
        Economy::proposals(&survey, &personality, commitments)
            .iter()
            .find(|proposal| proposal.posting == survey.posting(home, mason))
            .map_or(0, |proposal| proposal.count)
    };

    let claiming = count(&Commitments::default());
    let mut spent = Commitments::default();
    for asteroid in fixture.free(personality.claims) {
        spent.claim(asteroid, view.time);
    }
    let claimed = count(&spent);

    let spare = survey.count(home, mason).saturating_sub(personality.masons);
    assert_eq!(spare, 0, "the fixture already stands a spare constructor");
    assert_eq!(
        claiming,
        personality.masons + 1,
        "it stands the constructors it keeps and one to send"
    );
    assert_eq!(
        claimed, personality.masons,
        "it asked for another constructor with every claim already out"
    );
}
