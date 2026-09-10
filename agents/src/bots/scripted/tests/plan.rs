use std::collections::BTreeMap;

use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::pattern::EntityPattern as P;
use neumannarch_sim::state::{Command, MAX_COMMANDS_PER_TICK, MAX_WANT};
use neumannarch_sim::{Posting, SeatId};

use crate::bots::scripted::commitments::Commitments;
use crate::bots::scripted::dice::Dice;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::plan::*;
use crate::harness::fixture::{Fixture, surveyed};

fn damage_patterns(count: usize) -> Vec<EntityPattern> {
    let patterns: Vec<EntityPattern> = EntityPattern::EVERY
        .into_iter()
        .filter(|pattern| pattern.does_damage())
        .collect();
    (0..count).map(|at| patterns[at % patterns.len()]).collect()
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_want_the_plan_no_longer_carries_is_lowered_to_nothing() {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(1)[0];
    fixture.want(0, free, P::Shipyard, 1);
    fixture.want(0, free, P::Frigate, 3);
    fixture.run(1);
    let view = fixture.view(0);
    let plan = Plan {
        wants: BTreeMap::new(),
    };

    let commands = plan.commands(&view);

    assert!(
        commands.contains(&want(Posting::of(free, SeatId(0), P::Frigate), 0)),
        "the standing want was left alone: {commands:?}"
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_decision_asks_for_nothing_at_an_asteroid_no_builder_of_the_seat_stands_at() {
    let fixture = Fixture::drafted([Some(Personality::expand()), None]);
    let view = fixture.view(0);
    let survey = surveyed(&view);

    let plan = Plan::of(
        &survey,
        &Personality::expand(),
        &mut Commitments::default(),
        &mut Dice::new(0),
    );

    assert!(
        plan.wants
            .keys()
            .all(|posting| survey.builds_at(posting.asteroid())
                || survey.count(posting.asteroid(), posting.pattern()) > 0),
        "it wants {:?} where nothing of its own stands or builds",
        plan.wants
    );
}

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn a_decision_over_the_tick_s_cap_keeps_every_lowering_and_loses_only_its_lowest_raises() {
    let mut fixture = Fixture::drafted([None, Some(Personality::expand())]);
    let free = fixture.free(1)[0];
    fixture.want(0, free, P::Shipyard, 1);
    fixture.want(0, free, P::Frigate, 3);
    fixture.run(1);
    let view = fixture.view(0);
    let asking: Vec<Posting> = fixture
        .free(2 * MAX_COMMANDS_PER_TICK)
        .into_iter()
        .zip(damage_patterns(2 * MAX_COMMANDS_PER_TICK))
        .map(|(asteroid, pattern)| Posting::of(asteroid, SeatId(0), pattern))
        .collect();
    let plan = Plan {
        wants: asking
            .iter()
            .map(|posting| (*posting, MAX_WANT))
            .collect::<BTreeMap<Posting, u32>>(),
    };

    let commands = plan.commands(&view);

    assert!(
        asking.len() > MAX_COMMANDS_PER_TICK,
        "the plan asks for few"
    );
    assert_eq!(commands.len(), MAX_COMMANDS_PER_TICK);
    let lowered: Vec<Command> = view
        .plans
        .iter()
        .filter(|(_, plan)| plan.want > 0)
        .map(|(posting, _)| want(*posting, 0))
        .collect();
    assert!(!lowered.is_empty(), "the seat stood no want to lower");
    for command in &lowered {
        assert!(
            commands.contains(command),
            "a lowering was cut for a raise: {commands:?}"
        );
    }
    for (command, posting) in commands[lowered.len()..].iter().zip(asking) {
        assert_eq!(
            *command,
            want(posting, MAX_WANT),
            "the raises it kept are not the ones it ranked first"
        );
    }
}
