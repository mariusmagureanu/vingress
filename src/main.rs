use log::{error, info};

use clap::Parser;
use kube::Client;
use std::process;
use vcl::{start_varnish, Varnish};

mod ingress;
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

    #[arg(short, long, default_value = "")]
    params: String,

    #[arg(short, long, default_value = ":6081")]
    address: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    std::env::set_var("RUST_LOG", args.log_level);

    env_logger::init();

    let v = Varnish {
        cmd: "varnishd",
        address: &args.address,
        vcl: &args.vcl,
        work_dir: &args.working_folder,
        params: &args.params,
    };

    match start_varnish(&v) {
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

    info!("Started watching ingresses of class: [{}]", args.class);

    ingress::watch_ingresses(
        client,
        &args.vcl,
        &args.template,
        &args.working_folder,
        &args.class,
    )
    .await;
}
