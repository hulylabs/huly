//

use crate::db::Db;
use crate::id::{AccId, NodeId, OrgId};
use crate::message::{Message, SignedMessage, SignedMessageType, Timestamp};
use anyhow::Result;
use futures_lite::future::Boxed as BoxedFuture;
use futures_lite::StreamExt;
use iroh::endpoint::{get_remote_node_id, Connecting};
use iroh::protocol::ProtocolHandler;
use iroh::Endpoint;
use iroh_gossip::net::GossipSender;
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Membership {
    db: Db,
    endpoint: Endpoint,
    gossip: Gossip,
}

impl Membership {
    pub const ALPN: &[u8] = b"huly/membership/0";

    pub fn new(db: Db, endpoint: Endpoint, gossip: Gossip) -> Arc<Self> {
        Arc::new(Self {
            db,
            endpoint,
            gossip,
        })
    }
}

pub const MAX_MESSAGE_SIZE: usize = 4096;

async fn account_loop(mut sender: GossipSender, mut receiver: GossipReceiver) -> Result<()> {
    while let Some(event) = receiver.try_next().await? {
        if let Event::Gossip(GossipEvent::Received(msg)) = event {

            // let (from, message) = SignedMessage::verify_and_decode(&msg.content)?;
            // match message {
            //     Message::AboutMe { name } => {
            //         names.insert(from, name.clone());
            //         println!("> {} is now known as {}", from.fmt_short(), name);
            //     }
            //     Message::Message { text } => {
            //         let name = names
            //             .get(&from)
            //             .map_or_else(|| from.fmt_short(), String::to_string);
            //         println!("{}: {}", name, text);
            //     }
            // }
        }
    }
    Ok(())
}

impl ProtocolHandler for Membership {
    /// The returned future runs on a newly spawned tokio task, so it can run as long as
    /// the connection lasts.
    fn accept(&self, connecting: Connecting) -> BoxedFuture<Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            let connection = connecting.await?;
            let device_id = get_remote_node_id(&connection)?;
            println!("accepted connection from {device_id}");

            let account_id = this
                .db
                .get_device_account(device_id.into())?
                .ok_or(anyhow::anyhow!("unknown account"))?;

            println!("authenticated as {}", account_id);

            // fetch account's organizations

            let topic = TopicId::from_bytes(account_id.into());

            println!("subscribing");
            let (sender, receiver) = this.gossip.subscribe_and_join(topic, vec![]).await?.split();

            println!("spawning account loop");
            let x = tokio::spawn(account_loop(sender, receiver));

            println!("spawned");
            let (mut send, mut recv) = connection.accept_bi().await?;
            loop {
                let message = Message::read_async(&mut recv).await?;
                match message.get_type_unwrap() {
                    MembershipRequestType::TAG => {
                        let signed = SignedMessageType::decode(&message)?;
                        if signed.verify()? != device_id.into() {
                            anyhow::bail!("message must be signed by the device");
                        }

                        // let message = Message::decode(signed.get_payload().as_bytes())?;
                        let request = MembershipRequestType::decode(signed.get_message())?;
                        let device = request.device_ownership.device;
                        let account = request.device_ownership.account;

                        this.db.insert_device_account(device, account)?;
                        println!("added device `{}` to account `{}`", device, account);

                        let response = MembershipResponse::new(true, None);
                        let encoded = MembershipResponseType::encode(&response)?;
                        let signed = SignedMessage::sign(&this.endpoint.secret_key(), encoded)?;
                        let message = SignedMessageType::encode(&signed)?;
                        message.write_async(&mut send).await?;
                    }
                    _ => anyhow::bail!("unknown message type"),
                }
            }
        })
    }
}

//

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceOwnership {
    account: AccId,
    device: NodeId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipRequest {
    device_ownership: DeviceOwnership,
    org: OrgId,
}

impl MembershipRequest {
    pub fn new(device: NodeId, account: AccId, org: OrgId) -> Self {
        Self {
            device_ownership: DeviceOwnership { account, device },
            org,
        }
    }
}

pub type MembershipRequestType = crate::message::MessageType<MembershipRequest, 0x483A130AB92F3040>;

//

#[derive(Serialize, Deserialize)]
pub struct MembershipResponse {
    // request: SignedMessage,
    accepted: bool,
    expiration: Option<Timestamp>,
}

impl MembershipResponse {
    pub fn new(accepted: bool, expiration: Option<Timestamp>) -> Self {
        Self {
            accepted,
            expiration,
        }
    }
}

pub type MembershipResponseType =
    crate::message::MessageType<MembershipResponse, 0xE6DD0F88165F0752>;

//

#[derive(Debug, Serialize, Deserialize)]
pub struct Empty {}

pub type ServeMeRequestType = crate::message::MessageType<Empty, 0xBA030E95BD57F286>;
pub type ServeMeResponseType = crate::message::MessageType<Empty, 0x73D6A76A63E79C06>;
