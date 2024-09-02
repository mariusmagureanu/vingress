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

const CONFIGMAP_NAME: &str = "varnish-vcl";

pub const SNIPPET_KEY: &str = "snippet";
pub const VCL_RECV_SNIPPET_KEY: &str = "vcl_recv_snippet";

pub async fn watch_configmap(
    client: Client,
    vcl: &Rc<RefCell<Vcl<'_>>>,
    namespace: &str,
) -> Result<(), WatcherError> {
    let configmap_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    let mut observer = watcher(configmap_api, watcher::Config::default())
        .default_backoff()
        .boxed();

    info!(
        "Started watching configmap: [{}] in namespace: [{}]",
        CONFIGMAP_NAME, namespace
    );

    while let Some(event) = observer.try_next().await.unwrap() {
        match event {
            watcher::Event::Apply(cm) => handle_configmap_event(&cm, vcl, CONFIGMAP_NAME),
            watcher::Event::Delete(cm) => handle_configmap_event(&cm, vcl, CONFIGMAP_NAME),
            _ => {}
        }
    }

    Ok(())
}

fn handle_configmap_event(cm: &ConfigMap, vcl: &Rc<RefCell<Vcl>>, configmap_name: &str) {
    match cm.metadata().name.as_deref() {
        Some(name) if name == configmap_name => {
            info!("Reading the [{}] configmap", configmap_name);

            let data = cm.data.as_ref();

            let snippet_updated = if let Some(snippet) = data.and_then(|data| data.get(SNIPPET_KEY))
            {
                vcl.borrow_mut().snippet = snippet.clone();
                true
            } else {
                warn!(
                    "No 'snippet' key found in the [{}] configmap",
                    configmap_name
                );
                false
            };

            let vcl_recv_snippet_updated = if let Some(vcl_recv_snippet) =
                data.and_then(|data| data.get(VCL_RECV_SNIPPET_KEY))
            {
                vcl.borrow_mut().vcl_recv_snippet = vcl_recv_snippet.clone();

                true
            } else {
                warn!(
                    "No 'vcl_recv_snippet' key found in the [{}] configmap",
                    configmap_name
                );
                false
            };

            if snippet_updated || vcl_recv_snippet_updated {
                if let Err(e) = update(&vcl.borrow()) {
                    error!("Failed to update VCL with updated snippets: {}", e);
                    return;
                }

                if let Err(e) = reload(&vcl.borrow()) {
                    error!("Failed to reload VCL with updated snippets: {}", e);
                }
            }
        }
        Some(_) => {}
        None => warn!("Could not get the name of VCL configmap"),
    }
}
