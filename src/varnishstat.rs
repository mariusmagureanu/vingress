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
    let varnish_output = match run_varnishstat("/etc/varnish").await {
        Ok(s) => s,
        Err(e) => {
            error!("failed to run varnishstat: {}", e);
            return Err(e);
        }
    };

    let varnish_stats: Stats = match serde_json::from_str(&varnish_output) {
        Ok(s) => s,
        Err(e) => {
            error!("failed to Deserialize varnishstats: {}", e);
            return Err(e.to_string());
        }
    };

    let meter_guard = meter.lock().await;
    let cache_counter = meter_guard
        .u64_gauge("main_counter")
        .with_description("Varnish main.* counters")
        .build();

    for (key, value) in varnish_stats.counters {
        let label = &[KeyValue::new("main", key)];
        cache_counter.record(value.value, label);
    }
    drop(meter_guard); // Release the lock early.

    let encoder = TextEncoder::new();
    let registry_guard = registry.lock().await;
    let metric_families = registry_guard.gather();
    drop(registry_guard); // Release the lock early.

    let mut buffer = BufWriter::new(Vec::new());
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("failed to encode metrics: {}", e.to_string());
        return Err(e.to_string());
    }

    match String::from_utf8(buffer.into_inner().unwrap_or_default()) {
        Ok(r) => Ok(r),
        Err(e) => {
            error!("failed to convert metrics: {}", e);
            return Err(e.to_string());
        }
    }
}
