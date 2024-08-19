use futures::{StreamExt, TryStreamExt};
use k8s_openapi::{api::core::v1::ConfigMap, Metadata};
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};

pub async fn watch_configmap(client: Client, configmap_name: &str, namespace: &str) {
    let configmap_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    let mut observer = watcher(configmap_api, watcher::Config::default())
        .default_backoff()
        .boxed();

    while let Some(ev) = observer.try_next().await.unwrap() {
        match ev {
            watcher::Event::Apply(cm) => handle_configmap_event(&cm, configmap_name),
            watcher::Event::Delete(cm) => handle_configmap_event(&cm, configmap_name),
            _ => {}
        }
    }
}

fn handle_configmap_event(cm: &ConfigMap, cm_name: &str) {
    if cm.metadata().name.unwrap() != cm_name {
        return;
    }
}
