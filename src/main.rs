use clap::Parser;
use cli::Args;
use configmap::watch_configmap;
use ingress::watch_ingresses;
use kube::Client;
use leader::run_leader_election;
use log::error;
use service::watch_service;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use tokio::join;
use varnish::{start, Varnish};
use vcl::Vcl;

mod cli;
mod configmap;
mod ingress;
mod leader;
mod service;
mod varnish;
mod varnishlog;
mod varnishlog_test;
mod vcl;
mod vcl_test;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    std::env::set_var("RUST_LOG", args.log_level);

    env_logger::init();

    let v = Varnish {
        cmd: "varnishd",
        port: &args.http_port,
        vcl: &args.vcl_file,
        work_dir: &args.work_folder,
        params: &args.params,
        default_ttl: &args.default_ttl,
        storage: &args.storage,
    };

    start(&v).await;

    let varnish_work_folder = String::from(&args.work_folder);

    tokio::spawn(async move {
        varnishlog::start(&varnish_work_folder).await;
    });

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            error!("Could not init k8s client: {}", e);
            process::exit(1);
        }
    };

    let vcl = Vcl::new(
        &args.vcl_file,
        &args.template,
        &args.work_folder,
        args.vcl_recv_snippet,
        args.vcl_snippet,
    );

    let rc_vcl = Rc::new(RefCell::new(vcl));

    let leader_status = Arc::new(AtomicBool::new(false));

    let leader_future = run_leader_election(leader_status.clone(), client.clone());

    let service_future = watch_service(
        leader_status.clone(),
        client.clone(),
        "varnish-ingress-service",
        &args.namespace,
    );
    let ingress_future = watch_ingresses(client.clone(), &rc_vcl, &args.ingress_class);
    let configmap_future = watch_configmap(client, &rc_vcl, &args.namespace);

    let (leader_result, service_result, ingress_result, configmap_result) = join!(
        leader_future,
        service_future,
        ingress_future,
        configmap_future
    );

    if let Err(e) = leader_result {
        error!("Error establishing the leader: {}", e);
    }

    if let Err(e) = service_result {
        error!("Error watching service: {}", e);
    }

    if let Err(e) = ingress_result {
        error!("Error watching ingresses: {}", e);
    }

    if let Err(e) = configmap_result {
        error!("Error watching configmap: {}", e);
    }
}
