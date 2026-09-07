const SERIES_BOUND: f64 = 1.0;

const SERIES_TERMS: u32 = 10;

pub(crate) fn c2(z: f64) -> f64 {
    if z.abs() < SERIES_BOUND {
        series(z, 2)
    } else if z > 0.0 {
        (1.0 - libm::cos(z.sqrt())) / z
    } else {
        (libm::cosh((-z).sqrt()) - 1.0) / -z
    }
}

pub(crate) fn c3(z: f64) -> f64 {
    if z.abs() < SERIES_BOUND {
        series(z, 3)
    } else if z > 0.0 {
        let root = z.sqrt();
        (root - libm::sin(root)) / (z * root)
    } else {
        let root = (-z).sqrt();
        (libm::sinh(root) - root) / (-z * root)
    }
}

fn series(z: f64, first: u32) -> f64 {
    let mut term = 1.0 / f64::from((1..=first).product::<u32>());
    let mut sum = term;
    for k in 1..SERIES_TERMS {
        let n = first + 2 * k;
        term *= -z / f64::from((n - 1) * n);
        sum += term;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed_c2(z: f64) -> f64 {
        if z > 0.0 {
            (1.0 - libm::cos(z.sqrt())) / z
        } else {
            (libm::cosh((-z).sqrt()) - 1.0) / -z
        }
    }

    fn closed_c3(z: f64) -> f64 {
        if z > 0.0 {
            (z.sqrt() - libm::sin(z.sqrt())) / libm::pow(z, 1.5)
        } else {
            (libm::sinh((-z).sqrt()) - (-z).sqrt()) / libm::pow(-z, 1.5)
        }
    }

    #[test]
    fn at_zero_the_values_are_one_half_and_one_sixth() {
        assert_eq!(c2(0.0), 0.5);
        assert!((c3(0.0) - 1.0 / 6.0).abs() < 1e-16);
    }

    #[test]
    fn matches_the_closed_forms_away_from_zero() {
        for z in [1.0, -1.0, 10.0, -10.0, 0.5, -0.5] {
            assert!((c2(z) - closed_c2(z)).abs() < 1e-12, "c2({z})");
            assert!((c3(z) - closed_c3(z)).abs() < 1e-12, "c3({z})");
        }
    }

    #[test]
    fn continuous_across_the_series_switch() {
        for bound in [SERIES_BOUND, -SERIES_BOUND] {
            let below = bound * (1.0 - 1e-12);
            assert!((c2(below) - c2(bound)).abs() < 1e-12);
            assert!((c3(below) - c3(bound)).abs() < 1e-12);
        }
    }
}
