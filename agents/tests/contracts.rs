use neumannarch_agents::{Guarantees, minutes, shipped};

const CONTRACT_MINUTES: u64 = 5;

#[test]
#[ignore = "plays a match: cargo test -p neumannarch-agents --release -- --ignored"]
fn every_shipped_bot_builds_where_a_builder_stands_arms_early_and_spends_what_it_pulls() {
    for bot in shipped() {
        let watched = Guarantees::over(&[bot.bot, bot.bot], minutes(CONTRACT_MINUTES));

        assert_eq!(
            watched.every_unit_builds_where_a_builder_of_its_seat_stands(),
            None,
            "{}: it built a unit past a decision where no builder of its own stands",
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
