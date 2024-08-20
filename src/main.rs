use configmap::watch_configmap;
use ingress::watch_ingresses;
use log::{error, info};
use std::{cell::RefCell, rc::Rc};

use clap::Parser;
use kube::Client;
use std::process;
use tokio::join;
use varnish::{start, Varnish};
use vcl::Vcl;

mod configmap;
mod ingress;
mod varnish;
mod vcl;
mod vcl_test;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value = "info",
        help = "Sets the log level of the varnish ingress controller"
    )]
    log_level: String,

    #[arg(
        long,
        default_value = "/etc/varnish/default.vcl",
        env = "VARNISH_VCL",
        help = "Sets the path to Varnish's default vcl file (the equivalent of Varnish's [-f] param)"
    )]
    vcl_file: String,

    #[arg(
        long,
        default_value = "./template/vcl.hbs",
        help = "Sets the path to the template file used to generate the VCL"
    )]
    template: String,

    #[arg(
        long,
        default_value = "varnish",
        help = "Sets the ingress class that controller will be looking for"
    )]
    ingress_class: String,

    #[arg(
        long,
        default_value = "/etc/varnish",
        env = "VARNISH_WORK_FOLDER",
        help = "Sets the working folder for the running Varnish instance\
             (the equivalent of Varnish's [-n] param)"
    )]
    work_folder: String,

    #[arg(
        long,
        default_value = "",
        env = "VARNISH_PARAMS",
        help = "Extra parameters sent to Varnish (the equivalent of Varnish's [-p] param)"
    )]
    params: String,

    #[arg(
        long,
        default_value = "6081",
        env = "VARNISH_HTTP_PORT",
        help = "The http port at which Varnish will run"
    )]
    http_port: String,

    #[arg(
        long,
        env = "VARNISH_DEFAULT_TTL",
        default_value = "120s",
        help = "Default TTL for cached objects (the equivalent of Varnish's [-t] param)"
    )]
    default_ttl: String,

    #[arg(
        long,
        env = "VARNISH_VCL_SNIPPET",
        default_value = "",
        help = "Extra VCL code to be added at the end of the generated VCL"
    )]
    vcl_snippet: String,

    #[arg(
        long,
        env = "NAMESPACE",
        default_value = "default",
        help = "The namespace where Varnish Ingress Controller operates in"
    )]
    namespace: String,
}

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
    };

    match start(&v) {
        Ok(pid) => info!("Varnish process started with pid: {}", pid),
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    }

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
        args.vcl_snippet,
    );

    let rc_vcl = Rc::new(RefCell::new(vcl));

    let ingress_future = watch_ingresses(client.clone(), &rc_vcl, &args.ingress_class);
    let configmap_future = watch_configmap(client, &rc_vcl, &args.namespace);

    let (ingress_result, configmap_result) = join!(ingress_future, configmap_future);

    if let Err(e) = ingress_result {
        error!("Error watching ingresses: {}", e);
    }

    if let Err(e) = configmap_result {
        error!("Error watching configmap: {}", e);
    }
}
