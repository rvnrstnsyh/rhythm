use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::types::{Message, MessageBody, MessageCallback, Protocol, Ticket};

use anyhow::{Context, Result};
use futures_lite::StreamExt;
use iroh::{Endpoint, NodeAddr, NodeId, PublicKey, SecretKey, protocol::Router};
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver},
    proto::TopicId,
};
use tokio::sync::{RwLock, RwLockWriteGuard};

impl Protocol {
    pub async fn new(secret_key: Option<SecretKey>, callback: Option<MessageCallback>) -> Result<Self> {
        let secret_key: SecretKey = secret_key.unwrap_or_else(|| {
            let mut random_bytes: [u8; 32] = [0u8; 32];
            rand::Rng::fill(&mut rand::rng(), &mut random_bytes);
            return SecretKey::from_bytes(&random_bytes);
        });
        let endpoint: Endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .bind()
            .await
            .context("Failed to create endpoint.")?;
        let protocol: Gossip = Gossip::builder().spawn(endpoint.clone()).await.context("Failed to spawn gossip protocol.")?;
        let router: Router = Router::builder(endpoint.clone()).accept(iroh_gossip::ALPN, protocol.clone()).spawn();

        return Ok(Self {
            endpoint: endpoint.clone(),
            name: "Unknown".to_string(),
            protocol,
            router,
            node_id: endpoint.node_id(),
            tx: None,
            topic: None,
            names: Arc::new(RwLock::new(HashMap::new())),
            callback,
        });
    }

    pub async fn listen(&mut self) -> Result<Ticket> {
        let topic: TopicId = TopicId::from_bytes(rand::random());
        let (tx, rx) = self
            .protocol
            .subscribe_and_join(topic, vec![])
            .await
            .context("Failed to subscribe to topic.")?
            .split();

        self.tx = Some(tx);
        self.topic = Some(topic);
        // Start the message handling loop in a separate task.
        self.receiver(rx);

        {
            let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
            names.insert(self.node_id, self.name.clone());
        }

        // Automatically broadcast name.
        if let Some(tx) = &self.tx {
            let message: Message = Message::new(MessageBody::Ping {
                from: self.node_id,
                name: self.name.clone(),
            });
            tx.broadcast(message.to_vec().into()).await.context("Failed to broadcast ping message.")?;
        }

        // Create and return the invitation ticket.
        let current_node_addr: NodeAddr = self.endpoint.node_addr().await.context("Failed to get node address.")?;
        let nodes: Vec<NodeAddr> = vec![current_node_addr];

        return Ok(Ticket { topic, nodes });
    }

    pub async fn dial(&mut self, ticket: &str) -> Result<()> {
        let Ticket { topic, nodes } = Ticket::from_str(ticket).context("Failed to parse ticket.")?;

        // Add all peer addresses to the endpoint's address book.
        for node in &nodes {
            self.endpoint.add_node_addr(node.clone()).context("Failed to add node address.")?;
        }

        // Collect node IDs from addresses.
        let node_ids: Vec<PublicKey> = nodes.iter().map(|peer| peer.node_id).collect();
        // Subscribe to the topic and join the network.
        let (tx, rx) = self
            .protocol
            .subscribe_and_join(topic, node_ids)
            .await
            .context("Failed to subscribe and join.")?
            .split();

        self.tx = Some(tx);
        self.topic = Some(topic);
        self.receiver(rx);

        {
            let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
            names.insert(self.node_id, self.name.clone());
        }

        if let Some(tx) = &self.tx {
            let message: Message = Message::new(MessageBody::Ping {
                from: self.node_id,
                name: self.name.clone(),
            });
            tx.broadcast(message.to_vec().into()).await.context("Failed to broadcast ping message.")?;
        }
        return Ok(());
    }

    pub async fn broadcast(&self, text: String) -> Result<()> {
        if let (Some(tx), Some(_topic)) = (&self.tx, &self.topic) {
            tx.broadcast(Message::new(MessageBody::Message { from: self.node_id, text }).to_vec().into())
                .await
                .context("Failed to broadcast text message.")?;
            return Ok(());
        } else {
            return Err(anyhow::anyhow!("Not connected to a chat room."));
        }
    }

    pub async fn custom_broadcast(&self, payload: Vec<u8>) -> Result<()> {
        if let (Some(tx), Some(_topic)) = (&self.tx, &self.topic) {
            tx.broadcast(Message::new(MessageBody::Custom { from: self.node_id, payload }).to_vec().into())
                .await
                .context("Failed to broadcast custom message.")?;
            return Ok(());
        } else {
            return Err(anyhow::anyhow!("Not connected to a chat room."));
        }
    }

    pub async fn set_node_name(&mut self, name: Option<String>) -> Result<()> {
        let name_to_use: String = name.unwrap_or_else(|| return self.name.clone());

        self.name = name_to_use.clone();

        if let (Some(tx), Some(_topic)) = (&self.tx, &self.topic) {
            {
                let mut names: RwLockWriteGuard<'_, HashMap<PublicKey, String>> = self.names.write().await;
                names.insert(self.node_id, name_to_use.clone());
            }

            let message: Message = Message::new(MessageBody::Ping {
                from: self.node_id,
                name: name_to_use,
            });
            tx.broadcast(message.to_vec().into()).await.context("Failed to broadcast name change.")?;
        }
        return Ok(());
    }

    pub async fn get_node_name(&self, node_id: &NodeId) -> String {
        return self.names.read().await.get(node_id).cloned().unwrap_or_else(|| return node_id.fmt_short());
    }

    pub fn get_node_id(&self) -> NodeId {
        return self.node_id;
    }

    pub fn get_current_node_topic(&self) -> Option<TopicId> {
        return self.topic;
    }

    pub async fn shutdown(self) -> Result<()> {
        return self.router.shutdown().await.context("Failed to shut down router.");
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
                            MessageBody::Ping { from, name } => {
                                names.write().await.insert(*from, name.clone());
                            }
                            MessageBody::Message { from, text } => {
                                println!("{}: {}", from, text);
                            }
                            MessageBody::Custom { .. } => {}
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
