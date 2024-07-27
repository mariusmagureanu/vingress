use futures::TryStreamExt;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    runtime::{watcher, WatchStreamExt},
    Api, Client,
};
use std::pin::pin;
use vcl::{update, Backend, Vcl};

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

        let ing_class = n.spec.clone().unwrap().ingress_class_name;

        if ing_class.is_none() {
            continue;
        }

        let class_name = ing_class.unwrap().to_lowercase();

        if class_name != "varnish".to_string() {
            continue;
        }

        let mut backends: Vec<Backend> = vec![];
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
        });

        let mut v = Vcl::new("default.vcl", "./template/vcl.hbs");
        update(&mut v, backends);
    }
}
