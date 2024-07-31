use log::{debug, error, info, warn};

use clap::Parser;
use futures::TryStreamExt;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::pin::pin;
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

    info!("begin watching ingress of class: {}", args.class);

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

        if class_name != ingress_class_name {
            debug!("skipping ingress class {}", class_name);
            continue;
        }

        let mut backends: Vec<Backend> = vec![];

        if let Some(spec) = n.spec {
            if let Some(rules) = spec.rules {
                rules.iter().for_each(|x| {
                    if let Some(http) = x.http.clone() {
                        http.paths.iter().for_each(|y| {
                            if let Some(ibs) = y.backend.clone().service {
                                let h = x.host.clone().unwrap();
                                let p = y.path.clone().unwrap();
                                let bn =
                                    format!("{}-{}", n.metadata.clone().name.unwrap(), ibs.name);
                                let sp: u16 = ibs.port.unwrap().number.unwrap().try_into().unwrap();
                                let ns = n.metadata.clone().namespace.unwrap();

                                let backend = Backend::new(ns, bn, h, p, ibs.name, sp);

                                debug!("adding backend {}", backend.name);
                                backends.push(backend);
                            }
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

        let mut v = Vcl::new(vcl_file, vcl_template, working_folder);

        match update(&mut v, backends) {
            None => {}
            Some(e) => error!("{}", e),
        }

        match reload(&v) {
            None => {}
            Some(e) => error!("{}", e),
        }
    }
}
