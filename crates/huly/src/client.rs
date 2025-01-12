//

use crate::id::{AccId, OrgId};
use crate::membership::{Membership, MembershipRequest, ServeMeRequest};
use crate::message::{Message, MessageType, SignedMessage};
use anyhow::Result;
use futures_lite::StreamExt;
use iroh::{Endpoint, NodeId, SecretKey};
use iroh_gossip::{net::Gossip, proto::TopicId};

pub async fn request_membership(
    secret_key: &SecretKey,
    endpoint: Endpoint,
    account: AccId,
    org: OrgId,
    gossip: Gossip,
) -> Result<()> {
    let node_id = NodeId::from_bytes(org.as_bytes())?;
    let conn = endpoint.connect(node_id, Membership::ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;

    let request = MembershipRequest::new(secret_key.public().into(), account, org);
    let encoded = MembershipRequest::encode(&request)?;
    let signed = SignedMessage::sign(secret_key, encoded)?;
    let encoded = SignedMessage::encode(&signed)?;

    encoded.write_async(&mut send).await?;
    println!("sent membership request: {:?}", request);

    let response = Message::read_async(&mut recv).await?;
    println!("got membership response: {:?}", response);

    let topic = TopicId::from_bytes(account.into());
    // let gossip = Gossip::builder().spawn(endpoint.clone()).await?;

    let (_sender, mut receiver) = gossip.subscribe(topic, vec![node_id])?.split();

    println!("started gossip proto");

    // Spawn a task to handle received messages
    let gossip_handle = tokio::spawn(async move {
        while let Some(event) = receiver.try_next().await? {
            println!("Client received gossip event: {:?}", event);
        }
        Ok::<_, anyhow::Error>(())
    });

    let request = ServeMeRequest::encode(&ServeMeRequest {})?;
    request.write_async(&mut send).await?;
    println!("sent serve me request");

    let response = Message::read_async(&mut recv).await?;
    println!("got serve me response: {:?}", response);

    // Wait for Ctrl-C while keeping gossip connection alive
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl-C, shutting down");
        }
        res = gossip_handle => {
            if let Err(e) = res {
                println!("Gossip task error: {:?}", e);
            }
        }
    }

    send.finish()?;
    send.stopped().await?;

    Ok(())
}
