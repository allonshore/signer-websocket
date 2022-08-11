use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};
use notify::{EventHandler, EventKind};
use tokio::{runtime::Handle, task::JoinHandle};

use crossbeam_channel::unbounded as channel;

pub use arcstr::ArcStr;
use wmjtyd_libstock::message::{
    traits::{Connect, StreamExt, Subscribe},
    zeromq::ZeromqSubscriber,
};

pub type SignerName = String;
pub type IpcUrl = Vec<ArcStr>;
pub type IpcSubList = Arc<Mutex<HashMap<ArcStr, JoinHandle<()>>>>;
pub type IpcData = (ArcStr, Vec<u8>);
pub type IpcSender = Sender<IpcData>;
pub type IpcReceiver = Receiver<IpcData>;
pub type IpcTaskCreator = dyn Fn(Handle, SignerName, IpcReceiver) -> JoinHandle<()> + Send + Sync;
pub type IpcBasicRepr = (SignerName, IpcUrl);

// #[derive(Default)]
// struct IpcDiff {
//     pub added: HashSet<IpcBasicRepr>,
//     pub removed: HashSet<IpcBasicRepr>,
// }

#[derive(Debug, Clone)]
struct SignerTask {
    pub signer_type: SignerName,
    pub task_handle: Arc<JoinHandle<()>>,
    pub ipc_sub_list: IpcSubList,
    pub tx: IpcSender,
}

pub struct DynamicConfigHandler {
    /// To allow spawning green thread in handler.
    handle: Handle,

    on_create: Box<IpcTaskCreator>,
    sginer_map: HashMap<String, SignerTask>,
}

impl DynamicConfigHandler {
    pub fn new(handle: Handle, on_create: Box<IpcTaskCreator>) -> Self {
        Self {
            handle,
            on_create,
            sginer_map: HashMap::with_capacity(3),
        }
    }

    fn cleanup(&mut self, tasks_to_clean_up: impl Iterator<Item = String>) {
        for sginer_type in tasks_to_clean_up {
            let task = self.sginer_map.remove(&sginer_type);
            if let Some(SignerTask {
                task_handle,
                signer_type,
                ipc_sub_list,
                tx,
            }) = task
            {
                tracing::debug!("abort signer {}", signer_type);
                tracing::debug!("abort tx len {}", tx.len());

                // 删除 ipc 订阅时产生的任务
                let ipc_sub_list_lock = if let Ok(lock) = ipc_sub_list.lock() {
                    lock
                } else {
                    continue;
                };
                for (_, ipc_sub) in ipc_sub_list_lock.iter() {
                    ipc_sub.abort();
                }

                // 删除主要信号任务
                task_handle.abort();
            }
        }
    }

    // 动态管理 ipc 的添加和删除
    // 这里应该把它抽离出来
    fn create_ipc_task(&self, tx: IpcSender, ipc_sub_list: IpcSubList, ipcs: Vec<ArcStr>) {
        let mut ipc_sub_list = if let Ok(lock) = ipc_sub_list.lock() {
            lock
        } else {
            tracing::error!("ipc sub not lock");
            return;
        };

        // 需要移除的 ipc
        // 先所有当前的 ipc 全部获取
        let mut remove: HashSet<ArcStr> =
            ipc_sub_list.iter().map(|(key, _)| key.to_owned()).collect();

        tracing::debug!("{:?}", ipcs);
        for ipc in ipcs {
            if remove.remove(&ipc) {
                continue;
            }

            let tx = tx.clone();
            tracing::debug!("path {}", ipc);
            let ipc_ = ipc.to_owned();
            let h = self.handle.spawn(async move {
                tracing::debug!("handle path {}", ipc);
                let sub = ZeromqSubscriber::new();
                if sub.is_err() {
                    tracing::error!("sub init");
                    return;
                }
                let mut sub = sub.unwrap();
                if sub.connect(ipc.as_str()).is_err() || sub.subscribe(b"").is_err() {
                    tracing::error!("sub connect");
                    return;
                }

                loop {
                    // 数据 payload
                    let message = StreamExt::next(&mut sub).await;
                    if message.is_none() {
                        break;
                    }

                    tracing::debug!("get data ipc: {}", ipc);

                    match message.unwrap() {
                        Ok(message) => {
                            if tx.send((ipc.to_owned(), message)).is_err() {
                                tracing::error!("send data");
                            }
                        }
                        Err(_) => {
                            tracing::error!("message zero");
                            break;
                        }
                    }
                }
                tracing::debug!("loop end")
            });
            ipc_sub_list.insert(ipc_.to_owned(), h);
        }
    }

