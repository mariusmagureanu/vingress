use futures::AsyncReadExt;
use log::{debug, error, info};
use rocket::{get, routes, Ignite, Rocket, State};
use std::io::BufWriter;
use std::sync::Arc;
use tokio::join;
use tokio::process::Command;
use tokio::sync::Mutex;

use opentelemetry::{metrics::Counter, metrics::MeterProvider, KeyValue};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, Registry, TextEncoder};

pub async fn start(work_dir: &str) {
    let registry = prometheus::Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()
        .unwrap();
    let provider = SdkMeterProvider::builder().with_reader(exporter).build();
    let meter = provider.meter("varnish");

    let main_counter = meter
        .u64_counter("main_counter")
        .with_description("Varnish main.* counters")
        .build();

    let varnishstat_task = run_varnishstat(work_dir);

    let shared_stats = Arc::new(Mutex::new(main_counter));
    let shared_stats_registry = Arc::new(Mutex::new(registry));

    let server_task = launch_rocket(shared_stats, shared_stats_registry);

    let (server_result, varnishstat_result) = join!(server_task, varnishstat_task);

    if let Err(e) = server_result {
        error!("Failed launching Rocket: {}", e);
    }

    if let Err(e) = varnishstat_result {
        error!("Failed launching the varnishstat loop: {}", e);
    }
}

async fn run_varnishstat(work_dir: &str) -> Result<String, String> {
    let args: &[&str] = &["-n", work_dir, "-j"];

    info!("Running [varnishtat -n {} -j] ", work_dir);

    debug!("Running varnishstat with args: {:?}", args);

    match Command::new("varnishstat").args(args).output().await {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Varnishstat error: {}", stderr);
            Err(stderr.to_string())
        }
        Err(e) => {
            error!("Failed to execute varnishstat: {}", e);
            Err(e.to_string())
        }
    }
}

async fn launch_rocket(
    shared_stats: Arc<Mutex<Counter<u64>>>,
    shared_stats_registry: Arc<Mutex<Registry>>,
) -> Result<Rocket<Ignite>, rocket::Error> {
    info!("Starting the varnishstat exporter server");

    rocket::build()
        .manage(shared_stats)
        .manage(shared_stats_registry)
        .mount("/", routes![metrics])
        .launch()
        .await
}

#[get("/metrics")]
async fn metrics(
    main_counter: &State<Arc<Mutex<Counter<u64>>>>,
    registry: &State<Arc<Mutex<Registry>>>,
) -> Result<String, String> {
    main_counter
        .lock()
        .await
        .add(100, &[KeyValue::new("key", "value")]);

    let encoder = TextEncoder::new();
    let metric_families = registry.lock().await.gather();
    let mut result = BufWriter::new(vec![]);
    let _ = encoder.encode(&metric_families, &mut result);

    let mut out = String::with_capacity(result.buffer().len());

    result.buffer().read_to_string(&mut out).await.unwrap();

    Ok(out)
}
