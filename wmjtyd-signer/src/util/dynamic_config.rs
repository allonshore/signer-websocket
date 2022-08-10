use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use notify::{EventHandler, EventKind};
use tokio::{runtime::Handle, task::JoinHandle};

pub use arcstr::ArcStr;

pub type SignerName = String;
pub type IpcUrl = Vec<ArcStr>;
pub type IpcTaskCreator = dyn Fn(Handle, SignerName, IpcUrl) -> JoinHandle<()> + Send + Sync;
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
            }) = task
            {
                tracing::debug!("abort signer {}", signer_type);
                task_handle.abort();
            }
        }
    }

    pub fn handel_config(&mut self, config_path: &str) -> anyhow::Result<()> {
        let new_config = deserialize_data(config_path)
            .context("failed to deserialize the configuration file")?;

        // if new_config.is_err() {
        //     panic!("123123");
        // }
        // let new_config = new_config.unwrap();

        let mut remove = HashSet::new();

        for (key, _) in self.sginer_map.iter() {
            remove.insert(key.to_owned());
        }

        tracing::debug!("add");
        tracing::debug!("{:?}", remove);
        tracing::debug!("{:?}", self.sginer_map);


        // 添加任务
        for (keys, ipcs) in new_config {
            if !remove.remove(&keys) {
                tracing::debug!("add task");
                self.sginer_map.insert(
                    keys.to_owned(),
                    SignerTask {
                        signer_type: keys.to_owned(),
                        task_handle: (self.on_create)(
                            self.handle.clone(),
                            keys.to_owned(),
                            ipcs.iter().map(|v| arcstr::ArcStr::from(v)).collect(),
                        )
                        .into(),
                    },
                );
            }
        }

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
