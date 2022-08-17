#![allow(unused_variables)] //允许未使用的变量
#![allow(dead_code)] //允许未使用的代码
#![allow(unused_must_use)]
use crate::config::config::ApplicationConfig;
pub mod config;

pub async fn init_context() {
    init_config().await;
}

//初始化配置信息
pub async fn init_config() -> ApplicationConfig {
    // let content = read_to_string("application.toml").await.unwrap();
    let config = ApplicationConfig::new();
    println!("读取配置成功");
    config
}