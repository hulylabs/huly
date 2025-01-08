//

use crate::id::{AccId, OrgId};
use crate::membership::{
    Empty, Membership, MembershipRequest, MembershipRequestType, ServeMeRequestType,
};
use crate::message::{Message, SignedMessage, SignedMessageType};
use anyhow::Result;
use iroh::{Endpoint, NodeId, SecretKey};
use iroh_gossip::net::GossipSender;
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver},
    proto::TopicId,
};

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
    let encoded = MembershipRequestType::encode(&request)?;
    let signed = SignedMessage::sign(secret_key, encoded)?;
    let encoded = SignedMessageType::encode(&signed)?;

    encoded.write_async(&mut send).await?;
    println!("sent membership request: {:?}", request);

    let response = Message::read_async(&mut recv).await?;
    println!("got membership response: {:?}", response);

    let topic = TopicId::from_bytes(account.into());
    // let gossip = Gossip::builder().spawn(endpoint.clone()).await?;

    let (sender, receiver) = gossip
        .subscribe_and_join(topic, vec![node_id])
        .await?
        .split();

    println!("started gossip proto");

    let request = ServeMeRequestType::encode(&Empty {})?;
    request.write_async(&mut send).await?;
    println!("sent serve me request");

    let response = Message::read_async(&mut recv).await?;
    println!("got serve me response: {:?}", response);

    tokio::signal::ctrl_c().await?;

    send.finish()?;
    send.stopped().await?;

    // // In this example, we simply collect all results into a vector.
    // // For real protocols, you'd usually want to return a stream of results instead.
    // let mut out = vec![];

    // // The response is sent as a list of 32-byte long hashes.
    // // We simply read one after the other into a byte buffer.
    // let mut hash_bytes = [0u8; 32];
    // loop {
    //     // Read 32 bytes from the stream.
    //     match recv.read_exact(&mut hash_bytes).await {
    //         // FinishedEarly means that the remote side did not send further data,
    //         // so in this case we break our loop.
    //         Err(quinn::ReadExactError::FinishedEarly(_)) => break,
    //         // Other errors are connection errors, so we bail.
    //         Err(err) => return Err(err.into()),
    //         Ok(_) => {}
    //     };
    //     // Upcast the raw bytes to the `Hash` type.
    //     let hash = Hash::from_bytes(hash_bytes);
    //     // Download the content via iroh-blobs.
    //     self.blobs.download(hash, node_id.into()).await?.await?;
    //     // Add the blob to our local database.
    //     self.add_to_index(hash).await?;
    //     out.push(hash);
    // }

    Ok(())
}
