pub trait DB {
    type Error;
    // type ResultError;
    fn send_db(&mut self, influx_query: &str) -> Result<(), Self::Error>;
}

pub trait SendShareData {
    type Error;
    fn send_share_data(&mut self, json: &str) -> Result<(), Self::Error>;
}

pub trait GetShareData {
    type Error;
    fn get_share_data(&mut self) -> Result<&[u8], Self::Error>;
}
