use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::chrono::Utc;
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    Client,
};
use log::{error, info};
use serde_json::json;
use std::env;
use tokio::time::{sleep, Duration};

pub async fn run_leader_election(client: Client) -> Result<(), Box<dyn std::error::Error>> {
    let pod_name = env::var("POD_NAME")?;
    let namespace = env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

    let leases: Api<Lease> = Api::namespaced(client.clone(), &namespace);

    let lease_name = "vingress-election-lock";

    loop {
        match try_acquire_leadership(&leases, &lease_name, &pod_name).await {
            Ok(true) => {
                info!("I am the leader: {}", pod_name);
                maintain_leadership(&leases, &lease_name, &pod_name).await;
            }
            Ok(false) => {
                info!("Waiting for leadership...");
                sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                error!("Error during leader election: {:?}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn try_acquire_leadership(
    leases: &Api<Lease>,
    lease_name: &str,
    pod_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let lease = leases.get_opt(lease_name).await?;

    if let Some(existing_lease) = lease {
        // Check if current leader has expired
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
            // If the leader has not renewed in the last 15 seconds, try to take over
            update_lease(leases, lease_name, pod_name).await?;
            return Ok(true);
        }

        Ok(false)
    } else {
        // Lease does not exist, create it
        create_lease(leases, lease_name, pod_name).await?;
        Ok(true)
    }
}

async fn create_lease(
    leases: &Api<Lease>,
    lease_name: &str,
    pod_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_lease = json!({
        "metadata": {
            "name": lease_name,
        },
        "spec": {
            "holderIdentity": pod_name,
            "leaseDurationSeconds": 15,
            "renewTime": Utc::now(),
        }
    });

    leases
        .create(&PostParams::default(), &serde_json::from_value(new_lease)?)
        .await?;
    Ok(())
}

async fn update_lease(
    leases: &Api<Lease>,
    lease_name: &str,
    pod_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let patch = json!({
        "spec": {
            "holderIdentity": pod_name,
            "leaseDurationSeconds": 15,
            "renewTime": Utc::now(),
        }
    });

    leases
        .patch(
            lease_name,
            &PatchParams::apply("leader-election"),
            &Patch::Merge(&patch),
        )
        .await?;
    Ok(())
}

async fn maintain_leadership(leases: &Api<Lease>, lease_name: &str, pod_name: &str) {
    loop {
        match update_lease(leases, lease_name, pod_name).await {
            Ok(_) => {
                info!("Leadership maintained by: {}", pod_name);
            }
            Err(e) => {
                error!("Failed to maintain leadership: {:?}", e);
                break;
            }
        }
        sleep(Duration::from_secs(10)).await; // Renew lease every 10 seconds
    }
}
