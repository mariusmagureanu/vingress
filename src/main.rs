use futures::TryStreamExt;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::pin::pin;
use vcl::Backend;

mod vcl;
mod vcl_test;

#[tokio::main]
async fn main() {
    let client = Client::try_default().await.unwrap();

    let ings: Api<Ingress> = Api::all(client);

    let wc = watcher::Config::default();
    let obs = watcher(ings, wc).default_backoff().applied_objects();

    let mut obs = pin!(obs);

    let mut backends: Vec<Backend> = vec![];

    while let Some(n) = obs.try_next().await.unwrap() {
        if n.spec
            .clone()
            .unwrap()
            .ingress_class_name
            .unwrap()
            .to_lowercase()
            != "varnish".to_string()
        {
            continue;
        }

        println!("{:?}", n.metadata.name);

        n.spec.unwrap().rules.unwrap().iter().for_each(|x| {
            x.http.clone().unwrap().paths.iter().for_each(|y| {
                let backend = Backend::new(
                    n.metadata.clone().name.unwrap(),
                    x.host.clone().unwrap(),
                    y.path.clone().unwrap(),
                    y.backend
                        .clone()
                        .service
                        .unwrap()
                        .port
                        .unwrap()
                        .number
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                backends.push(backend);
            })
        })
    }
}
