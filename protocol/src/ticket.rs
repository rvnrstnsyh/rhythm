use std::{fmt, str::FromStr};

use crate::types::Ticket;

use anyhow::{Context, Result};
use iroh::NodeAddr;
use iroh_gossip::proto::TopicId;

impl Ticket {
    pub fn new(topic: TopicId, nodes: Vec<NodeAddr>) -> Self {
        return Self { topic, nodes };
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        return serde_json::from_slice(bytes).context("Failed to deserialize ticket.");
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        return serde_json::to_vec(self).unwrap_or_default();
    }
}

impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "{:?}", data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]).make_ascii_lowercase());
    }
}

impl FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        return Self::from_bytes(
            &data_encoding::BASE32_NOPAD
                .decode(s.to_ascii_uppercase().as_bytes())
                .context("Failed to decode base32 string.")?,
        );
    }
}
