use super::result::InfluxQuery;

pub trait Indicator: InfluxQuery + serde::Serialize {
    fn calculation(&self);
}
