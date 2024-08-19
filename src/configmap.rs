use futures::{StreamExt, TryStreamExt};
use k8s_openapi::{api::core::v1::ConfigMap, Metadata};
use kube::runtime::watcher::Error as WatcherError;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};

pub async fn watch_configmap(
    client: Client,
    configmap_name: &str,
    namespace: &str,
) -> Result<(), WatcherError> {
    let configmap_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    let mut observer = watcher(configmap_api, watcher::Config::default())
        .default_backoff()
        .boxed();

    while let Some(event) = observer.try_next().await? {
        match event {
            watcher::Event::Apply(cm) => handle_configmap_event(&cm, configmap_name),
            watcher::Event::Delete(cm) => handle_configmap_event(&cm, configmap_name),
            _ => {}
        }
    }

    Ok(())
}

fn handle_configmap_event(cm: &ConfigMap, configmap_name: &str) {
    if let Some(name) = cm.metadata().name.as_ref() {
        if name == configmap_name {
            println!("ConfigMap '{}' event handled", configmap_name);
        }
    }
}
