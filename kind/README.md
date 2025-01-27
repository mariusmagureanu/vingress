### KinD

If you don't have a test cluster at hand, you're welcome to use [kind](https://kind.sigs.k8s.io/).

**Install KinD**:

```sh
$ go install sigs.k8s.io/kind@v0.26.0 
```

**Create the cluster**:

```sh
$ kind create cluster --config kind/cluster.yaml 
```

**Create some test infra: pods, services, ingresses ..etc**:

```sh
$ kubectl apply -f media.yaml
$ kubectl apply -f smp.yaml
$ kubectl apply -f full.yaml
$ kubectl apply -f prometheus.yaml
$ kubectl apply -f grafana.yaml
```

Install the ``varnish-ingress-controller``:

```sh
$ helm package chart/
$ helm upgrade varnish-ingress-controller --install --namespace vingress --create-namespace ./varnish-ingress-controller-0.3.2.tgz -f charts/values.yaml
```

Port forward the ``varnish-ingress-controller`` service:

```sh
$ kubectl -n vingress port-forward svc/varnish-ingress-service 6081:80
```


Run a couple ``curl`` of test requests:

```sh
$ curl 127.1:6081/v1 -H "Host: media.example.com"
$ curl 127.1:6081/v2 -H "Host: media.example.com"
$ curl 127.1:6081/smp -H "Host: smp.example.com" 
```

---

### Grafana 

The ``varnish-ingress-controller`` exposes a couple of varnishstat [counters](https://varnish-cache.org/docs/trunk/reference/varnish-counters.html#main-main-counters):

* MAIN.backend_conn
* MAIN.backend_req 
* MAIN.cache_hit
* MAIN.cache_miss
* MAIN.client_req
* MAIN.n_backend
* MAIN.n_object
* MAIN.n_vcl
* MAIN.threads
* MAIN.uptime 


If the ``grafana.yaml`` manifest has been applied, run the following to expose the running the Grafana instance:

```shell
$ kubectl -n monitoring port-forward svc/grafana 3000:3000
```

The default username and password is ``admin`` and ``admin``.

Head over to ```http://localhost:3000``` in your browser, log into Grafana and create a new Prometheus datasource using this url for it:

```yaml
http://prometheus.monitoring.svc.cluster.local:9090
```

--- 

### Development

With KinD it is possible to load local built Docker images into the cluster. This way it's rather convenient to test while under development.

However, the ``varnish-ingress-controller`` deployment needs a couple of tweaks:

```shell
$ kubectl -n vingress edit deploy/varnish-ingress-controller
```

Edit the ``image`` and ``imagePullPolicy`` as follows:

```yaml
image: vingress-dev:latest
imagePullPolicy: Never
```

Every time new code updates should be run and tested, run the ``test.sh`` shell script from the root of the repository.

The script builds a new Docker image, loads it into the cluster and restarts the ``varnish-ingress-controller``.

--- 

As you keep loading images into the cluster, its storage may run out. It's a good idea
you prune unused images.

> [!NOTE]
> This affects all your docker images


```sh
docker system prune -a --volumes
```
