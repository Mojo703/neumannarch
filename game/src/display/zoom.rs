const SHARPNESS_PER_POINT: f32 = 0.5;

pub fn floored(points: f32, floor: f32) -> f32 {
    floor + softplus(SHARPNESS_PER_POINT * (points - floor)) / SHARPNESS_PER_POINT
}

pub fn faded(points: f32, gone: f32, whole: f32) -> f32 {
    let along = (points.max(f32::MIN_POSITIVE).ln() - gone.ln()) / (whole.ln() - gone.ln());
    let along = along.clamp(0.0, 1.0);
    along * along * (3.0 - 2.0 * along)
}

fn softplus(over: f32) -> f32 {
    over.max(0.0) + (-over.abs()).exp().ln_1p()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: f32 = 12.0;

    #[test]
    fn a_floored_size_is_never_below_the_floor_nor_the_size_and_meets_each_far_from_the_other() {
        for points in [0.0, 0.1, 3.0, 11.0, 12.0, 13.0, 30.0, 400.0] {
            let drawn = floored(points, FLOOR);
            assert!(
                drawn >= FLOOR && drawn >= points,
                "{points} drew at {drawn}"
            );
        }
        assert!((floored(0.1, FLOOR) - FLOOR).abs() < 0.02);
        assert!((floored(400.0, FLOOR) - 400.0).abs() < 0.02);
    }

    #[test]
    fn a_fade_is_gone_below_its_lower_size_whole_above_its_upper_and_rises_between() {
        assert_eq!(faded(0.5, 1.0, 8.0), 0.0);
        assert_eq!(faded(1.0, 1.0, 8.0), 0.0);
        assert_eq!(faded(8.0, 1.0, 8.0), 1.0);
        assert_eq!(faded(50.0, 1.0, 8.0), 1.0);
        let mut last = 0.0;
        for step in 1..=16 {
            let now = faded(1.0 + 7.0 * step as f32 / 16.0, 1.0, 8.0);
            assert!(now >= last, "the fade fell from {last} to {now}");
            last = now;
        }
    }
}
