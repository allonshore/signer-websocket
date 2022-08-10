use crate::indicator_trait::data::{GetShareData};
use anyhow::Context;
use notify::{EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

// 设置 questdb 的连接地址
// 设置 需要开启的信号！
// 设置 配置文件的路径

// 这里可以构建 questdb 连接
// 可以构建订阅 ipc
// 可以构建发布 ipc
// 可以构建监听 配置文件的更新
pub struct ConfigBuilder {
    pub questdb: QuestdbConfig,
    pub config_path: String,
}

// 设置 questdb
pub struct QuestdbConfig {
    pub addr: SocketAddr,
    pub ttl: i32,
}

impl ConfigBuilder {
    pub fn new(questdb: QuestdbConfig, config_path: &str) -> Self {
        ConfigBuilder {
            questdb,
            config_path: config_path.to_owned(),
        }
    }

    // quest 连接
    pub async fn create_connect_questdb(&self) -> anyhow::Result<tokio::net::UdpSocket> {
        // let questdb = QuestDB::connect(&self.questdb).await?;

        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .context("unable to bind on address")?;

        socket
            .connect(self.questdb.addr)
            .await
            .context("unable to connect to QuestDB")?;
        socket.set_ttl(100).context("unable to set ttl")?;

        Ok(socket)
    }

    // 发布
    // pub fn create_send_ipc(&self) -> anyhow::Result<Box<dyn SendShareData<Error = String>>> {

    // }

    // 监听
    pub fn create_watcher(&self, handler: impl EventHandler) -> anyhow::Result<RecommendedWatcher> {
        let path = self.config_path.as_ref();

        let mut watcher = notify::recommended_watcher(handler)?;

        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .context("failed to watch the specified path")?;

        Ok(watcher)
    }

    // 订阅
    pub fn create_get_ipc(&self) -> anyhow::Result<Box<dyn GetShareData<Error = String>>> {
        todo!()
    }
}

impl QuestdbConfig {
    pub fn new(addr: &str, ttl: i32) -> Self {
        let addr: SocketAddr = if let Ok(v) = addr.parse() {
            v
        } else {
            panic!("questdb addr error");
        };
        QuestdbConfig { addr, ttl }
    }
}
