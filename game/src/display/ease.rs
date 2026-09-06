use core::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Fast,
    Slow,
}

impl Span {
    pub fn duration(self) -> Duration {
        match self {
            Span::Fast => Duration::from_millis(20),
            Span::Slow => Duration::from_millis(100),
        }
    }

    pub fn seconds(self) -> f64 {
        self.duration().as_secs_f64()
    }
}

pub fn toward(value: f64, target: f64, dt: f64, span: Span) -> f64 {
    let share = (dt / span.seconds()).clamp(0.0, 1.0);
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
        let fast = Span::Fast.seconds();
        assert_eq!(toward(0.0, 1.0, fast / 2.0, Span::Fast), 0.5);
        assert_eq!(toward(0.0, 1.0, fast, Span::Fast), 1.0);
        assert_eq!(
            toward(0.0, 1.0, fast * 4.0, Span::Fast),
            1.0,
            "a slow frame lands"
        );
        assert_eq!(toward(1.0, 0.0, fast / 4.0, Span::Fast), 0.75);
        assert_eq!(
            toward(3.0, 3.0, 0.016, Span::Fast),
            3.0,
            "a settled value holds"
        );
    }

    #[test]
    fn the_slow_span_is_five_fast_ones() {
        assert_eq!(Span::Fast.duration(), Duration::from_millis(20));
        assert_eq!(Span::Slow.duration(), Duration::from_millis(100));
        assert!((toward(0.0, 1.0, Span::Fast.seconds(), Span::Slow) - 0.2).abs() < 1e-9);
    }
}
