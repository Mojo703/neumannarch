use crate::belt::Belt;
use crate::materials::{Materials, PerSecond};
use crate::orbit::body::Body;
use crate::orbit::elements::Orbit;
use crate::real::Real;
use crate::vec3::Vec3;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Asteroid {
    orbit: Orbit,
    caps: Materials,
    radius: Real,
    pull: PerSecond,
}

impl Asteroid {
    pub fn new(orbit: Orbit, caps: Materials, radius: f64) -> Asteroid {
        Asteroid {
            orbit,
            caps,
            radius: Real(radius),
            pull: PerSecond::default(),
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

    pub fn pull(&self) -> Materials {
        self.pull.completed()
    }

    pub fn strayed(&self, body: Body, pos: Vec3) -> f64 {
        let distance = body.pos.distance(pos);
        let floor = self.radius() + Belt::SPACING_METERS;
        (distance - Belt::ZONE_RADIUS_METERS).max(0.0) - (floor - distance).max(0.0)
    }

    pub(crate) fn extract(&mut self, taken: Materials) {
        self.pull.fill(taken);
    }

    pub(crate) fn close_second(&mut self) {
        self.pull.close();
    }
}
