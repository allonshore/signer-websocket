pub mod indicator;
pub mod indicator_trait;
pub mod util;

use std::io::Write;

use crossbeam_channel::unbounded as channel;
use indicator::create_ichimoku_cloud;
use indicator_trait::result::SerializeToJsonAndInfluxQuery;

use util::{config::ConfigBuilder, dynamic_config::DynamicConfigHandler};
use wmjtyd_libstock::message::{zeromq::ZeromqPublisher, traits::Bind};

pub async fn start(config: ConfigBuilder) {
    tracing::debug!("start questdb connect");
    let db = config
        .create_connect_questdb()
        .await
        .expect("connect quest error");

    
    let mut pub_socket = ZeromqPublisher::new().expect("init zeromq pub");
    pub_socket.bind("ipc:///tmp/signer_all.ipc").expect("path error");
    tracing::debug!("pub_socket bind");
    

    let handle = tokio::runtime::Handle::current();

    let (tx, rs) = channel::<Box<dyn SerializeToJsonAndInfluxQuery>>();

    let mut handler = DynamicConfigHandler::new(
        handle,
        Box::new(move |handle, sginer_name, ipcs| {
            tracing::debug!("handler");
            let tx = tx.clone();
            let handle_ = handle.clone();
            handle.spawn(async move {
                create_ichimoku_cloud(handle_, &sginer_name, tx, ipcs).await;
            })
        }),
    );

    handler.handel_config(config.config_path.as_ref()).expect("format error");


    let _watcher = config.create_watcher(handler).unwrap();

    for data in rs {
        let (json, influx_query) = data.to_json_and_influx_query();
        tracing::debug!("json {}", json);
        tracing::debug!("influx_query {}", influx_query);

        if db.send(influx_query.as_bytes()).await.is_err() {
            tracing::error!("quest send");
        }

        if pub_socket.write_all(&mut json.as_bytes()).is_err() {
            tracing::error!("ipc send");
        }
        
    }
}
