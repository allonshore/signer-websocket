use arcstr::ArcStr;
use crossbeam_channel::Receiver;

use crossbeam_channel::Sender;
use serde::Serialize;

use crate::indicator_trait::result::{InfluxQuery, Json, SerializeToJsonAndInfluxQuery};

pub mod ichimoku_cloud;

#[derive(Debug, Clone, Serialize)]
pub struct Indicator<T>
where
    T: Serialize + InfluxQuery + Clone,
{
    table_name: String,
    exchange: String,
    market_type: String,
    msg_type: String,
    symbol: String,
    signer_type: String,
    data: T,
}

impl<T> Json for Indicator<T>
where
    T: InfluxQuery + Serialize + Clone,
{
    fn serialize(&self) -> String {
        if let Ok(json) = serde_json::to_string(&self) {
            json
        } else {
            r#"{}"#.to_string()
        }
    }
}

impl<T> InfluxQuery for Indicator<T>
where
    T: InfluxQuery + Serialize + Clone,
{
    fn serialize(&self) -> String {
        // "table_name,symbolset columnset timestamp\n"
        format!(
            "{table_name},{esymbolset} {symbolsetc_olumnset_timestamp}\n",
            table_name = self.table_name,
            esymbolset = format!(
                "exchange={},market_type={},msg_type={},symbol={}",
                self.exchange, self.market_type, self.msg_type, self.symbol
            ),
            symbolsetc_olumnset_timestamp = InfluxQuery::serialize(&self.data)
        )
    }
}

impl<T> SerializeToJsonAndInfluxQuery for Indicator<T>
where
    T: InfluxQuery + Serialize + Clone,
{
    fn to_json_and_influx_query(&self) -> (String, String) {
        let json = Json::serialize(self);
        let influx_query = InfluxQuery::serialize(self);

        (json, influx_query)
    }
}

unsafe impl<T> Send for Indicator<T> where T: InfluxQuery + Serialize + Clone {}

unsafe impl<T> Sync for Indicator<T> where T: InfluxQuery + Serialize + Clone {}

pub async fn create_ichimoku_cloud(
    signer_type: &str,
    sender: Sender<Box<dyn SerializeToJsonAndInfluxQuery>>,
    data: Receiver<(ArcStr, Vec<u8>)>,
) {
    match signer_type {
        "ichimoku_cloud" => ichimoku_cloud::kline_ichimku_monitor(data, sender).await,
        _ => tracing::error!("error sygner_typew not exis"),
    };
}
