use log::{debug, error, info, warn};

use clap::Parser;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::collections::HashMap;
use std::process;
use vcl::{reload, update, Backend, Vcl};

mod vcl;
mod vcl_test;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[arg(short, long, default_value = "/etc/varnish/default.vcl")]
    vcl: String,

    #[arg(short, long, default_value = "./template/vcl.hbs")]
    template: String,

    #[arg(short, long, default_value = "varnish")]
    class: String,

    #[arg(short, long, default_value = "/etc/varnish/work")]
    working_folder: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    std::env::set_var("RUST_LOG", args.log_level);

    env_logger::init();

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            error!("could not init k8s client: {}", e);
            process::exit(1);
        }
    };

    info!("begin watching ingresses of class: [{}]", args.class);

    watch_ingresses(
        client,
        &args.vcl,
        &args.template,
        &args.working_folder,
        &args.class,
    )
    .await;
}

async fn watch_ingresses(
    client: Client,
    vcl_file: &str,
    vcl_template: &str,
    working_folder: &str,
    ingress_class_name: &str,
) {
    let ingress_api: Api<Ingress> = Api::all(client);

    let mut observer = watcher(
        ingress_api,
        watcher::Config::default()
            .labels(format!("kubernetes.io/ingress={}", ingress_class_name).as_str()),
    )
    .default_backoff()
    .boxed();

    let mut backends: HashMap<String, Vec<Backend>> = HashMap::new();

    while let Some(ev) = observer.try_next().await.unwrap() {
        match ev {
            watcher::Event::Apply(n) => {
                let ing_name = n.metadata.clone().name.unwrap();

                if !is_varnish_class(&n, ingress_class_name) {
                    info!(
                        "skipping ingress [{}], it does not have the varnish class",
                        ing_name
                    );
                    continue;
                }

                info!("parsing ingress [{}]", ing_name);
                let bbs = match parse_ingress_spec(n) {
                    Ok(bbs) => bbs,
                    Err(e) => {
                        error!("{}", e.to_string());
                        continue;
                    }
                };

                backends.insert(ing_name, bbs);

                reconcile(
                    vcl_file,
                    vcl_template,
                    working_folder,
                    backends.values().flat_map(|x| x.clone()).collect(),
                );
            }
            watcher::Event::Delete(obj) => {
                let ing_name = obj.metadata.clone().name.unwrap();

                if !is_varnish_class(&obj, ingress_class_name) {
                    info!(
                        "skipping ingress [{}], it does not have the varnish class",
                        ing_name
                    );
                    continue;
                }

                warn!("deleting ingress [{}]", ing_name);
                backends.remove(&ing_name);

                reconcile(
                    vcl_file,
                    vcl_template,
                    working_folder,
                    backends.values().flat_map(|x| x.clone()).collect(),
                );
            }
            watcher::Event::Init => {
                debug!("init event");
            }
            watcher::Event::InitApply(n) => {
                let ing_name = n.metadata.clone().name.unwrap();

                if !is_varnish_class(&n, ingress_class_name) {
                    info!(
                        "skipping ingress [{}], it does not have the varnish class",
                        ing_name
                    );
                    continue;
                }

                info!("[init-apply event] parsing ingress [{}]", ing_name);

                let bbs = match parse_ingress_spec(n) {
                    Ok(bbs) => bbs,
                    Err(e) => {
                        error!("{}", e.to_string());
                        continue;
                    }
                };

                backends.insert(ing_name, bbs);
            }
            watcher::Event::InitDone => {
                info!("done parsing ingresses, will now vcl reconcile");

                reconcile(
                    vcl_file,
                    vcl_template,
                    working_folder,
                    backends.values().flat_map(|x| x.clone()).collect(),
                );
            }
        }
    }
}

fn reconcile(vcl_file: &str, vcl_template: &str, working_folder: &str, backends: Vec<Backend>) {
    let mut v = Vcl::new(vcl_file, vcl_template, working_folder);

    if let Some(e) = update(&mut v, backends) {
        error!("{}", e);
        return;
    }

    if let Some(e) = reload(&v) {
        error!("{}", e);
    }
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
                    let host = rule.host.as_deref().unwrap_or("");
                    let path_str = path.path.as_deref().unwrap_or("");
                    let backend_name = format!(
                        "{}-{}",
                        ing.metadata.name.as_deref().unwrap_or(""),
                        backend_service.name
                    );
                    let port = backend_service
                        .port
                        .as_ref()
                        .and_then(|p| p.number)
                        .ok_or("Port number is missing")?;
                    let namespace = ing.metadata.namespace.as_deref().unwrap_or("");

                    let backend = Backend::new(
                        namespace.to_string(),
                        backend_name.clone(),
                        host.to_string(),
                        path_str.to_string(),
                        backend_service.name.clone(),
                        port as u16,
                    );

                    info!(
                        "found backend [{}] from ingress [{}]",
                        backend_name,
                        ing.metadata.name.as_deref().unwrap_or("")
                    );
                    backends.push(backend);
                }
            }
        }
    }

    Ok(backends)
}
