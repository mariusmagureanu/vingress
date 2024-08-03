use log::{error, info};

use clap::Parser;
use kube::Client;
use std::process;

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
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    std::env::set_var("RUST_LOG", args.log_level);

    env_logger::init();

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
