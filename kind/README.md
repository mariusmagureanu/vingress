#### KinD

If you don't have a test cluster at hand, you're welcome to use [kind](https://kind.sigs.k8s.io/).

Install KinD:

```sh
$ go install sigs.k8s.io/kind@v0.23.0 
```

Create the cluster:

```sh
$ kind create cluster --config kind/cluster.yaml 
```

Create some test infra: pods, services, ingresses ..etc

```sh
$ kubectl apply -f media.yaml
$ kubectl apply -f smp.yaml
```

Install the ``varnish-ingress-controller``:

```sh
$ helm package chart/
$ helm upgrade varnish-ingress-controller --install --namespace vingress --create-namespace ./varnish-ingress-controller-0.3.1.tgz -f chart/values.yaml
```

Port forward the ``varnish-ingress-controller`` service:

```sh
$ kubectl -n vingress port-forward svc/varnish-ingress-service 8081:8081
```


Run a couple ``curl`` of test requests:

```sh
$ curl 127.1:8081/v1 -H "Host: media.example.com" -v
$ curl 127.1:8081/v2 -H "Host: media.example.com" -v
$ curl 127.1:8081/smp -H "Host: smp.example.com" -v
```
