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
