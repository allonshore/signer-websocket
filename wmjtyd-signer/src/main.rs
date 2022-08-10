use clap::clap_app;
use wmjtyd_signer::util::config::{ConfigBuilder, QuestdbConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches: clap::ArgMatches = clap_app!(quic =>
        (about: "Signal calculation organization procedure")
        (@arg SIGNERLIST: -l --signerlist "View all signals")
        (@arg CONFIGPATH: -c --configpath "Config path")
        (@arg QUESTDBADDR: --questdbaddr "QuestDB-Addr default 127.0.0.1:9009")
        (@arg QUESTDBTTL: --questdbttl "QuestDB-TTL default 100")
    )
    .get_matches();

    tracing::debug!("yes");

    let config_path = matches.value_of("CONFIGPATH").unwrap_or("./conf/signer.json");

    let questdb_addr = matches.value_of("QUESTDBADDR").unwrap_or("127.0.0.1:9009");

    let questdb_ttl: i32 = matches
        .value_of("QUESTDBTTL")
        .unwrap_or("100")
        .parse()
        .expect("--questdb-ttl arg error; Number required");

    let questdb_config = QuestdbConfig::new(questdb_addr, questdb_ttl);

    let config_builder = ConfigBuilder::new(questdb_config, config_path);

    // let mut signer_pool = SignerPool::new(config_builder);

    // signer_pool.start();
    wmjtyd_signer::start(config_builder).await;
}
