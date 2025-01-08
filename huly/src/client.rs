//

use crate::id::{AccId, Hash, OrgId, Uid};
use crate::membership::{
    Membership, MembershipRequestType, MembershipResponseType, MAX_MESSAGE_SIZE,
};
use crate::message::{read_lp, write_lp, Message};
use anyhow::Result;
use bytes::BytesMut;
use iroh::{Endpoint, NodeId, SecretKey};

pub async fn request_membership(
    secret_key: &SecretKey,
    endpoint: Endpoint,
    account: AccId,
    org: OrgId,
) -> Result<()> {
    let node_id = NodeId::from_bytes(org.as_bytes())?;
    let conn = endpoint.connect(node_id, Membership::ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;

    let request = MembershipRequestType::make(secret_key.public().into(), account, org);
    let encoded = MembershipRequestType::sign_and_encode(secret_key, &request)?;
    write_lp(&mut send, &encoded, MAX_MESSAGE_SIZE).await?;

    let mut buffer = BytesMut::with_capacity(MAX_MESSAGE_SIZE);
    let encoded = read_lp(&mut recv, &mut buffer, MAX_MESSAGE_SIZE).await?;

    if let Some(encoded) = encoded {
        let message = Message::decode(encoded.as_ref())?;
        println!("got membership response: {:?}", message);
    } else {
        println!("unexpected end of stream?")
    }

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
