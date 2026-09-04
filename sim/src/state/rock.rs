use crate::belt::Belt;
use crate::materials::Materials;
use crate::orbit::body::Body;
use crate::orbit::elements::Orbit;
use crate::real::Real;
use crate::vec3::Vec3;

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

    pub fn strayed(&self, body: Body, pos: Vec3) -> f64 {
        let distance = body.pos.distance(pos);
        let floor = self.radius() + Belt::SPACING_METERS;
        (distance - Belt::ZONE_RADIUS_METERS).max(0.0) - (floor - distance).max(0.0)
    }
}
