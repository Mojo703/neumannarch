use neumannarch_sim::Material;
use neumannarch_sim::roster::{METALS_EXTRACTOR, Roster, SHIPYARD};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::economy::*;
use crate::bots::scripted::personality::Personality;
use crate::harness::fixture::{Fixture, surveyed};

#[test]
fn an_extractor_is_asked_for_where_the_asteroid_has_cap_to_spare_and_not_where_it_has_none() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let pulled_dry = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        let yielding = survey.roles.yields_on_completion();
        let dry = survey.developed().into_iter().any(|asteroid| {
            Material::EVERY
                .into_iter()
                .any(|material| survey.spare_cap_at(asteroid, material) <= 0.0)
        });
        let extracting = |row| survey.roles.extractors.iter().any(|(_, it)| *it == row);
        let asking = Economy::proposals(&survey, &personality, &Commitments::default())
            .iter()
            .any(|proposal| {
                let (asteroid, row) = (proposal.posting.asteroid(), proposal.posting.row());
                extracting(row)
                    && proposal.count > survey.count(asteroid, row)
                    && !survey.short_of(asteroid, &yielding)
            });
        dry && asking
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), None]);
    fixture.until(pulled_dry);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);

    let proposals = Economy::proposals(&survey, &personality, &Commitments::default());

    let asked = |asteroid, row| {
        proposals
            .iter()
            .any(|proposal| proposal.posting == survey.posting(asteroid, row))
    };
    let yielding = survey.roles.yields_on_completion();
    let mut spared = 0;
    for asteroid in survey.developed() {
        let short = survey.short_of(asteroid, &yielding);
        for (material, row) in &survey.roles.extractors {
            match survey.spare_cap_at(asteroid, *material) > 0.0 && !short {
                true => spared += usize::from(asked(asteroid, *row)),
                false => assert!(
                    !asked(asteroid, *row),
                    "it asked for a {material:?} extractor at {asteroid:?}, where nothing is left to take or a row it is already building pays on completion"
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
fn no_second_row_that_pays_on_completion_is_asked_for_where_one_is_still_unbuilt() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let mut fixture = Fixture::drafted([None, None]);
    let asteroid = fixture.free(1)[0];
    fixture.want(0, asteroid, SHIPYARD, 1);
    let alone = {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        Economy::proposals(&survey, &personality, &Commitments::default())
            .iter()
            .filter(|proposal| proposal.posting.asteroid() == asteroid)
            .filter(|proposal| proposal.count > survey.count(asteroid, proposal.posting.row()))
            .count()
    };
    assert!(
        alone > 0,
        "the asteroid with nothing building was offered nothing to build"
    );

    fixture.want(0, asteroid, METALS_EXTRACTOR, 1);

    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    assert!(
        survey.short_of(asteroid, &survey.roles.yields_on_completion()),
        "the extractor the seat asked for at {asteroid:?} is already built"
    );
    let proposals = Economy::proposals(&survey, &personality, &Commitments::default());
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.posting.asteroid() == asteroid)
    {
        let row = proposal.posting.row();
        assert!(
            proposal.count <= survey.count(asteroid, row),
            "it asked for {} {} at {asteroid:?}, over the {} standing, while an extractor is unbuilt there",
            proposal.count,
            roster[row].name,
            survey.count(asteroid, row)
        );
    }
}

#[test]
fn one_more_constructor_stands_at_home_only_while_a_claim_is_meant_and_none_is_spare() {
    let roster = Roster::shipped();
    let personality = Personality::expand();
    let short_of_room = |fixture: &Fixture| {
        let view = fixture.view(0);
        let survey = surveyed(&view, &roster);
        let Some(home) = survey.home() else {
            return false;
        };
        survey.short_of_room(personality.demand(&survey))
            && !survey.short_of(home, &survey.roles.yields_on_completion())
    };
    let mut fixture = Fixture::drafted([Some(personality.clone()), None]);
    fixture.until(short_of_room);
    let view = fixture.view(0);
    let survey = surveyed(&view, &roster);
    let mason = survey.roles.masons[0];
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
