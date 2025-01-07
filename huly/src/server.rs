//

use crate::db::Db;
use crate::proto::{MembershipRequest, SignedMessage};
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
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, Clone)]
struct Server {
    // blobs: MemClient,
    db: Db,
    endpoint: Endpoint,
    // index: Arc<Mutex<HashMap<String, Hash>>>,
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

impl ProtocolHandler for Server {
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
                .get_device_account(device_id.as_bytes())?
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
                        let message = SignedMessage::from_bytes(bytes.as_ref())?;
                        if device_id.as_bytes() == message.get_signer() {
                            match message.get_type() {
                                MembershipRequest::TYPE => {
                                    let request: MembershipRequest = message.decode()?;
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

// impl BlobSearch {
//     /// Create a new protocol handler.
//     pub fn new(blobs: MemClient, endpoint: Endpoint) -> Arc<Self> {
//         Arc::new(Self {
//             blobs,
//             endpoint,
//             index: Default::default(),
//         })
//     }

//     /// Query a remote node, download all matching blobs and print the results.
//     pub async fn query_remote(&self, node_id: NodeId, query: &str) -> Result<Vec<Hash>> {
//         // Establish a connection to our node.
//         // We use the default node discovery in iroh, so we can connect by node id without
//         // providing further information.
//         let conn = self.endpoint.connect(node_id, ALPN).await?;

//         // Open a bi-directional in our connection.
//         let (mut send, mut recv) = conn.open_bi().await?;

//         // Send our query.
//         send.write_all(query.as_bytes()).await?;

//         // Finish the send stream, signalling that no further data will be sent.
//         // This makes the `read_to_end` call on the accepting side terminate.
//         send.finish()?;
//         // By calling stopped we wait until the remote iroh Endpoint has acknowledged all
//         // data.  This does not mean the remote application has received all data from the
//         // Endpoint.
//         send.stopped().await?;

//         // In this example, we simply collect all results into a vector.
//         // For real protocols, you'd usually want to return a stream of results instead.
//         let mut out = vec![];

//         // The response is sent as a list of 32-byte long hashes.
//         // We simply read one after the other into a byte buffer.
//         let mut hash_bytes = [0u8; 32];
//         loop {
//             // Read 32 bytes from the stream.
//             match recv.read_exact(&mut hash_bytes).await {
//                 // FinishedEarly means that the remote side did not send further data,
//                 // so in this case we break our loop.
//                 Err(quinn::ReadExactError::FinishedEarly(_)) => break,
//                 // Other errors are connection errors, so we bail.
//                 Err(err) => return Err(err.into()),
//                 Ok(_) => {}
//             };
//             // Upcast the raw bytes to the `Hash` type.
//             let hash = Hash::from_bytes(hash_bytes);
//             // Download the content via iroh-blobs.
//             self.blobs.download(hash, node_id.into()).await?.await?;
//             // Add the blob to our local database.
//             self.add_to_index(hash).await?;
//             out.push(hash);
//         }
//         Ok(out)
//     }

//     /// Query the local database.
//     ///
//     /// Returns the list of hashes of blobs which contain `query` literally.
//     pub fn query_local(&self, query: &str) -> Vec<Hash> {
//         let db = self.index.lock().unwrap();
//         db.iter()
//             .filter_map(|(text, hash)| text.contains(query).then_some(*hash))
//             .collect::<Vec<_>>()
//     }

//     /// Insert a text string into the database.
//     ///
//     /// This first imports the text as a blob into the iroh blob store, and then inserts a
//     /// reference to that hash in our (primitive) text database.
//     pub async fn insert_and_index(&self, text: String) -> Result<Hash> {
//         let hash = self.blobs.add_bytes(text.into_bytes()).await?.hash;
//         self.add_to_index(hash).await?;
//         Ok(hash)
//     }

//     /// Index a blob which is already in our blob store.
//     ///
//     /// This only indexes complete blobs that are smaller than 1KiB.
//     ///
//     /// Returns `true` if the blob was indexed.
//     async fn add_to_index(&self, hash: Hash) -> Result<bool> {
//         let mut reader = self.blobs.read(hash).await?;
//         // Skip blobs larger than 1KiB.
//         if reader.size() > 1024 * 1024 {
//             return Ok(false);
//         }
//         let bytes = reader.read_to_bytes().await?;
//         match String::from_utf8(bytes.to_vec()) {
//             Ok(text) => {
//                 let mut db = self.index.lock().unwrap();
//                 db.insert(text, hash);
//                 Ok(true)
//             }
//             Err(_err) => Ok(false),
//         }
//     }
// }
