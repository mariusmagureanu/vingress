use log::{error, info};
use opentelemetry::KeyValue;
use opentelemetry::metrics::Gauge;
use rocket::{Ignite, Rocket, State, get, routes};
use serde::Deserialize;
use std::sync::Arc;
use std::{collections::HashMap, io::BufWriter};
use tokio::process::Command;
use tokio::sync::Mutex;

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, Registry, TextEncoder};

const VARNISH_STAT_BIN: &str = "varnishstat";

#[derive(Deserialize)]
struct Stats {
    counters: HashMap<String, VarnishCounter>,
}

#[derive(Deserialize)]
struct VarnishCounter {
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

    let cache_counter = meter
        .u64_gauge("main_counter")
        .with_description("Varnish main.* counters")
        .build();

    let shared_gauge = Arc::new(cache_counter);
    let shared_stats_registry = Arc::new(Mutex::new(registry));

    let server_task = launch_rocket(shared_gauge, shared_stats_registry, work_dir);

    if let Err(e) = server_task.await {
        error!("Could not start Rocket: {e:?}")
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
        "MAIN.client_req",
        "-f",
        "MAIN.backend_conn",
        "-f",
        "MAIN.threads",
        "-f",
        "MAIN.n_object",
        "-f",
        "MAIN.n_backend",
        "-f",
        "MAIN.uptime",
        "-f",
        "MAIN.backend_req",
        "-f",
        "MAIN.n_vcl",
        "-j",
    ];

    info!("Running varnishstat with args: {args:?}");

    match Command::new(VARNISH_STAT_BIN).args(args).output().await {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Varnishstat error: {stderr}");
            Err(stderr.to_string())
        }
        Err(e) => {
            error!("Failed to execute varnishstat: {e}");
            Err(e.to_string())
        }
    }
}

async fn launch_rocket(
    shared_gauge: Arc<Gauge<u64>>,
    shared_stats_registry: Arc<Mutex<Registry>>,
    shared_work_dir: &str,
) -> Result<Rocket<Ignite>, rocket::Error> {
    info!("Starting the varnishstat exporter server");

    rocket::build()
        .manage(shared_gauge)
        .manage(shared_stats_registry)
        .manage(String::from(shared_work_dir))
        .mount("/", routes![metrics])
        .launch()
        .await
}

#[get("/metrics")]
async fn metrics(
    gauge: &State<Arc<Gauge<u64>>>,
    registry: &State<Arc<Mutex<Registry>>>,
    work_dir: &State<String>,
) -> Result<String, String> {
    let varnish_output = match run_varnishstat(work_dir).await {
        Ok(s) => s,
        Err(e) => {
            error!("failed to run varnishstat: {e}");
            return Err(e);
        }
    };

    let varnish_stats: Stats = match serde_json::from_str(&varnish_output) {
        Ok(s) => s,
        Err(e) => {
            error!("failed to Deserialize varnishstats: {e}");
            return Err(e.to_string());
        }
    };

    for (key, value) in varnish_stats.counters {
        let label = &[KeyValue::new("main", key)];
        gauge.record(value.value, label);
    }

    let encoder = TextEncoder::new();
    let registry_guard = registry.lock().await;
    let metric_families = registry_guard.gather();
    drop(registry_guard);

    let mut buffer = BufWriter::new(Vec::new());
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("failed to encode metrics: {e}");
        return Err(e.to_string());
    }

    match String::from_utf8(buffer.into_inner().unwrap_or_default()) {
        Ok(r) => Ok(r),
        Err(e) => {
            error!("failed to convert metrics: {e}");
            Err(e.to_string())
        }
    }
}