    // 该函数做的事情太多了，但是不知道怎么拆分出来。
    // 管理 信号 创建和删除
    pub fn handel_config(&mut self, config_path: &str) -> anyhow::Result<()> {
        // 获取配置文件数据
        let new_config = deserialize_data(config_path)
            .context("failed to deserialize the configuration file")?;

        // 将要删除的信号
        // 暂时缓存起来
        let mut remove: HashSet<String> = self
            .sginer_map
            .iter()
            .map(|(key, _)| key.to_owned())
            .collect();

        tracing::debug!("add");
        tracing::debug!("{:?}", remove);
        tracing::debug!("{:?}", self.sginer_map);

        for (signer_type, ipcs) in new_config {
            // 把需要的重移除列表中删除需要的信号
            if remove.remove(&signer_type) {
                let ipcs: Vec<ArcStr> = ipcs.iter().map(|v| arcstr::ArcStr::from(v)).collect();
                if let Some(signer_task) = self.sginer_map.get(&signer_type) {
                    self.create_ipc_task(
                        signer_task.tx.clone(),
                        signer_task.ipc_sub_list.clone(),
                        ipcs,
                    );
                }
                continue;
            }

            // 任务不存在时创建任务
            // 这里有维护 ipc 通道的创建和销毁
            // 这里还维护了 ipc 线程数据通道
            tracing::debug!("add task");
            let ipc_sub_list: IpcSubList = Arc::new(Mutex::new(HashMap::new()));
            let ipcs: Vec<ArcStr> = ipcs.iter().map(|v| arcstr::ArcStr::from(v)).collect();

            // 获取到的数据全部从这里发送出去
            let (tx, ipc_data) = channel();

            // 管理 ipc
            self.create_ipc_task(tx.clone(), ipc_sub_list.clone(), ipcs);

            // 创建信号
            self.sginer_map.insert(
                signer_type.to_owned(),
                SignerTask {
                    signer_type: signer_type.to_owned(),
                    // 创建信号函数
                    task_handle: (self.on_create)(
                        self.handle.clone(),
                        signer_type.to_owned(),
                        ipc_data,
                    )
                    .into(),
                    ipc_sub_list,
                    tx: tx.clone(),
                },
            );
        }

        // 把删除列表中的任务全部删除
        self.cleanup(remove.into_iter());
        Ok(())
    }

    fn _handle_event(&mut self, event: notify::Result<notify::Event>) -> anyhow::Result<()> {
        let event = event.context("failed to handle data change")?;

        if let EventKind::Modify(_) = event.kind {
            let dir = event.paths.get(0).expect("should have at least a path");

            tracing::debug!("dir {}", dir.to_str().unwrap_or(""));
            self.handel_config(dir.to_str().unwrap_or(""))?;
        }

        Ok(())
    }
}

impl EventHandler for DynamicConfigHandler {
    fn handle_event(&mut self, event: notify::Result<notify::Event>) {
        let span = tracing::info_span!("handle_event");
        let _span = span.enter();

        match self._handle_event(event) {
            Ok(()) => tracing::debug!("Successfully handled event"),
            Err(err) => tracing::error!("Failed to handle event: {}", err),
        };
    }
}

fn deserialize_data(path: impl AsRef<Path>) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}
