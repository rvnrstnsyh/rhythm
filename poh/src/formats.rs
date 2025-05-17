use hex::{decode, encode};
use serde::{Deserialize, Deserializer, Serializer, de::Error};

pub fn serialize<T: Serializer>(bytes: &[u8; 32], serializer: T) -> Result<T::Ok, T::Error> {
    return serializer.serialize_str(&encode(bytes));
}

pub fn deserialize<'a, T: Deserializer<'a>>(deserializer: T) -> Result<[u8; 32], T::Error> {
    let str: String = String::deserialize(deserializer)?;
    let bytes: Vec<u8> = decode(str).map_err(Error::custom)?;

    if bytes.len() != 32 {
        return Err(Error::custom(format!("Expected 32 bytes, got {}.", bytes.len())));
    }

    let mut arr: [u8; 32] = [0u8; 32];
    arr.copy_from_slice(&bytes);

    return Ok(arr);
}
