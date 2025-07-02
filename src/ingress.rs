use crate::vcl::{Backend, Vcl, reload, update};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::networking::v1::{Ingress, IngressLoadBalancerIngress};
use kube::api::{ListParams, Patch, PatchParams};
use kube::runtime::watcher::Error as WatcherError;
use kube::{
    Api, Client,
    runtime::{WatchStreamExt, watcher},
};
use log::{debug, error, info, warn};
use serde_json::json;
use std::process;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const VARNISH_CLASS: &str = "varnish";

pub async fn watch_ingresses(
    client: Client,
    vcl: &Rc<RefCell<Vcl<'_>>>,
    ingress_class_name: &str,
) -> Result<(), WatcherError> {
    let ingress_api: Api<Ingress> = Api::all(client);

    let mut observer = watcher(
        ingress_api,
        watcher::Config::default()
            .labels(format!("kubernetes.io/ingress={ingress_class_name}").as_str()),
    )
    .default_backoff()
    .boxed();

    let mut backends: HashMap<String, Vec<Backend>> = HashMap::new();
    info!(
        "Started watching ingresses of class: [{ingress_class_name}]",
    );

    while let Some(ev) = observer.try_next().await.unwrap() {
        match ev {
            watcher::Event::Apply(ingress) => {
                handle_ingress_event(&ingress, ingress_class_name, &mut backends);
                reconcile_backends(vcl, &backends);
            }
            watcher::Event::Delete(ingress) => {
                handle_ingress_delete(&ingress, ingress_class_name, &mut backends);
                reconcile_backends(vcl, &backends);
            }
            watcher::Event::Init => {
                debug!("Initialization event received");
            }
            watcher::Event::InitApply(ingress) => {
                handle_ingress_event(&ingress, ingress_class_name, &mut backends);
            }
            watcher::Event::InitDone => {
                info!(
                    "Finished processing initial ingress resources. Starting VCL reconciliation."
                );
                reconcile_backends(vcl, &backends);
            }
        }
    }
    Ok(())
}

pub async fn update_status(
    client: Client,
    load_balancer_ingress: Vec<IngressLoadBalancerIngress>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ingress_api: Api<Ingress> = Api::all(client.clone());

    let ingresses = ingress_api
        .list(&ListParams::default())
        .await
        .map_err(|err| {
            error!("Error listing ingresses: {err:?}");
            err
        })?;

    for ingress in ingresses.into_iter().filter(|ingress| {
        ingress
            .spec
            .as_ref()
            .and_then(|spec| spec.ingress_class_name.as_deref())
            == Some(VARNISH_CLASS)
    }) {
        let name = ingress.metadata.name.as_deref().unwrap_or_default();
        let namespace = ingress.metadata.namespace.as_deref().unwrap_or("default");

        if ingress.status.is_none() {
            warn!(
                "Ingress [{name}] in namespace [{namespace}] has no status"
            );
            continue;
        }

        let patch = json!({
            "status": {
                "loadBalancer": {
                    "ingress": load_balancer_ingress
                }
            }
        });

        debug!("Applying ingress status patch {patch}");

        let patch_params = PatchParams::apply("update-status");

        let ingress_api_namespaced = Api::<Ingress>::namespaced(client.clone(), namespace);

        match ingress_api_namespaced
            .patch_status(name, &patch_params, &Patch::Merge(&patch))
            .await
        {
            Ok(updated) => info!(
                "Patched ingress: [{}]",
                updated.metadata.name.unwrap_or_default()
            ),
            Err(err) => error!("Failed to patch ingress [{name}]: {err:?}"),
        }
    }

    Ok(())
}

fn is_varnish_class(ing: &Ingress, ingress_class_name: &str) -> bool {
    if let Some(spec) = &ing.spec {
        if let Some(class_name) = &spec.ingress_class_name {
            return class_name.eq_ignore_ascii_case(ingress_class_name);
        }
    }
    false
}

fn parse_ingress_spec(ing: Ingress) -> Result<Vec<Backend>, String> {
    let mut backends = Vec::new();

    let spec = match ing.spec {
        Some(spec) => spec,
        None => return Ok(backends),
    };

    if let Some(rules) = spec.rules {
        for rule in rules {
            let http = match &rule.http {
                Some(http) => http,
                None => continue,
            };

            for path in &http.paths {
                if let Some(backend_service) = &path.backend.service {
                    let namespace = ing.metadata.namespace.as_deref().unwrap_or("default");
                    let host = rule.host.as_deref().unwrap_or("");
                    let path_str = path.path.as_deref().unwrap_or("/");
                    let backend_name = format!(
                        "{}-{}-{}",
                        namespace,
                        ing.metadata.name.as_deref().unwrap_or("default"),
                        backend_service.name
                    );
                    let port = backend_service
                        .port
                        .as_ref()
                        .and_then(|p| p.number)
                        .ok_or("Port number is missing")?;

                    let backend = Backend::new(
                        namespace.to_string(),
                        backend_name.clone(),
                        host.to_string(),
                        path_str.to_string(),
                        backend_service.name.clone(),
                        path.path_type.clone(),
                        port as u16,
                    );

                    info!(
                        "Found backend [{}] from ingress [{}]",
                        backend_name,
                        ing.metadata.name.as_deref().unwrap_or("default")
                    );
                    backends.push(backend);
                }
            }
        }
    }

    Ok(backends)
}

fn handle_ingress_event(
    ingress: &Ingress,
    ingress_class_name: &str,
    backends: &mut HashMap<String, Vec<Backend>>,
) {
    let ing_name = ingress.metadata.name.as_deref().unwrap_or("default");

    if !is_varnish_class(ingress, ingress_class_name) {
        info!(
            "Skipping ingress [{ing_name}], it does not have the Varnish class."
        );
        return;
    }

    info!("Parsing ingress [{ing_name}]");
    match parse_ingress_spec(ingress.clone()) {
        Ok(bbs) => {
            backends.insert(ing_name.to_string(), bbs);
        }
        Err(e) => {
            error!("Error parsing ingress [{ing_name}]: {e}");
        }
    }
}

fn handle_ingress_delete(
    ingress: &Ingress,
    ingress_class_name: &str,
    backends: &mut HashMap<String, Vec<Backend>>,
) {
    let ing_name = ingress.metadata.name.as_deref().unwrap_or_default();

    if !is_varnish_class(ingress, ingress_class_name) {
        info!(
            "Skipping ingress [{ing_name}], it does not have the Varnish class."
        );
        return;
    }

    warn!("Deleting ingress [{ing_name}]");
    backends.remove(ing_name);
}

fn reconcile_backends(v: &Rc<RefCell<Vcl>>, backends: &HashMap<String, Vec<Backend>>) {
    let backends_list = backends.values().flatten().cloned().collect();

    v.borrow_mut().backends = backends_list;

    if let Err(e) = update(&v.borrow()) {
        error!("{e}");
        process::exit(1);
    }

    if let Err(e) = reload(&v.borrow()) {
        error!("{e}");
        process::exit(1);
    }
}
