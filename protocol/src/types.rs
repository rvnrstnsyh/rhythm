use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use iroh::{Endpoint, NodeAddr, NodeId, protocol::Router};
use iroh_gossip::{
    net::{Gossip, GossipSender},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Protocol {
    pub endpoint: Endpoint,
    pub name: String,
    pub protocol: Gossip,
    pub router: Router,
    pub node_id: NodeId,
    pub tx: Option<GossipSender>,
    pub topic: Option<TopicId>,
    pub names: Arc<RwLock<HashMap<NodeId, String>>>,
    pub callback: Option<MessageCallback>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub body: MessageBody,
    pub nonce: [u8; 16],
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageBody {
    Ping { from: NodeId, name: String },
    Message { from: NodeId, text: String },
    Custom { from: NodeId, payload: Vec<u8> },
}

pub type MessageCallback = Arc<dyn Fn(Message) -> Result<()> + Send + Sync>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub topic: TopicId,
    pub nodes: Vec<NodeAddr>,
}
