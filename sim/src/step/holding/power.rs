use crate::roster::Row;

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub(crate) struct Power(f64);

impl Power {
    pub(crate) fn of(row: &Row, hp: f64) -> Power {
        Power(row.dps_through(0.0) * hp)
    }

    pub(crate) fn get(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::Roster;
    use crate::roster::{CONSTRUCTOR, FRIGATE};

    #[test]
    fn power_is_damage_per_second_through_no_plating_times_remaining_hp() {
        let roster = Roster::shipped();
        let frigate = &roster[FRIGATE];
        assert_eq!(Power::of(frigate, 150.0).get(), 6.0 * 2.0 * 150.0);
        assert_eq!(Power::of(frigate, 75.0).get(), 6.0 * 2.0 * 75.0);
    }

    #[test]
    fn an_unarmed_row_has_no_power_at_any_health() {
        let roster = Roster::shipped();
        assert_eq!(Power::of(&roster[CONSTRUCTOR], 50.0), Power::default());
    }

    #[test]
    fn power_falls_with_the_hp_it_is_read_at() {
        let roster = Roster::shipped();
        let hurt = Power::of(&roster[FRIGATE], 10.0);
        assert!(hurt < Power::of(&roster[FRIGATE], 150.0));
        assert!(hurt > Power::default());
    }
}
