//! The one encoding two machines exchange values in.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Why bytes are not the value they were read as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed(String);

/// A value two machines exchange: CBOR, the same bytes on native and in the
/// browser.
pub trait Wire: DeserializeOwned + Serialize {
    /// This value as bytes.
    fn encoded(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // A `Vec` never fails to take a write, and every wire type is built
        // from integers, strings, sequences and maps, which all encode.
        ciborium::into_writer(self, &mut bytes).expect("a wire value encodes into a Vec");
        bytes
    }

    /// The value `bytes` hold, or why they are not one.
    fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        ciborium::from_reader(bytes).map_err(|why| Malformed(why.to_string()))
    }
}

impl<T: DeserializeOwned + Serialize> Wire for T {}

impl core::fmt::Display for Malformed {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        out.write_str(&self.0)
    }
}
