pub trait Json {
    fn serialize(&self) -> String;
}

pub trait InfluxQuery {
    fn serialize(&self) -> String;
}

pub trait SerializeToJsonAndInfluxQuery: Json + InfluxQuery + Send + Sync {
    fn to_json_and_influx_query(&self) -> (String, String);
}
