use futures::TryStreamExt;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::pin::pin;

mod vcl;
mod vcl_test;

#[tokio::main]
async fn main() {
    let client = Client::try_default().await.unwrap();

    let ings: Api<Ingress> = Api::all(client);

    let wc = watcher::Config::default();
    let obs = watcher(ings, wc).default_backoff().applied_objects();

    let mut obs = pin!(obs);

    while let Some(n) = obs.try_next().await.unwrap() {
        println!("{:?}", n);
    }
}
