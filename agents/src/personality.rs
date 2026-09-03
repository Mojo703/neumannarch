//! The constants one scripted agent plays by, and the two the game ships.

use probe_protocol::Bot;
use probe_sim::RowId;
use probe_sim::roster::Roster;

use crate::roles::Roles;

/// How many hit points a point of plating is worth when a personality
/// weighs durability, in HP per point. A hypothesis.
const PLATING_WORTH: f64 = 20.0;

/// How an agent chooses its army mix.
#[derive(Clone, Debug, PartialEq)]
pub enum Mix {
    /// The mix the roster's own ratios pick against what the enemy fields.
    Counters,
    /// A weight per row, fixed: what the harness's matrix plays.
    Pinned(Vec<(RowId, f64)>),
}

/// One way of playing: every number an agent decides by, with the unit it
/// is in. Each is a hypothesis the harness confirms or kills.
#[derive(Clone, Debug, PartialEq)]
pub struct Personality {
    /// What the harness prints it as.
    pub name: &'static str,
    /// The seed its dice start from, so two alike agents differ.
    pub seed: u64,
    /// Rocks it tries to hold.
    pub rocks: usize,
    /// Extractors it wants per rock, before the rock's caps cut it.
    pub extractors_per_rock: u32,
    /// Stores it wants at its home rock.
    pub stores: u32,
    /// Rocks that get a fast builder, its home first.
    pub yards: usize,
    /// Mobile builders it keeps.
    pub masons: u32,
    /// Scouts it keeps walking unseen rocks.
    pub scouts: u32,
    /// Expansions it will have in flight at once.
    pub claims: usize,
    /// Army value it wants, as a multiple of the enemy value it estimates.
    pub army_ratio: f64,
    /// Army value it wants regardless of the enemy, in cost units.
    pub army_floor: f64,
    /// Army value it posts at a threatened rock, per cost unit of threat.
    pub defence_ratio: f64,
    /// Its army value over the enemy estimate before it stages an attack.
    pub attack_ratio: f64,
    /// Staged value over the target's threat before the force moves in.
    pub commit_ratio: f64,
    /// How its army mix is chosen.
    pub mix: Mix,
    /// How much it pays for reach past the enemy's, per unit of edge.
    pub range_taste: f64,
    /// How much it pays for durability, per hit point of cost unit.
    pub armour_taste: f64,
}

impl Personality {
    /// Holds few rocks, saturates them, and keeps a heavy defensive army.
    pub fn turtle() -> Personality {
        Personality {
            name: "turtle",
            seed: 0x7075_7274_6c65,
            rocks: 3,
            extractors_per_rock: 4,
            stores: 2,
            yards: 1,
            masons: 1,
            scouts: 1,
            claims: 1,
            army_ratio: 1.3,
            army_floor: 150.0,
            defence_ratio: 2.0,
            attack_ratio: 2.5,
            commit_ratio: 2.0,
            mix: Mix::Counters,
            range_taste: 0.5,
            armour_taste: 1.0,
        }
    }

    /// Claims rocks fast, spreads its economy thin, and commits early.
    pub fn expand() -> Personality {
        Personality {
            name: "expand",
            seed: 0x6578_7061_6e64,
            rocks: 8,
            extractors_per_rock: 2,
            stores: 1,
            yards: 3,
            masons: 3,
            scouts: 2,
            claims: 2,
            army_ratio: 0.9,
            army_floor: 90.0,
            defence_ratio: 1.2,
            attack_ratio: 1.2,
            commit_ratio: 1.2,
            mix: Mix::Counters,
            range_taste: 0.0,
            armour_taste: 0.0,
        }
    }

    /// The constants a lobby's `bot` plays by. Exhaustive, so a scripted
    /// opponent the protocol can name always has a way of playing.
    pub fn of(bot: Bot) -> Personality {
        match bot {
            Bot::Turtle => Personality::turtle(),
            Bot::Expand => Personality::expand(),
        }
    }

    /// The personality of that name, or `None`: what a command line takes.
    pub fn named(name: &str) -> Option<Personality> {
        [Personality::turtle(), Personality::expand()]
            .into_iter()
            .find(|personality| personality.name == name)
    }

    /// The share of army value each armed row is worth having against an
    /// enemy of `plating` damage reduction and `range` meters of reach, in
    /// row order, the shares summing to one. Empty only when no row is
    /// armed.
    pub fn weights(
        &self,
        roster: &Roster,
        roles: &Roles,
        plating: f64,
        range: f64,
    ) -> Vec<(RowId, f64)> {
        let scored = match &self.mix {
            Mix::Pinned(pinned) => pinned
                .iter()
                .filter(|(row, _)| roles.army.contains(row))
                .map(|(row, weight)| (*row, weight.max(0.0)))
                .collect(),
            Mix::Counters => self.counters(roster, roles, plating, range),
        };
        share(scored)
    }

