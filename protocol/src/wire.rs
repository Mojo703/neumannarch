use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed(String);

pub trait Codec: DeserializeOwned + Serialize {
    fn encoded(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        ciborium::into_writer(self, &mut bytes).expect("a wire value encodes into a Vec");
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        ciborium::from_reader(bytes).map_err(|why| Malformed(why.to_string()))
    }
}

impl<T: DeserializeOwned + Serialize> Codec for T {}

impl core::fmt::Display for Malformed {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        out.write_str(&self.0)
    }
}
