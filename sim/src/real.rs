//! The stored scalar.

use core::hash::{Hash, Hasher};

/// An `f64` held in state. Equal and hashed by bit pattern, so `-0.0` and
/// `0.0` differ and every state type derives `Hash`; arithmetic happens on
/// the `f64`.
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
