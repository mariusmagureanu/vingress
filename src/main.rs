use log::{debug, error, info, warn};

use futures::TryStreamExt;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::pin::pin;
use std::process;
use vcl::{update, Backend, Vcl};

mod vcl;
mod vcl_test;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };

    let cluster_name = match get_cluster_name_from_context() {
        Ok(cn) => cn,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };

    info!("connected to cluster: {}", cluster_name);

    watch_ingresses(client).await;
}

async fn watch_ingresses(client: Client) {
    let ings: Api<Ingress> = Api::all(client);

    let wc = watcher::Config::default();
    let observer = watcher(ings, wc).default_backoff().applied_objects();

    let mut obs = pin!(observer);

    while let Some(n) = obs.try_next().await.unwrap() {
        let ing_class = n.spec.clone().unwrap().ingress_class_name;

        let class_name = match ing_class {
            Some(ic) => ic.to_lowercase(),
            None => continue,
        };

        if class_name != "varnish" {
            debug!("skipping ingress class {}", class_name);
            continue;
        }

        let mut backends: Vec<Backend> = vec![];

        if let Some(spec) = n.spec {
            if let Some(rules) = spec.rules {
                rules.iter().for_each(|x| {
                    if let Some(http) = x.http.clone() {
                        http.paths.iter().for_each(|y| {
                            let backend = Backend::new(
                                n.metadata.clone().name.unwrap(),
                                x.host.clone().unwrap(),
                                y.path.clone().unwrap(),
                                y.backend
                                    .clone()
                                    .service
                                    .unwrap()
                                    .port
                                    .unwrap()
                                    .number
                                    .unwrap()
                                    .try_into()
                                    .unwrap(),
                            );

                            debug!("adding backend {}", backend.name);
                            backends.push(backend);
                        })
                    }
                });
            } else {
                warn!("no rules found in the ingress manifest");
                continue;
            }
        } else {
            warn!("no spec found in the ingress manifest");
            continue;
        }

        let mut v = Vcl::new("default.vcl", "./template/vcl.hbs");

        match update(&mut v, backends) {
            None => info!("default.vcl file has just been updated"),
            Some(e) => error!("{}", e),
        }
    }
}

fn get_cluster_name_from_context() -> Result<String, String> {
    let kube_config = match kube::config::Kubeconfig::read() {
        Ok(kc) => kc,
        Err(e) => return Err(e.to_string()),
    };

    match kube_config.current_context {
        Some(kcc) => Ok(kcc),
        None => Err("could not retrieve a current k8s context".to_string()),
    }
}
