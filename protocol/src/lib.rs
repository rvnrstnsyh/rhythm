use std::{collections::HashMap, fmt, str::FromStr};

use anyhow::Result;
use futures_lite::StreamExt;
use iroh::{Endpoint, NodeAddr, NodeId, PublicKey, SecretKey, protocol::Router};
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver, GossipSender},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    body: MessageBody,
    nonce: [u8; 16],
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageBody {
    Origin { from: NodeId, name: String },
    Message { from: NodeId, text: String },
    Custom { from: NodeId, payload: Vec<u8> },
}

impl Message {
    pub fn new(body: MessageBody) -> Self {
        return Self { body, nonce: rand::random() };
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        return serde_json::from_slice(bytes).map_err(Into::into);
    }

    pub fn to_vec(&self) -> Vec<u8> {
        return serde_json::to_vec(self).expect("serde_json::to_vec is infallible.");
    }

    pub fn get_body(&self) -> &MessageBody {
        return &self.body;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub topic: TopicId,
    pub nodes: Vec<NodeAddr>,
}

impl Ticket {
    pub fn new(topic: TopicId, nodes: Vec<NodeAddr>) -> Self {
        return Self { topic, nodes };
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        return serde_json::from_slice(bytes).map_err(Into::into);
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        return serde_json::to_vec(self).expect("serde_json::to_vec is infallible.");
    }
}

impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut text: String = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        return write!(f, "{}", text);
    }
}

impl FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes: Vec<u8> = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        return Self::from_bytes(&bytes);
    }
}

// Callback function type for message handling.
pub type MessageCallback = Arc<dyn Fn(Message) -> Result<()> + Send + Sync>;

pub struct Protocol {
    endpoint: Endpoint,
    name: String,
    protocol: Gossip,
    router: Router,
    node_id: NodeId,
    tx: Option<GossipSender>,
    topic: Option<TopicId>,
    names: Arc<RwLock<HashMap<NodeId, String>>>,
    callback: Option<MessageCallback>,
}

impl Protocol {
    pub async fn new(secret_key: Option<SecretKey>, callback: Option<MessageCallback>) -> Result<Self> {
        let endpoint: Endpoint = Endpoint::builder()
            .secret_key(secret_key.unwrap_or_else(|| SecretKey::from_bytes(&rand::random())))
            .discovery_n0()
            .bind()
            .await?;
        let protocol: Gossip = Gossip::builder().spawn(endpoint.clone()).await?;

        return Ok(Self {
            endpoint: endpoint.clone(),
            name: "Unknown".to_string(),
            protocol: protocol.clone(),
            router: Router::builder(endpoint.clone()).accept(iroh_gossip::ALPN, protocol.clone()).spawn(),
            node_id: endpoint.node_id(),
            tx: None,
            topic: None,
            names: Arc::new(RwLock::new(HashMap::new())),
            callback,
        });
    }

    pub async fn listen(&mut self) -> Result<Ticket> {
        let topic: TopicId = TopicId::from_bytes(rand::random());
        let (tx, rx) = self.protocol.subscribe_and_join(topic, vec![]).await?.split();

        self.tx = Some(tx);
        self.topic = Some(topic);

        // Start the message handling loop.
        self.receiver(rx);

        {
            let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
            names.insert(self.node_id, self.name.clone());
        }

        // Automatically broadcast name.
        if let Some(tx) = &self.tx {
            let message = Message::new(MessageBody::Origin {
                from: self.node_id,
                name: self.name.clone(),
            });
            tx.broadcast(message.to_vec().into()).await?;
        }

        // Create and return the ticket.
        let current_node_addr: NodeAddr = self.endpoint.node_addr().await?;
        let nodes: Vec<NodeAddr> = vec![current_node_addr];

        return Ok(Ticket { topic, nodes });
    }

    pub async fn dial(&mut self, ticket: &str) -> Result<()> {
        let Ticket { topic, nodes } = Ticket::from_str(ticket)?;

        // Add the peer addresses to the endpoint's address book.
        for node in nodes.iter() {
            self.endpoint.add_node_addr(node.clone())?;
        }

        let node_ids: Vec<PublicKey> = nodes.iter().map(|peer| peer.node_id).collect();
        let (tx, rx) = self.protocol.subscribe_and_join(topic, node_ids).await?.split();

        self.tx = Some(tx);
        self.topic = Some(topic);
        self.receiver(rx);

        {
            let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
            names.insert(self.node_id, self.name.clone());
        }

        if let Some(tx) = &self.tx {
            let message: Message = Message::new(MessageBody::Origin {
                from: self.node_id,
                name: self.name.clone(),
            });
            tx.broadcast(message.to_vec().into()).await?;
        }
        return Ok(());
    }

    pub async fn send_text_message(&self, text: String) -> Result<()> {
        if let Some(tx) = &self.tx {
            if self.topic.is_some() {
                let message = Message::new(MessageBody::Message { from: self.node_id, text });
                tx.broadcast(message.to_vec().into()).await?;
                return Ok(());
            }
        }
        return Err(anyhow::anyhow!("Not connected to a chat room."));
    }

    pub async fn send_custom_message(&self, payload: Vec<u8>) -> Result<()> {
        if let Some(tx) = &self.tx {
            if self.topic.is_some() {
                let message: Message = Message::new(MessageBody::Custom { from: self.node_id, payload });
                tx.broadcast(message.to_vec().into()).await?;
                return Ok(());
            }
        }
        return Err(anyhow::anyhow!("Not connected to a chat room."));
    }

    pub async fn set_node_name(&mut self, name: Option<String>) -> Result<()> {
        let name_to_use: String = name.unwrap_or_else(|| self.name.clone());

        self.name = name_to_use.clone();

        if let Some(tx) = &self.tx {
            if let Some(_topic) = self.topic {
                {
                    let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
                    names.insert(self.node_id, name_to_use.clone());
                }
                // Broadcast name to others.
                let message: Message = Message::new(MessageBody::Origin {
                    from: self.node_id,
                    name: name_to_use,
                });
                tx.broadcast(message.to_vec().into()).await?;
            }
        }
        return Ok(());
    }

    pub async fn get_node_name(&self, node_id: &NodeId) -> String {
        let names: RwLockReadGuard<'_, HashMap<PublicKey, String>> = self.names.read().await;
        return names.get(node_id).cloned().unwrap_or_else(|| node_id.fmt_short());
    }

    pub fn get_node_id(&self) -> NodeId {
        return self.node_id;
    }

    pub fn get_current_node_topic(&self) -> Option<TopicId> {
        return self.topic;
    }

    pub async fn shutdown(self) -> Result<()> {
        self.router.shutdown().await?;
        return Ok(());
    }

    fn receiver(&self, mut rx: GossipReceiver) {
        let names: Arc<RwLock<HashMap<PublicKey, String>>> = self.names.clone();
        let callback: Option<MessageCallback> = self.callback.clone();

        tokio::spawn(async move {
            while let Ok(Some(event)) = rx.try_next().await {
                if let Event::Gossip(GossipEvent::Received(msg)) = event {
                    if let Ok(message) = Message::from_bytes(&msg.content) {
                        // Process the message based on its type.
                        match &message.body {
                            MessageBody::Origin { from, name } => {
                                let mut names_write: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = names.write().await;
                                names_write.insert(*from, name.clone());
                            }
                            MessageBody::Message { from, text } => {
                                println!("{}: {}", from, text);
                            }
                            _ => {}
                        }
                        // Call the user-provided callback if it exists.
                        if let Some(cb) = &callback {
                            let _ = cb(message);
                        }
                    }
                }
            }
        });
    }
}
