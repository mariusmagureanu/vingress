use futures::{StreamExt, TryStreamExt};
use k8s_openapi::{api::core::v1::ConfigMap, Metadata};
use kube::runtime::watcher::Error as WatcherError;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use log::{error, info, warn};
use std::{cell::RefCell, rc::Rc};

use crate::vcl::{reload, update, Vcl};

pub async fn watch_configmap(
    client: Client,
    vcl: &Rc<RefCell<Vcl<'_>>>,
    configmap_name: &str,
    namespace: &str,
) -> Result<(), WatcherError> {
    let configmap_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    let mut observer = watcher(configmap_api, watcher::Config::default())
        .default_backoff()
        .boxed();

    info!(
        "Started watching configmap: [{}] in namespace: [{}]",
        configmap_name, namespace
    );

    while let Some(event) = observer.try_next().await.unwrap() {
        match event {
            watcher::Event::Apply(cm) => handle_configmap_event(&cm, vcl, configmap_name),
            watcher::Event::Delete(cm) => handle_configmap_event(&cm, vcl, configmap_name),
            _ => {}
        }
    }

    Ok(())
}

fn handle_configmap_event(cm: &ConfigMap, vcl: &Rc<RefCell<Vcl>>, configmap_name: &str) {
    match cm.metadata().name.as_deref() {
        Some(name) if name == configmap_name => {
            info!("Reading the [{}] configmap", configmap_name);

            match cm.data.as_ref().and_then(|data| data.get("snippet")) {
                Some(snippet) => {
                    vcl.borrow_mut().snippet = snippet.clone();

                    if let Err(e) = update(&vcl.borrow()) {
                        error!("Failed to update VCL snippet: {}", e);
                        return;
                    }

                    if let Err(e) = reload(&vcl.borrow()) {
                        error!("Failed to reload VCL with snippet: {}", e);
                    }
                }
                None => {
                    warn!(
                        "No 'snippet' key found in the [{}] configmap",
                        configmap_name
                    );
                }
            }
        }
        Some(_) => { /* ConfigMap name does not match; do nothing. */ }
        None => warn!("Could not get the name of VCL configmap"),
    }
}
