use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::pattern::EntityPattern as P;
use neumannarch_sim::{Material, Materials};

use crate::bots::scripted::personality::*;
use crate::bots::scripted::proposal::Reason;
use crate::harness::fixture::{Fixture, surveyed};

fn weights(personality: &Personality, plating: f64, range: f64) -> Vec<(EntityPattern, f64)> {
    personality.weights(plating, range)
}

fn of(weights: &[(EntityPattern, f64)], pattern: EntityPattern) -> f64 {
    weights
        .iter()
        .find(|(held, _)| *held == pattern)
        .map_or(0.0, |(_, weight)| *weight)
}

#[test]
fn every_mix_is_a_set_of_shares_over_the_patterns_that_do_damage() {
    for personality in [Personality::turtle(), Personality::expand()] {
        let weights = weights(&personality, 0.0, 0.0);
        assert_eq!(weights.len(), 3);
        assert!((weights.iter().map(|(_, share)| share).sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(weights.iter().all(|(_, share)| *share > 0.0));
    }
}

#[test]
fn plating_shifts_the_mix_off_the_pattern_it_blunts() {
    let personality = Personality::expand();
    let unplated = weights(&personality, 0.0, 0.0);
    let plated = weights(&personality, 2.0, 0.0);
    assert!(
        of(&plated, P::Raider) < of(&unplated, P::Raider),
        "the fastest, weakest hit loses most to plating"
    );
    assert!(of(&plated, P::Lancer) > of(&unplated, P::Lancer));
}

#[test]
fn a_taste_for_reach_shifts_the_mix_toward_the_longer_pattern() {
    let mut personality = Personality::expand();
    let flat = weights(&personality, 0.0, 6.0);
    personality.range_taste = 1.0;
    let keen = weights(&personality, 0.0, 6.0);
    assert!(of(&keen, P::Lancer) > of(&flat, P::Lancer));
    assert!(of(&keen, P::Raider) < of(&flat, P::Raider));
}

#[test]
fn a_taste_for_durability_shifts_the_mix_toward_the_plated_pattern() {
    let mut personality = Personality::expand();
    let flat = weights(&personality, 0.0, 0.0);
    personality.armour_taste = 2.0;
    let tough = weights(&personality, 0.0, 0.0);
    assert!(of(&tough, P::Frigate) > of(&flat, P::Frigate));
}

#[test]
fn a_pinned_mix_plays_only_the_patterns_it_names() {
    let personality = Personality {
        mix: Mix::Pinned(vec![(P::Frigate, 1.0)]),
        ..Personality::turtle()
    };

    let weights = weights(&personality, 0.0, 0.0);

    assert_eq!(weights, vec![(P::Frigate, 1.0)]);
}

#[test]
fn defence_and_offence_stand_above_the_economy_while_an_enemy_stands_and_the_army_is_behind() {
    let expand = Personality::expand();

    assert_eq!(
        expand.order(false, true),
        vec![
            Reason::Economy,
            Reason::Expansion,
            Reason::Defence,
            Reason::Offence
        ],
        "ahead and unthreatened, it funds its economy first and its army last"
    );
    assert_eq!(
        expand.order(false, false),
        vec![
            Reason::Offence,
            Reason::Economy,
            Reason::Expansion,
            Reason::Defence
        ]
    );
    assert_eq!(
        expand.order(true, false),
        vec![
            Reason::Defence,
            Reason::Offence,
            Reason::Economy,
            Reason::Expansion
        ]
    );
    let turtle = Personality::turtle();
    assert_eq!(
        turtle.order(true, false),
        vec![Reason::Defence, Reason::Offence, Reason::Economy],
        "a turtle never funds an expansion"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn the_shares_a_pick_is_weighed_by_discount_what_the_asteroids_already_drafted_supply() {
    let personality = Personality::expand();
    let fixture = Fixture::drafted([Some(personality.clone()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view);
    let caps = survey
        .held()
        .into_iter()
        .filter_map(|asteroid| view.terrain_of(asteroid))
        .fold(Materials::ZERO, |caps, terrain| caps + terrain.caps);
    let mix = personality.to_build(&survey);

    let wanted = personality.wanted_shares(&survey);

    let discount = |material: Material| {
        wanted[material] / (mix[material] / mix.total()).max(f64::MIN_POSITIVE)
    };
    let richest = Material::EVERY
        .into_iter()
        .max_by(|one, other| caps[*one].total_cmp(&caps[*other]))
        .expect("a material the asteroids hold");
    let poorest = Material::EVERY
        .into_iter()
        .min_by(|one, other| caps[*one].total_cmp(&caps[*other]))
        .expect("a material the asteroids lack");
    assert!(caps[richest] > caps[poorest], "the drafted caps are flat");
    assert!(
        discount(richest) < discount(poorest),
        "the material the drafted asteroids are richest in ({richest:?}, {} against {}) kept its weight",
        discount(richest),
        discount(poorest)
    );
}
