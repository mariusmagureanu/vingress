use log::{debug, error, info};
use rocket::{get, routes, Ignite, Rocket, State};
use std::sync::Arc;
use tokio::join;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub async fn start(work_dir: &str, interval: u64) {
    let shared_stats = Arc::new(Mutex::new(String::new()));

    let varnishstat_task = run_varnishstat(work_dir, interval, Arc::clone(&shared_stats));
    let server_task = start_server(shared_stats);

    let (server_result, varnishstat_result) = join!(server_task, varnishstat_task);

    if let Err(e) = server_result {
        error!("Failed launching Rocket: {}", e);
    }

    if let Err(e) = varnishstat_result {
        error!("Failed launching the varnishstat loop: {}", e);
    }
}

async fn run_varnishstat(
    work_dir: &str,
    interval: u64,
    shared_data: Arc<Mutex<String>>,
) -> Result<(), String> {
    let args: &[&str] = &["-n", work_dir, "-j"];

    info!(
        "Started running [varnishtat -n {} -j] every [{}] seconds",
        work_dir, interval
    );

    loop {
        debug!("Running varnishstat with args: {:?}", args);

        match Command::new("varnishstat").args(args).output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut data = shared_data.lock().await;
                *data = stdout.into_owned();
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Varnishstat error: {}", stderr);
            }
            Err(e) => {
                error!("Failed to execute varnishstat: {}", e);
            }
        }

        sleep(Duration::from_secs(interval)).await;
    }
}

async fn start_server(shared_data: Arc<Mutex<String>>) -> Result<Rocket<Ignite>, rocket::Error> {
    info!("Starting the varnishstat exporter server");

    rocket::build()
        .manage(shared_data)
        .mount("/", routes![stats])
        .launch()
        .await
}

#[get("/")]
async fn stats(shared_data: &State<Arc<Mutex<String>>>) -> Result<String, String> {
    debug!("Handling index request");

    let data = shared_data.lock().await;
    Ok(data.clone())
}
