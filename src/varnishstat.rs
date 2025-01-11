use futures::AsyncReadExt;
use log::{error, info};
use opentelemetry::metrics::Meter;
use opentelemetry::KeyValue;
use rocket::{get, routes, Ignite, Rocket, State};
use serde::Deserialize;
use std::sync::Arc;
use std::{collections::HashMap, io::BufWriter};
use tokio::join;
use tokio::process::Command;
use tokio::sync::Mutex;

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, Registry, TextEncoder};

#[derive(Deserialize, Debug)]
struct Stats {
    counters: HashMap<String, VarnishCounter>,
}

#[derive(Debug, Deserialize)]
struct VarnishCounter {
    description: String,
    flag: String,
    format: String,
    value: u64,
}

pub async fn start(work_dir: &str) {
    let registry = prometheus::Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()
        .unwrap();
    let provider = SdkMeterProvider::builder().with_reader(exporter).build();
    let meter = provider.meter("varnish");

    let varnishstat_task = run_varnishstat(work_dir);

    let shared_meter = Arc::new(Mutex::new(meter));
    let shared_stats_registry = Arc::new(Mutex::new(registry));

    let server_task = launch_rocket(shared_meter, shared_stats_registry);

    let (server_result, varnishstat_result) = join!(server_task, varnishstat_task);

    if let Err(e) = server_result {
        error!("Failed launching Rocket: {}", e);
    }

    if let Err(e) = varnishstat_result {
        error!("Failed launching the varnishstat loop: {}", e);
    }
}

async fn run_varnishstat(work_dir: &str) -> Result<String, String> {
    let args: &[&str] = &[
        "-n",
        work_dir,
        "-f",
        "MAIN.cache_hit",
        "-f",
        "MAIN.cache_miss",
        "-f",
        "MAIN.uptime",
        "-j",
    ];

    info!("Running varnishstat with args: {:?}", args);

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
    shared_meter: Arc<Mutex<Meter>>,
    shared_stats_registry: Arc<Mutex<Registry>>,
) -> Result<Rocket<Ignite>, rocket::Error> {
    info!("Starting the varnishstat exporter server");

    rocket::build()
        .manage(shared_meter)
        .manage(shared_stats_registry)
        .mount("/", routes![metrics])
        .launch()
        .await
}

#[get("/metrics")]
async fn metrics(
    meter: &State<Arc<Mutex<Meter>>>,
    registry: &State<Arc<Mutex<Registry>>>,
) -> Result<String, String> {
    let out = run_varnishstat("/etc/varnish").await;

    let vs: Stats = serde_json::from_str(&out.unwrap()).unwrap();

    let cache_counter = meter
        .lock()
        .await
        .u64_gauge("main_counter")
        .with_description("Varnish main.* counters")
        .build();

    for (k, v) in vs.counters {
        let countet_label = &[KeyValue::new("main", k)];
        cache_counter.record(v.value, countet_label);
    }

    let encoder = TextEncoder::new();
    let metric_families = registry.lock().await.gather();
    let mut result = BufWriter::new(vec![]);
    let _ = encoder.encode(&metric_families, &mut result);

    let mut out = String::with_capacity(result.buffer().len());

    result.buffer().read_to_string(&mut out).await.unwrap();

    Ok(out)
}
