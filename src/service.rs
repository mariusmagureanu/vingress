use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::{Service, ServiceSpec};
use k8s_openapi::api::networking::v1::IngressLoadBalancerIngress;

use kube::runtime::watcher::Error as WatcherError;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use log::{debug, error, info};

pub async fn watch_service(
    client: Client,
    name: &str,
    namespace: &str,
) -> Result<(), WatcherError> {
    let service_api: Api<Service> = Api::namespaced(client, namespace);

    let mut observer = watcher(
        service_api,
        watcher::Config::default().labels("app=varnish-ingress-controller".to_string().as_str()),
    )
    .default_backoff()
    .boxed();

    while let Some(sv) = observer.try_next().await.unwrap() {
        if let watcher::Event::Apply(svc) = sv { match update_status_from_svc(svc).await {
            Ok(_lbi) => {
                info!("reading service [{}]", name);
            }
            Err(e) => {
                error!("{}", e);
            }
        } }
    }
    Ok(())
}

async fn update_status_from_svc(svc: Service) -> Result<Vec<IngressLoadBalancerIngress>, String> {
    match svc.spec {
        Some(ServiceSpec {
            type_: Some(ref svc_type),
            external_name: Some(ref external_name),
            cluster_ip: Some(ref _cluster_ip),
            external_ips: Some(ref _external_ips),
            ..
        }) if svc_type == "ExternalName" => {
            debug!("reading service type ExternalName");
            Ok(vec![IngressLoadBalancerIngress {
                hostname: Some(external_name.clone()),
                ip: None,
                ports: None,
            }])
        }
        Some(ServiceSpec {
            type_: Some(ref svc_type),
            cluster_ip: Some(ref cluster_ip),
            ..
        }) if svc_type == "ClusterIP" => {
            debug!("reading service type ClusterIP");
            Ok(vec![IngressLoadBalancerIngress {
                ip: Some(cluster_ip.clone()),
                hostname: None,
                ports: None,
            }])
        }
        Some(ServiceSpec {
            type_: Some(ref svc_type),
            external_ips: Some(ref external_ips),
            cluster_ip: Some(ref cluster_ip),
            ..
        }) if svc_type == "NodePort" => {
            debug!("reading service type NodePort");
            if external_ips.is_empty() {
                return Ok(vec![IngressLoadBalancerIngress {
                    ip: Some(cluster_ip.clone()),
                    hostname: None,
                    ports: None,
                }]);
            }
            let mut addrs = vec![];
            for ip in external_ips {
                addrs.push(IngressLoadBalancerIngress {
                    ip: Some(ip.clone()),
                    hostname: None,
                    ports: None,
                });
            }
            Ok(addrs)
        }
        _ => Err(String::from("unknown service type")),
    }
}
