use chrono::{SecondsFormat, Utc};
use k8s_openapi::api::coordination::v1::Lease;
use kube::{
    Client,
    api::{Api, Patch, PatchParams, PostParams},
};
use log::{debug, error};
use serde_json::json;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, sleep};

const LEASE_NAME: &str = "vingress-leader-lock";

pub async fn run_leader_election(
    leader_status: Arc<AtomicBool>,
    client: Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let pod_name = env::var("POD_NAME")?;
    let namespace = env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

    let leases: Api<Lease> = Api::namespaced(client.clone(), &namespace);

    loop {
        match try_acquire_leadership(&leases, &pod_name).await {
            Ok(true) => {
                leader_status.store(true, Ordering::Relaxed);
                debug!("Current varnish-ingress-controller leader: {}", pod_name);
                maintain_leadership(&leases, &pod_name).await;
            }
            Ok(false) => {
                leader_status.store(false, Ordering::Relaxed);
                debug!("Waiting for leadership...");
                sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                leader_status.store(false, Ordering::Relaxed);
                error!("Error during leader election: {:?}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn try_acquire_leadership(
    leases: &Api<Lease>,
    pod_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let lease = leases.get_opt(LEASE_NAME).await?;

    if let Some(existing_lease) = lease {
        let renewal_time = existing_lease
            .spec
            .as_ref()
            .unwrap()
            .renew_time
            .as_ref()
            .unwrap()
            .0;
        let duration_since_last_renewal = Utc::now() - renewal_time;

        if duration_since_last_renewal.num_seconds() > 15 {
            update_lease(leases, pod_name).await?;
            return Ok(true);
        }

        Ok(false)
    } else {
        create_lease(leases, pod_name).await?;
        Ok(true)
    }
}

async fn create_lease(
    leases: &Api<Lease>,
    pod_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_lease = json!({
        "metadata": {
            "name": LEASE_NAME,
        },
        "spec": {
            "holderIdentity": pod_name,
            "leaseDurationSeconds": 15,
            "renewTime": Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
        }
    });

    leases
        .create(&PostParams::default(), &serde_json::from_value(new_lease)?)
        .await?;
    Ok(())
}

async fn update_lease(
    leases: &Api<Lease>,
    pod_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let patch = json!({
        "spec": {
            "holderIdentity": pod_name,
            "leaseDurationSeconds": 15,
            "renewTime": Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
        }
    });

    leases
        .patch(
            LEASE_NAME,
            &PatchParams::apply("leader-election"),
            &Patch::Merge(&patch),
        )
        .await?;
    Ok(())
}

async fn maintain_leadership(leases: &Api<Lease>, pod_name: &str) {
    loop {
        match update_lease(leases, pod_name).await {
            Ok(_) => {
                debug!("Leadership maintained by: {}", pod_name);
            }
            Err(e) => {
                error!("Failed to maintain leadership: {:?}", e);
                break;
            }
        }
        sleep(Duration::from_secs(10)).await; // Renew lease every 10 seconds
    }
}