    /// Each armed row rated by damage through `plating` per cost unit,
    /// skewed by this personality's taste for reach and for durability. A
    /// row that cannot hurt `plating` at all is rated as if it were
    /// unplated, since an army of nothing is worse than a poor answer.
    fn counters(
        &self,
        roster: &Roster,
        roles: &Roles,
        plating: f64,
        range: f64,
    ) -> Vec<(RowId, f64)> {
        let rated = |through: f64| -> Vec<(RowId, f64)> {
            roles
                .army
                .iter()
                .filter_map(|id| roster.get(*id).map(|row| (*id, row)))
                .map(|(id, row)| {
                    let cost = row.cost.total().max(f64::MIN_POSITIVE);
                    let reach = row.max_damage_range();
                    let edge = (reach - range) / reach.max(range).max(1.0);
                    let durability = (row.hp.0 + PLATING_WORTH * row.plating.0) / cost;
                    let rate = row.dps_through(through) / cost;
                    (
                        id,
                        rate * (1.0 + self.range_taste * edge).max(0.0)
                            * (1.0 + self.armour_taste * durability),
                    )
                })
                .collect()
        };
        let against = rated(plating);
        if against.iter().any(|(_, weight)| *weight > 0.0) {
            against
        } else {
            rated(0.0)
        }
    }
}

/// `scored` scaled to sum to one, or every entry at an equal share when
/// nothing scored above zero.
fn share(scored: Vec<(RowId, f64)>) -> Vec<(RowId, f64)> {
    let total: f64 = scored.iter().map(|(_, weight)| weight).sum();
    let count = scored.len() as f64;
    scored
        .into_iter()
        .map(|(row, weight)| match total > 0.0 {
            true => (row, weight / total),
            false => (row, 1.0 / count),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use probe_sim::roster::{FRIGATE, LANCER, RAIDER};

    fn weights(personality: &Personality, plating: f64, range: f64) -> Vec<(RowId, f64)> {
        let roster = Roster::shipped();
        personality.weights(&roster, &Roles::of(&roster), plating, range)
    }

    fn of(weights: &[(RowId, f64)], row: RowId) -> f64 {
        weights
            .iter()
            .find(|(id, _)| *id == row)
            .map_or(0.0, |(_, weight)| *weight)
    }

    #[test]
    fn every_mix_is_a_set_of_shares_over_the_armed_rows() {
        for personality in [Personality::turtle(), Personality::expand()] {
            let weights = weights(&personality, 0.0, 0.0);
            assert_eq!(weights.len(), 3);
            assert!((weights.iter().map(|(_, share)| share).sum::<f64>() - 1.0).abs() < 1e-12);
            assert!(weights.iter().all(|(_, share)| *share > 0.0));
        }
    }

    #[test]
    fn plating_shifts_the_mix_off_the_row_it_blunts() {
        let personality = Personality::expand();
        let unplated = weights(&personality, 0.0, 0.0);
        let plated = weights(&personality, 2.0, 0.0);
        assert!(
            of(&plated, RAIDER) < of(&unplated, RAIDER),
            "the fastest, weakest hit loses most to plating"
        );
        assert!(of(&plated, LANCER) > of(&unplated, LANCER));
    }

    #[test]
    fn a_taste_for_reach_shifts_the_mix_toward_the_longer_row() {
        let mut personality = Personality::expand();
        let flat = weights(&personality, 0.0, 6.0);
        personality.range_taste = 1.0;
        let keen = weights(&personality, 0.0, 6.0);
        assert!(of(&keen, LANCER) > of(&flat, LANCER));
        assert!(of(&keen, RAIDER) < of(&flat, RAIDER));
    }

    #[test]
    fn a_taste_for_durability_shifts_the_mix_toward_the_plated_row() {
        let mut personality = Personality::expand();
        let flat = weights(&personality, 0.0, 0.0);
        personality.armour_taste = 2.0;
        let tough = weights(&personality, 0.0, 0.0);
        assert!(of(&tough, FRIGATE) > of(&flat, FRIGATE));
    }

    #[test]
    fn a_pinned_mix_plays_only_the_rows_it_names() {
        let personality = Personality {
            mix: Mix::Pinned(vec![(FRIGATE, 1.0)]),
            ..Personality::turtle()
        };

        let weights = weights(&personality, 0.0, 0.0);

        assert_eq!(weights, vec![(FRIGATE, 1.0)]);
    }

    #[test]
    fn a_personality_is_found_by_the_name_it_prints_as() {
        assert_eq!(Personality::named("turtle"), Some(Personality::turtle()));
        assert_eq!(Personality::named("expand"), Some(Personality::expand()));
        assert_eq!(Personality::named("scripted"), None);
    }

    #[test]
    fn every_bot_a_lobby_can_seat_plays_by_the_personality_of_its_own_name() {
        for bot in [Bot::Turtle, Bot::Expand] {
            let personality = Personality::of(bot);
            assert_eq!(
                Personality::named(personality.name),
                Some(personality.clone()),
                "{bot:?} plays by no shipped personality"
            );
        }
    }
}
