//

use anyhow::Result;
use bytes::Bytes;
use ed25519_dalek::Signature;
use futures_lite::StreamExt;
use iroh::{Endpoint, PublicKey, SecretKey};
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver, GOSSIP_ALPN},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub async fn run(endpoint: Endpoint, peers: Vec<PublicKey>) -> Result<()> {
    // create the gossip protocol
    let gossip = Gossip::builder().spawn(endpoint.clone()).await?;

    let topic = TopicId::from_bytes([0xc0; 32]);
    // let topic = TopicId::from_bytes(rand::random());
    // let ticket = Ticket { topic, peers };
    // println!("ticket to join: {ticket}");

    // setup router
    let router = iroh::protocol::Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .spawn()
        .await?;

    let (sender, receiver) = gossip.subscribe_and_join(topic, peers).await?.split();
    println!("connected!");

    let message = Message::AboutMe {
        name: "Huly 0.1".to_string(),
    };
    let encoded_message = SignedMessage::sign_and_encode(endpoint.secret_key(), &message)?;
    sender.broadcast(encoded_message).await?;

    // subscribe and print loop
    tokio::spawn(subscribe_loop(receiver));

    // spawn an input thread that reads stdin
    // not using tokio here because they recommend this for "technical reasons"
    let (line_tx, mut line_rx) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || input_loop(line_tx));

    println!("type a message and hit enter to broadcast...");
    while let Some(text) = line_rx.recv().await {
        let message = Message::Message { text: text.clone() };
        let encoded_message = SignedMessage::sign_and_encode(endpoint.secret_key(), &message)?;
        sender.broadcast(encoded_message).await?;
        println!("sent: {text}");
    }

    router.shutdown().await?;

    Ok(())
}

async fn subscribe_loop(mut receiver: GossipReceiver) -> Result<()> {
    let mut names = HashMap::new();
    while let Some(event) = receiver.try_next().await? {
        if let Event::Gossip(GossipEvent::Received(msg)) = event {
            let (from, message) = SignedMessage::verify_and_decode(&msg.content)?;
            match message {
                Message::AboutMe { name } => {
                    names.insert(from, name.clone());
                    println!("> {} is now known as {}", from.fmt_short(), name);
                }
                Message::Message { text } => {
                    let name = names
                        .get(&from)
                        .map_or_else(|| from.fmt_short(), String::to_string);
                    println!("{}: {}", name, text);
                }
            }
        }
    }
    Ok(())
}

fn input_loop(line_tx: tokio::sync::mpsc::Sender<String>) -> Result<()> {
    let mut buffer = String::new();
    let stdin = std::io::stdin(); // We get `Stdin` here.
    loop {
        stdin.read_line(&mut buffer)?;
        line_tx.blocking_send(buffer.clone())?;
        buffer.clear();
    }
}

// helpers

// Ticket

// #[derive(Debug, Serialize, Deserialize)]
// struct Ticket {
//     topic: TopicId,
//     peers: Vec<NodeAddr>,
// }

// impl Ticket {
//     fn from_bytes(bytes: &[u8]) -> Result<Self> {
//         postcard::from_bytes(bytes).map_err(Into::into)
//     }
//     pub fn to_bytes(&self) -> Vec<u8> {
//         postcard::to_stdvec(self).expect("no chance")
//     }
// }

// /// Serializes to base32.
// impl fmt::Display for Ticket {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
//         text.make_ascii_lowercase();
//         write!(f, "{}", text)
//     }
// }

// messages

#[derive(Debug, Serialize, Deserialize)]
struct SignedMessage {
    from: PublicKey,
    data: Bytes,
    signature: Signature,
}

impl SignedMessage {
    pub fn verify_and_decode(bytes: &[u8]) -> Result<(PublicKey, Message)> {
        let signed_message: Self = postcard::from_bytes(bytes)?;
        let key: PublicKey = signed_message.from;
        key.verify(&signed_message.data, &signed_message.signature)?;
        let message: Message = postcard::from_bytes(&signed_message.data)?;
        Ok((signed_message.from, message))
    }

    pub fn sign_and_encode(secret_key: &SecretKey, message: &Message) -> Result<Bytes> {
        let data: Bytes = postcard::to_stdvec(&message)?.into();
        let signature = secret_key.sign(&data);
        let from: PublicKey = secret_key.public();
        let signed_message = Self {
            from,
            data,
            signature,
        };
        let encoded = postcard::to_stdvec(&signed_message)?;
        Ok(encoded.into())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    AboutMe { name: String },
    Message { text: String },
}
