//! An asteroid: a fixed orbit and what it yields.

use crate::materials::Materials;
use crate::orbit::elements::Orbit;
use crate::real::Real;

/// A rock. It never thrusts, so its orbit holds for the whole match.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rock {
    orbit: Orbit,
    caps: Materials,
    radius: Real,
}

impl Rock {
    /// A rock on `orbit`, read at whatever epoch that orbit carries,
    /// yielding up to `caps` per second of each material, drawn `radius`
    /// meters across.
    pub fn new(orbit: Orbit, caps: Materials, radius: f64) -> Rock {
        Rock {
            orbit,
            caps,
            radius: Real(radius),
        }
    }

    /// The orbit the rock keeps.
    pub fn orbit(&self) -> &Orbit {
        &self.orbit
    }

    /// The most of each material extractable per second.
    pub fn caps(&self) -> Materials {
        self.caps
    }

    /// Visual radius in meters.
    pub fn radius(&self) -> f64 {
        self.radius.0
    }
}
