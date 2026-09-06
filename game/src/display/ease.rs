use core::time::Duration;

pub const SPAN_SECONDS: f64 = 0.02;

pub fn toward(value: f64, target: f64, dt: f64) -> f64 {
    toward_over(value, target, dt, SPAN_SECONDS)
}

pub fn toward_over(value: f64, target: f64, dt: f64, span: f64) -> f64 {
    let share = (dt / span).clamp(0.0, 1.0);
    value + (target - value) * share
}

#[derive(Debug, Default)]
pub struct Clock {
    shown_at: Duration,
}

impl Clock {
    pub fn frame(&mut self, elapsed: Duration) -> f64 {
        let dt = elapsed.saturating_sub(self.shown_at).as_secs_f64();
        self.shown_at = elapsed;
        dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frames_span_is_the_time_the_game_has_run_since_the_last() {
        let mut clock = Clock::default();
        assert_eq!(clock.frame(Duration::from_millis(100)), 0.1);
        assert_eq!(
            clock.frame(Duration::from_millis(100)),
            0.0,
            "no time, no step"
        );
        assert_eq!(clock.frame(Duration::from_millis(125)), 0.025);
    }

    #[test]
    fn a_value_closes_the_spans_share_of_its_gap_and_never_overshoots() {
        assert_eq!(toward(0.0, 1.0, SPAN_SECONDS / 2.0), 0.5);
        assert_eq!(toward(0.0, 1.0, SPAN_SECONDS), 1.0);
        assert_eq!(
            toward(0.0, 1.0, SPAN_SECONDS * 4.0),
            1.0,
            "a slow frame lands"
        );
        assert_eq!(toward(1.0, 0.0, SPAN_SECONDS / 4.0), 0.75);
        assert_eq!(toward(3.0, 3.0, 0.016), 3.0, "a settled value holds");
        assert_eq!(
            toward_over(0.0, 1.0, 0.1, 0.4),
            0.25,
            "a longer span closes less"
        );
    }
}
