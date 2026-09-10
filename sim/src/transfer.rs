use crate::belt::Belt;
use crate::orbit::body::Body;
use crate::time::RunningSpan;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transfer {
    relative: Body,
    limit: f64,
}

impl Transfer {
    pub(crate) const ARRIVAL_POSITION_METERS: f64 = 0.25;

    pub(crate) const ARRIVAL_SPEED_METERS_PER_SECOND: f64 = 0.02;

    pub(crate) const APPROACH_MARGIN_METERS_PER_SECOND: f64 = 1.0;

    pub(crate) fn of(flier: Body, destination: Body, limit: f64) -> Transfer {
        let apart = flier.pos - destination.pos;
        let rim = apart
            .normalized()
            .map_or(Vec3::ZERO, |away| away * Belt::ZONE_RADIUS_METERS);
        Transfer {
            relative: Body::new(apart - rim, flier.vel - destination.vel),
            limit,
        }
    }

    pub(crate) fn thrust(self, over: RunningSpan) -> Vec3 {
        let seconds = over.seconds();
        let correction = self.wanted_velocity() - self.relative.vel;
        match correction
            .normalized()
            .filter(|_| correction.length() > self.limit * seconds)
        {
            Some(along) => along * self.limit,
            None => correction * (1.0 / seconds),
        }
    }

    pub(crate) fn arrived(self) -> bool {
        self.relative.pos.length() <= Transfer::ARRIVAL_POSITION_METERS
            && self.relative.vel.length() <= Transfer::ARRIVAL_SPEED_METERS_PER_SECOND
    }

    fn wanted_velocity(self) -> Vec3 {
        let Some(away) = self.relative.pos.normalized() else {
            return Vec3::ZERO;
        };
        let stopping = (2.0 * self.limit * self.relative.pos.length()).sqrt();
        away * -(stopping - Transfer::APPROACH_MARGIN_METERS_PER_SECOND).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Time;

    const LIMIT: f64 = 80.0;

    const APART: f64 = 2_000.0;

    fn from_rest(meters: f64) -> Transfer {
        Transfer::of(
            Body::new(Vec3::new(meters, 0.0, 0.0), Vec3::ZERO),
            Body::new(Vec3::ZERO, Vec3::ZERO),
            LIMIT,
        )
    }

    fn drifting(meters: f64, along_z: f64) -> Transfer {
        Transfer::of(
            Body::new(Vec3::new(meters, 0.0, 0.0), Vec3::new(0.0, 0.0, along_z)),
            Body::new(Vec3::ZERO, Vec3::ZERO),
            LIMIT,
        )
    }

    fn close(got: Vec3, wanted: Vec3) -> bool {
        got.distance(wanted) < 1e-9
    }

    fn a_tick() -> RunningSpan {
        RunningSpan::of(Time(1)).expect("a tick of match time")
    }

    #[test]
    fn a_flier_aims_at_the_rim_of_the_zone_on_its_own_side_and_arrives_there() {
        let aiming = from_rest(APART);
        assert!(
            (aiming.relative.pos.x - (APART - Belt::ZONE_RADIUS_METERS)).abs() < 1e-9,
            "{:?}",
            aiming.relative.pos
        );

        assert!(from_rest(Belt::ZONE_RADIUS_METERS).arrived());
        assert!(
            !from_rest(Belt::ZONE_RADIUS_METERS + 1.0).arrived(),
            "a metre outside the rim is not arrived"
        );
    }

    #[test]
    fn a_flier_at_rest_a_hop_away_thrusts_straight_at_its_destination_at_the_limit() {
        let thrust = from_rest(APART).thrust(a_tick());

        assert!(close(thrust, Vec3::new(-LIMIT, 0.0, 0.0)), "{thrust:?}");
    }

    #[test]
    fn the_speed_it_wants_is_what_the_limit_can_stop_from_less_the_margin() {
        let wanted = from_rest(APART).wanted_velocity();

        let stopping = (2.0 * LIMIT * (APART - Belt::ZONE_RADIUS_METERS)).sqrt();
        assert!(
            close(
                wanted,
                Vec3::new(
                    -(stopping - Transfer::APPROACH_MARGIN_METERS_PER_SECOND),
                    0.0,
                    0.0
                )
            ),
            "{wanted:?}"
        );
        assert_eq!(
            from_rest(0.0).wanted_velocity(),
            Vec3::ZERO,
            "on its destination it wants to stand still"
        );
    }

    #[test]
    fn a_flier_faster_than_it_wants_to_be_thrusts_against_the_excess() {
        let racing = Transfer::of(
            Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(-40.0, 0.0, 0.0)),
            Body::new(Vec3::ZERO, Vec3::ZERO),
            LIMIT,
        );

        let thrust = racing.thrust(a_tick());

        assert!(close(thrust, Vec3::new(LIMIT, 0.0, 0.0)), "{thrust:?}");
    }

    #[test]
    fn a_thrust_that_would_carry_it_past_the_velocity_it_wants_is_exactly_the_difference() {
        let creeping = drifting(0.0, LIMIT * Time(1).seconds() / 2.0);

        let reached = creeping.relative.vel + creeping.thrust(a_tick()) * Time(1).seconds();

        assert!(
            close(reached, creeping.wanted_velocity()),
            "a tick of thrust reached {reached:?}, not {:?}",
            creeping.wanted_velocity()
        );
        assert!(
            creeping.thrust(a_tick()).length() < LIMIT,
            "a thrust that cannot overshoot is below the limit"
        );
    }

    #[test]
    fn no_thrust_of_a_transfer_exceeds_the_movement_limit() {
        let states = [
            from_rest(APART),
            from_rest(0.0),
            drifting(1.0, 300.0),
            drifting(0.0, 0.0),
            drifting(1e-9, LIMIT * Time(1).seconds()),
        ];

        for transfer in states {
            let asked = transfer.thrust(a_tick()).length();
            assert!(asked <= LIMIT + 1e-12, "{asked} against {LIMIT}");
        }
    }

    #[test]
    fn a_flier_at_rest_on_its_destination_asks_for_nothing_and_has_arrived() {
        let landed = from_rest(0.0);

        assert_eq!(landed.thrust(a_tick()), Vec3::ZERO);
        assert!(landed.arrived());
        assert!(!from_rest(APART).arrived(), "a whole hop away");
        assert!(
            !drifting(0.0, 1.0).arrived(),
            "on the destination but not at its speed"
        );
    }
}
