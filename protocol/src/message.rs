use crate::types::{Message, MessageBody};

use anyhow::{Context, Result};

impl Message {
    pub fn new(body: MessageBody) -> Self {
        return Self { body, nonce: rand::random() };
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        return serde_json::from_slice(bytes).context("Failed to deserialize message.");
    }

    pub fn to_vec(&self) -> Vec<u8> {
        return serde_json::to_vec(self).unwrap_or_default();
    }

    pub fn get_body(&self) -> &MessageBody {
        return &self.body;
    }
}
