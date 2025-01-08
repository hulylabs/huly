//

use anyhow::{bail, Ok, Result};
use clap::Parser;
use config::Config;
use huly::db::Db;
use huly::id::{AccId, OrgId};
use huly::membership::Membership;
use iroh::protocol::Router;
use iroh::{Endpoint, PublicKey, RelayMap, RelayMode, RelayUrl, SecretKey};
use iroh_gossip::net::GossipSender;
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver, GOSSIP_ALPN},
    proto::TopicId,
};
use std::net::{Ipv4Addr, SocketAddrV4};

/// By default, the relay server run by n0 is used. To use a local relay server, run
///     cargo run --bin iroh-relay --features iroh-relay -- --dev
/// in another terminal and then set the `-d http://localhost:3340` flag on this example.
#[derive(Parser, Debug)]
struct Args {
    // #[clap(long)]
    // secret_key: Option<String>,
    #[clap(short, long)]
    relay: Option<RelayUrl>,
    #[clap(long)]
    no_relay: bool,
    #[clap(short, long)]
    db: String,
    #[clap(long)]
    db_init: bool,
    #[clap(short, long, default_value = "0")]
    bind_port: u16,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    Client { server: String, account: String },
    Server {},
    CreateDb,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let settings = Config::builder()
        // .add_source(config::File::with_name("settings"))
        .add_source(config::Environment::with_prefix("HULY"))
        .build()
        .unwrap();

    let secret_key = match settings.get::<Option<String>>("secret")? {
        None => SecretKey::generate(rand::rngs::OsRng),
        Some(key) => key.parse()?,
    };

    println!("secret: {}", secret_key);

    // configure relay map
    let relay_mode = match (args.no_relay, args.relay) {
        (false, None) => RelayMode::Default,
        (false, Some(url)) => RelayMode::Custom(RelayMap::from_url(url)),
        (true, None) => RelayMode::Disabled,
        (true, Some(_)) => bail!("You cannot set --no-relay and --relay at the same time"),
    };

    println!("using secret key: {secret_key}");
    println!("using relay servers: {}", fmt_relay_mode(&relay_mode));

    let endpoint = Endpoint::builder()
        .secret_key(secret_key.clone())
        .relay_mode(relay_mode)
        .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, args.bind_port))
        .discovery_local_network()
        // .discovery_dht()
        // .discovery_n0()
        .bind()
        .await?;

    println!("ready with node id: {}", endpoint.node_id());

    let db = match args.db_init {
        true => Db::create(&args.db)?,
        false => Db::open(&args.db)?,
    };

    let router_builder = Router::builder(endpoint.clone());

    let gossip = Gossip::builder().spawn(endpoint.clone()).await?;
    let router_builder = router_builder.accept(GOSSIP_ALPN, gossip.clone());

    let membership = Membership::new(db, endpoint.clone(), gossip.clone());
    let router_builder = router_builder.accept(Membership::ALPN, membership.clone());

    let router = router_builder.spawn().await?;

    match args.command {
        Command::Server {} => {
            let node_id = router.endpoint().node_id();
            println!("membership proto started on node id: {node_id}");

            // for text in text.into_iter() {
            //     proto.insert_and_index(text).await?;
            // }

            // Wait for Ctrl-C to be pressed.
            tokio::signal::ctrl_c().await?;
        }
        Command::Client { server, account } => {
            let account: AccId = account.parse()?;
            let org: OrgId = server.parse()?;
            huly::client::request_membership(&secret_key.clone(), endpoint.clone(), account, org)
                .await?;
        }
        Command::CreateDb => {
            let _ = Db::create(&args.db)?;
        }
    }

    // sleep(Duration::from_secs(60)).await;

    // 88877a049601655b479cf46b906669266066a6eda2473aadf1574fffaa1353a7
    // 67c78c9886bc71fd91415577e078de03966bc17603d52a1355ad53cb53571ae1

    // 802ec3ff23cdd6bc67b4b45c9d3dd92bd518c1b4c6708fcde1ce2a1a7abc6aef
    // b60988059e237d6e1ccc9f1b9985123a3db34b21a527e14b4bad99574aeabed9

    // Account:
    // d28aeaafe8e8c70f16bc862085795dfcb45c083ab8ff0754654b0e35a45fe339
    // 22cfbf283eb134a3cde229fec9de9f97aa946021d484e66a308b7a79b005c814

    // let peers: Vec<PublicKey> = vec![
    //     "67c78c9886bc71fd91415577e078de03966bc17603d52a1355ad53cb53571ae1".parse()?,
    //     "b60988059e237d6e1ccc9f1b9985123a3db34b21a527e14b4bad99574aeabed9".parse()?,
    //     "22cfbf283eb134a3cde229fec9de9f97aa946021d484e66a308b7a79b005c814".parse()?,
    // ];

    // run(endpoint, peers).await
    // let client = Client::connect(
    //     uuid::Uuid::new_v4(),
    //     secret_key,
    //     vec![],
    //     relay_mode,
    //     args.bind_port,
    // )
    // .await?;

    // client.run().await

    router.shutdown().await?;

    Ok(())
}

fn fmt_relay_mode(relay_mode: &RelayMode) -> String {
    match relay_mode {
        RelayMode::Disabled => "None".to_string(),
        RelayMode::Default => "Default Relay (production) servers".to_string(),
        RelayMode::Staging => "Default Relay (staging) servers".to_string(),
        RelayMode::Custom(map) => map
            .urls()
            .map(|url| url.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    }
}
