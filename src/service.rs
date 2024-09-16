use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1::IngressLoadBalancerIngress;
use std::cmp::Ordering;
use std::collections::HashSet;

use kube::runtime::watcher::Error as WatcherError;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use log::{error, info};

use crate::ingress::update_status;

pub async fn watch_service(
    client: Client,
    name: &str,
    namespace: &str,
) -> Result<(), WatcherError> {
    let service_api: Api<Service> = Api::namespaced(client.clone(), namespace);

    let mut observer = watcher(
        service_api,
        watcher::Config::default().labels("app=varnish-ingress-controller".to_string().as_str()),
    )
    .default_backoff()
    .boxed();

    info!(
        "Started watching service [{}] in namespace [{}]",
        name, namespace
    );

    while let Some(sv) = observer.try_next().await.unwrap() {
        if let watcher::Event::Apply(svc) = sv {
            match update_status_from_svc(svc).await {
                Ok(mut lbi) => {
                    info!("reading service [{}]", name);
                    lbi = sort_load_balancer_ingresses(lbi);
                    println!("{:?}", lbi);
                    if let Err(e) = update_status(client.clone(), lbi).await {
                        error!("Failed updating ingress status: {}", e);
                    }
                }
                Err(e) => {
                    error!("{}", e);
                }
            }
        }
    }
    Ok(())
}

async fn update_status_from_svc(svc: Service) -> Result<Vec<IngressLoadBalancerIngress>, String> {
    let spec = svc.spec.as_ref().ok_or("Service spec not found")?;

    match spec.type_.as_deref() {
        Some("ExternalName") => {
            let external_name = spec
                .external_name
                .as_ref()
                .ok_or("External name not found")?;
            info!("reading service type ExternalName");

            Ok(vec![IngressLoadBalancerIngress {
                hostname: Some(external_name.clone()),
                ip: None,
                ports: None,
            }])
        }

        Some("ClusterIP") => {
            let cluster_ip = spec.cluster_ip.as_ref().ok_or("Cluster IP not found")?;
            info!("reading service type ClusterIP");

            Ok(vec![IngressLoadBalancerIngress {
                ip: Some(cluster_ip.clone()),
                hostname: None,
                ports: None,
            }])
        }

        Some("NodePort") => {
            let cluster_ip = spec.cluster_ip.as_ref().ok_or("Cluster IP not found")?;
            let external_ips = spec.external_ips.as_deref().unwrap_or(&[]);

            info!("reading service type NodePort");

            if external_ips.is_empty() {
                return Ok(vec![IngressLoadBalancerIngress {
                    ip: Some(cluster_ip.clone()),
                    hostname: None,
                    ports: None,
                }]);
            }

            let addrs: Vec<IngressLoadBalancerIngress> = external_ips
                .iter()
                .map(|ip| IngressLoadBalancerIngress {
                    ip: Some(ip.clone()),
                    hostname: None,
                    ports: None,
                })
                .collect();

            Ok(addrs)
        }

        Some("LoadBalancer") => {
            let external_ips = spec.external_ips.as_deref().unwrap_or(&[]);
            let mut addrs: Vec<IngressLoadBalancerIngress> = vec![];

            info!("reading service type LoadBalancer");

            if let Some(status) = &svc.status {
                if let Some(load_balancer) = &status.load_balancer {
                    if let Some(ingresses) = &load_balancer.ingress {
                        addrs.extend(ingresses.iter().map(|ingress| IngressLoadBalancerIngress {
                            ip: ingress.ip.clone(),
                            hostname: ingress.hostname.clone(),
                            ports: None,
                        }));
                    }
                }
            }

            let existing_ips: HashSet<String> = addrs.iter().filter_map(|a| a.ip.clone()).collect();

            for ip in external_ips {
                if !existing_ips.contains(ip) {
                    addrs.push(IngressLoadBalancerIngress {
                        ip: Some(ip.clone()),
                        hostname: None,
                        ports: None,
                    });
                }
            }

            Ok(addrs)
        }

        Some(unknown_type) => Err(format!("Unknown service type: [{}]", unknown_type)),

        None => Err("Service type not specified".to_string()),
    }
}

fn sort_load_balancer_ingresses(
    mut lbi: Vec<IngressLoadBalancerIngress>,
) -> Vec<IngressLoadBalancerIngress> {
    lbi.sort_by(|a, b| match (&a.ip, &b.ip) {
        (Some(ip_a), Some(ip_b)) => ip_a.cmp(ip_b),
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (None, None) => Ordering::Equal,
    });

    lbi
}
