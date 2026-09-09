use neumannarch_agents::{Guarantees, minutes, shipped};

const CONTRACT_MINUTES: u64 = 4;

#[test]
fn every_shipped_bot_builds_where_a_builder_stands_arms_early_and_spends_what_it_pulls() {
    for bot in shipped() {
        let watched = Guarantees::over(&[bot.bot, bot.bot], minutes(CONTRACT_MINUTES));

        assert_eq!(
            watched.no_frame_outlives_a_decision_without_a_builder(),
            None,
            "{}: a frame outlived a decision with no builder",
            bot.name
        );
        assert_eq!(
            watched.both_sides_arm_and_trade_shots(),
            None,
            "{}: it did not arm and trade shots inside the clock",
            bot.name
        );
        assert_eq!(
            watched.no_stockpile_sits_full_with_builders_idle(),
            None,
            "{}: it hoarded with its builders idle",
            bot.name
        );
    }
}
