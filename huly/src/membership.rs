//

use crate::db::Db;
use crate::id::{AccId, NodeId, OrgId};
use crate::message::SignedMessage;
use anyhow::{Context, Result};
use bytes::{Bytes, BytesMut};
use futures_lite::future::Boxed as BoxedFuture;
use futures_lite::StreamExt;
use iroh::endpoint::{get_remote_node_id, Connecting};
use iroh::protocol::ProtocolHandler;
use iroh::Endpoint;
use iroh_gossip::net::GossipSender;
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver, GOSSIP_ALPN},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, Clone)]
pub struct Membership {
    db: Db,
    endpoint: Endpoint,
}

impl Membership {
    pub const ALPN: &[u8] = b"huly/membership/0";

    pub fn new(db: Db, endpoint: Endpoint) -> Arc<Self> {
        Arc::new(Self { db, endpoint })
    }
}

async fn read_lp(
    mut reader: impl AsyncRead + Unpin,
    buffer: &mut BytesMut,
    max_message_size: usize,
) -> Result<Option<Bytes>> {
    let size = match reader.read_u32().await {
        Ok(size) => size,
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut reader = reader.take(size as u64);
    let size = usize::try_from(size).context("frame larger than usize")?;
    if size > max_message_size {
        anyhow::bail!(
            "Incoming message exceeds the maximum message size of {max_message_size} bytes"
        );
    }
    buffer.reserve(size);
    loop {
        let r = reader.read_buf(buffer).await?;
        if r == 0 {
            break;
        }
    }
    Ok(Some(buffer.split_to(size).freeze()))
}

const MAX_MESSAGE_SIZE: usize = 4096;

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

            let gossip = Gossip::builder().spawn(this.endpoint.clone()).await?;
            let topic = TopicId::from_bytes(account_id.into());
            let router = iroh::protocol::Router::builder(this.endpoint)
                .accept(GOSSIP_ALPN, gossip.clone())
                .spawn()
                .await?;

            let (sender, receiver) = gossip.subscribe_and_join(topic, vec![]).await?.split();
            tokio::spawn(account_loop(sender, receiver));

            let (mut send, mut recv) = connection.accept_bi().await?;

            loop {
                let mut buffer = BytesMut::with_capacity(MAX_MESSAGE_SIZE);
                match read_lp(&mut recv, &mut buffer, MAX_MESSAGE_SIZE).await? {
                    Some(bytes) => {
                        let message = SignedMessage::decode_and_verify(bytes.as_ref())?;
                        if message.get_signer() == device_id.into() {
                            match message.get_type() {
                                MembershipRequestType::TAG => {
                                    let request = MembershipRequestType::decode(message)?;
                                    println!("membership request: {:?}", request);
                                }
                                _ => anyhow::bail!("unknown message type"),
                            }
                        } else {
                            anyhow::bail!("message must be signed by the device");
                        }
                    }
                    None => break Ok(()),
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

pub type MembershipRequestType = crate::message::MessageType<MembershipRequest, 0x483A130AB92F3040>;

impl MembershipRequestType {
    pub fn make(device: NodeId, account: AccId, org: OrgId) -> MembershipRequest {
        MembershipRequest {
            device_ownership: DeviceOwnership { account, device },
            org,
        }
    }
}
