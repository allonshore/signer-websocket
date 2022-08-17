#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, Clone)]
pub struct ApplicationConfig {
    pub log_dir: String,
    pub log_level: String,
    pub record_dir: String,
    pub host: String,
    pub port: u16,
}

impl ApplicationConfig {
    pub fn new() -> Self {
        let yml_data = include_str!("../../application.yml");
        let result = serde_yaml::from_str(yml_data).expect("load config file failed");
        println!("{:?}", result);
        result
    }
}
