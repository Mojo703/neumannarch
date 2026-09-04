use core::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, Default)]
pub struct Real(pub f64);

impl Eq for Real {}

impl From<f64> for Real {
    fn from(value: f64) -> Self {
        Real(value)
    }
}

impl Hash for Real {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl PartialEq for Real {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
