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

    let mut observer = watcher(ingress_api, watcher::Config::default())
        .default_backoff()
        .boxed();

    let mut backends: HashMap<String, Vec<Backend>> = HashMap::new();

    while let Some(ev) = observer.try_next().await.unwrap() {
        match ev {
            watcher::Event::Apply(n) => {
                let ing_name = n.metadata.clone().name.unwrap();

                if !is_varnish_class(n.clone(), ingress_class_name) {
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

                if !is_varnish_class(obj.clone(), ingress_class_name) {
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

                if !is_varnish_class(n.clone(), ingress_class_name) {
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

    match update(&mut v, backends) {
        None => {}
        Some(e) => {
            error!("{}", e);
            return;
        }
    }

    match reload(&v) {
        None => {}
        Some(e) => error!("{}", e),
    }
}

fn is_varnish_class(ing: Ingress, ingress_class_name: &str) -> bool {
    let ing_class = ing.spec.clone().unwrap().ingress_class_name;

    let class_name = match ing_class {
        Some(ic) => ic.to_lowercase(),
        None => return false,
    };

    class_name == ingress_class_name
}

fn parse_ingress_spec(ing: Ingress) -> Result<Vec<Backend>, String> {
    let mut backends: Vec<Backend> = vec![];

    if let Some(spec) = ing.spec {
        if let Some(rules) = spec.rules {
            rules.iter().for_each(|x| {
                if let Some(http) = x.http.clone() {
                    http.paths.iter().for_each(|y| {
                        if let Some(ibs) = y.backend.clone().service {
                            let h = x.host.clone().unwrap();
                            let p = y.path.clone().unwrap();
                            let bn = format!("{}-{}", ing.metadata.clone().name.unwrap(), ibs.name);
                            let sp: u16 = ibs.port.unwrap().number.unwrap().try_into().unwrap();
                            let ns = ing.metadata.clone().namespace.unwrap();

                            let backend = Backend::new(ns, bn, h, p, ibs.name, sp);

                            info!(
                                "found backend [{}] from ingress [{}]",
                                backend.name,
                                ing.metadata.clone().name.unwrap()
                            );
                            backends.push(backend);
                        }
                    })
                }
            });
        }
    }

    Ok(backends)
}
