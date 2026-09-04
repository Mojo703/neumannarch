use crate::materials::Materials;
use crate::orbit::elements::Orbit;
use crate::real::Real;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rock {
    orbit: Orbit,
    caps: Materials,
    radius: Real,
}

impl Rock {
    pub fn new(orbit: Orbit, caps: Materials, radius: f64) -> Rock {
        Rock {
            orbit,
            caps,
            radius: Real(radius),
        }
    }

    pub fn orbit(&self) -> &Orbit {
        &self.orbit
    }

    pub fn caps(&self) -> Materials {
        self.caps
    }

    pub fn radius(&self) -> f64 {
        self.radius.0
    }
}
