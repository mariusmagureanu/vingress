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
    vcl: &Rc<RefCell<Vcl>>,
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
    if let Some(name) = cm.metadata().name.as_ref() {
        if name != configmap_name {
            return;
        }
    } else {
        warn!("Could not get the name of vcl configmap");
        return;
    }

    info!("Reading the [{}] configmap", configmap_name);

    let snippet = cm.data.clone().unwrap().get("snippet").unwrap().clone();

    vcl.borrow_mut().snippet = snippet.to_string();

    if let Some(e) = update(&vcl.borrow()) {
        error!("{}", e);
        return;
    }

    if let Some(e) = reload(&vcl.borrow()) {
        error!("{}", e);
    }
}
