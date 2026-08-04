use clap::Parser;
use cli::Args;
use configmap::watch_configmap;
use env_logger::Env;
use ingress::watch_ingresses;
use kube::Client;
use leader::run_leader_election;
use log::error;
use service::watch_service;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::join;
use varnish::{Varnish, start};
use vcl::Vcl;

mod cli;
mod configmap;
mod ingress;
mod leader;
mod service;
mod varnish;
mod varnishlog;
mod varnishstat;
mod vcl;

const VARNISH_BIN: &str = "varnishd";

#[tokio::main]
async fn main() {
    let args = Args::parse();

    env_logger::Builder::from_env(Env::default().default_filter_or(&args.log_level)).init();

    let v = Varnish {
        cmd: VARNISH_BIN.to_string(),
        port: args.http_port.clone(),
        vcl: args.vcl_file.clone(),
        work_dir: args.work_folder.clone(),
        params: args.params.clone(),
        default_ttl: args.default_ttl.clone(),
        storage: args.storage.clone(),
    };

    if let Err(e) = start(&v).await {
        error!("Failed to start Varnish: {e}");
        process::exit(1);
    }

    let varnish_work_folder = args.work_folder.clone();
    let wfc = varnish_work_folder.clone();

    tokio::spawn(async move {
        varnishlog::start(&varnish_work_folder).await;
    });

    tokio::spawn(async move {
        varnishstat::start(&wfc).await;
    });

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            error!("Could not init k8s client: {e}");
            process::exit(1);
        }
    };

    let vcl = Vcl::new(
        args.vcl_file,
        args.template,
        args.work_folder.clone(),
        args.vcl_recv_snippet,
        args.vcl_snippet,
    );

    let arc_vcl = Arc::new(Mutex::new(vcl));

    let leader_status = Arc::new(AtomicBool::new(false));

    let leader_future = run_leader_election(leader_status.clone(), client.clone());

    let service_future = watch_service(
        leader_status.clone(),
        client.clone(),
        "varnish-ingress-service",
        &args.namespace,
    );
    let ingress_future = watch_ingresses(client.clone(), &arc_vcl, &args.ingress_class);
    let configmap_future = watch_configmap(client, &arc_vcl, &args.namespace);

    let (leader_result, service_result, ingress_result, configmap_result) = join!(
        leader_future,
        service_future,
        ingress_future,
        configmap_future
    );

    if let Err(e) = leader_result {
        error!("Error establishing the leader: {e}");
    }

    if let Err(e) = service_result {
        error!("Error watching service: {e}");
    }

    if let Err(e) = ingress_result {
        error!("Error watching ingresses: {e}");
    }

    if let Err(e) = configmap_result {
        error!("Error watching configmap: {e}");
    }
}
