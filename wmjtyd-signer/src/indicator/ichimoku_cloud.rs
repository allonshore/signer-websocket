// #[derive(Serialize, )]
// struct IchimokuCloud {

use std::collections::HashMap;

use anyhow::Context;
use arcstr::ArcStr;
use crossbeam_channel::{Receiver, Sender};
use crypto_message::CandlestickMsg;
use serde::Serialize;
use wmjtyd_libstock::data::{kline::KlineStructure, serializer::StructDeserializer};
use yata::{
    core::{Action, IndicatorConfigDyn},
    indicators::IchimokuCloud,
};

use crate::{
    indicator_trait::result::{InfluxQuery, SerializeToJsonAndInfluxQuery},
    util::string_tool::ipc_uri_to_ident,
};

use super::Indicator;

#[derive(Debug, Clone, Serialize)]
pub struct IchimokuCloudResult {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    tenkan_sen: f64,
    kijun_sen: f64,
    senkou_span_a: f64,
    senkou_span_b: f64,
    s1_action: String,
    s2_action: String,
    timestamp: i64,
}

impl InfluxQuery for IchimokuCloudResult {
    fn serialize(&self) -> String {
        format!(
            "{symbol} {columnset} {timestamp}00000",
            symbol = "",
            columnset = format_args!(
                "open={},high={},low={},close={},volume={},tenkan_sen={},kijun_sen={},senkou_span_a={},senkou_span_b={},s1=\"{}\",s2=\"{}\"",
                self.open, self.high, self.low, self.close, self.volume,
                self.tenkan_sen,
                self.kijun_sen,
                self.senkou_span_a,
                self.senkou_span_b,
                self.s1_action,
                self.s2_action
            ),
            timestamp = self.timestamp
        )
    }
}

// }
pub async fn kline_ichimku_monitor(
    data: Receiver<(ArcStr, Vec<u8>)>,
    sender: Sender<Box<dyn SerializeToJsonAndInfluxQuery>>,
) {
    tracing::debug!("start ichimku_monitor");
    let mut ichimoku_cloud_map = HashMap::new();

    let ichimoku_cloud_config = IchimokuCloud::default();

    // for ((_, ipc_uri), kline_msg) in rx {
    for (ipc, data) in data {
        tracing::debug!("get ichimku_monitor ipc: {}", ipc);
        let kline_msg = if let Ok(kline_msg) = KlineStructure::deserialize(&mut data.as_slice()) {
            kline_msg
        } else {
            tracing::error!("data deserialize");
            continue;
        };

        let kline_msg = if let Ok(kline_msg) = CandlestickMsg::try_from(kline_msg) {
            kline_msg
        } else {
            tracing::error!("data map");
            continue;
        };

        let mut task = || {
            let data = (
                kline_msg.open,
                kline_msg.high,
                kline_msg.low,
                kline_msg.close,
                kline_msg.volume,
            );

            let product = ArcStr::from(ipc_uri_to_ident(&ipc));

            let ichimoku_cloud = if let Some(v) = ichimoku_cloud_map.get_mut(&product) {
                // have initiated
                v
            } else {
                let v = ichimoku_cloud_config
                    .init(&data)
                    .context("failed to initiate ichimoku_cloud")?;
                ichimoku_cloud_map.insert(product, v);

                return Ok("continue");
            };

            let inchimoku_cloud_result = ichimoku_cloud.next(&data);

            let mut result = inchimoku_cloud_result.values().iter();
            let mut action = inchimoku_cloud_result.signals().iter();

            let tenkan_sen = *result
                .next()
                .ok_or_else(|| anyhow::anyhow!("tenkan_sen is None"))?;
            let kijun_sen = *result
                .next()
                .ok_or_else(|| anyhow::anyhow!("kijun_sen is None"))?;
            let senkou_span_a = *result
                .next()
                .ok_or_else(|| anyhow::anyhow!("senkou_span_a is None"))?;
            let senkou_span_b = *result
                .next()
                .ok_or_else(|| anyhow::anyhow!("senkou_span_b is None"))?;

            let s1 = action.next();
            let s1_action = match s1 {
                Some(&Action::Buy(_)) => "buy",
                Some(&Action::Sell(_)) => "sell",
                Some(&Action::None) => "no",
                None => anyhow::bail!("no action 1"),
            };
            // let _s1_action_str = format!("\"{}\"", _s1_action);

            let s2 = action.next();
            let s2_action = match s2 {
                Some(&Action::Buy(_)) => "buy",
                Some(&Action::Sell(_)) => "sell",
                Some(&Action::None) => "no",
                None => anyhow::bail!("no action 2"),
            };
            // let _s2_action_str = format!("\"{}\"", _s2_action);

            sender
                .send(Box::new(Indicator {
                    table_name: "kline_ichimoku_monitor_v4".to_string(),
                    exchange: "name".to_string(),
                    market_type: "sudo".to_string(),
                    msg_type: "ccc".to_string(),
                    symbol: "ccc".to_string(),
                    signer_type: "ichimoku_cloud".to_string(),
                    data: IchimokuCloudResult {
                        open: kline_msg.open,
                        high: kline_msg.high,
                        low: kline_msg.low,
                        close: kline_msg.close,
                        volume: kline_msg.volume,

                        tenkan_sen,
                        kijun_sen,
                        senkou_span_a,
                        senkou_span_b,
                        s1_action: s1_action.to_owned(),
                        s2_action: s2_action.to_owned(),

                        timestamp: kline_msg.timestamp,
                    },
                }))
                .unwrap();

            // questdb_udp_socket
            //     .send(ilp_inchimoku_cloud_signer.as_bytes())
            //     .await
            //     .context("failed to send kline_ichimoku_monitor_v3")?;

            Ok::<&'static str, anyhow::Error>("done")
        };

        match task() {
            Ok("continue") => {
                tracing::debug!("processed the kline_msg, but was ended early.");
                continue;
            }
            Ok(_) => {
                tracing::debug!("successfully processed this kline_msg.");
            }
            Err(e) => tracing::error!("failed to process kline_msg: {e:?}."),
        };
    }
}
