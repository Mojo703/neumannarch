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

    pub(crate) fn floor_meters(&self) -> f64 {
        self.radius() + Belt::SPACING_METERS
    }

    pub(crate) fn toward_shell(&self, body: Body, pos: Vec3) -> Vec3 {
        let out = pos - body.pos;
        let distance = out.length();
        let floor = self.floor_meters();
        let meters = match distance < floor {
            true => floor - distance,
            false => -(distance - Belt::ZONE_RADIUS_METERS).max(0.0),
        };
        out.normalized().map_or(Vec3::ZERO, |away| away * meters)
    }

    pub fn extract(&mut self, taken: Materials) {
        self.pull.fill(taken);
    }

    pub(crate) fn close_second(&mut self) {
        self.pull.close();
    }
}
