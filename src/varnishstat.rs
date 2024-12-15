use log::info;
use rocket::get;
use rocket::routes;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

pub async fn start(work_dir: &str) {
    let args: Vec<&str> = vec!["-n", work_dir, "-j"];

    //  let _s = Arc::new(RefCell::new(String::new()));

    start_server().await;

    loop {
        run_varnishstat(&args).await;
        sleep(Duration::from_secs(1)).await;
    }
}

async fn run_varnishstat(args: &Vec<&str>) {
    info!("Running Varnishstat with the following args: {:?}", args);

    let mut child = Command::new("varnishstat")
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start Varnishstat");

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut stats = String::new();
        let _ = reader.buffer().read_to_string(&mut stats).await;
    };
}

async fn start_server() {
    info!("Starting the stat exporter server");

    let _ = rocket::build().mount("/", routes![index]).launch().await;
}

#[get("/")]
async fn index() -> String {
    info!("running index");

    let args = vec!["-n", "/etc/varnish", "-j"];

    let mut child = Command::new("varnishstat")
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start Varnishstat");

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut stats = String::new();
        let _ = reader.buffer().read_to_string(&mut stats).await;
        stats
    } else {
        String::from("foo bar")
    }
}
