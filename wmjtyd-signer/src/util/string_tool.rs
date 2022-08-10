#[macro_export]
/// Usage: `{table},{tags} {fields} {timestamp}`
macro_rules! influx_query {
    (table = $table:expr, tags = $tags:expr, fields = $fields:expr, timestamp = $timestamp:expr) => {{
        let query = format!(
            "{table},{tags} {fields} {timestamp}",
            table = $table,
            tags = $tags,
            fields = $fields,
            timestamp = $timestamp
        );
        tracing::debug!("InfluxDB Line Query → {table}: {query}", table = $table);
        query
    }};
}

pub use influx_query;

pub fn ipc_uri_to_ident(uri: &str) -> &str {
    uri.trim_start_matches("ipc:///tmp/")
        .trim_end_matches(".ipc")
}
