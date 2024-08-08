[![Build master](https://github.com/mariusmagureanu/vingress/actions/workflows/rust.yml/badge.svg)](https://github.com/mariusmagureanu/vingress/actions/workflows/rust.yml)

### Varnish Ingress controller
Lite implementation of a Varnish Ingress controller.

---

### How does it work

The ``varnish-ingress-controller`` watches over Ingress objects in the cluster. The watcher is configured to
filter through Ingress objects with the following label:

```
kubernetes.io/ingress-class=varnish
```

Also, all filtered Ingress objects must have their ``spec.ingressClassName`` set to ``varnish`` too. The spec of the Ingress objects 
is then translated into Varnish [VCL](https://varnish-cache.org/docs/trunk/users-guide/vcl.html).

The ``varnish-ingress-controller`` watches over ``INIT | ADD | UPDATE | DELETE`` Ingress events and updates
the Varnish VCL accordingly. After a succesfull VCL file update, Varnish will reload its VCL just so it becomes aware of the latest configuration.


Example:

This Ingress spec:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  generation: 4
  labels:
    kubernetes.io/ingress: varnish
  name: media
  namespace: demo
  resourceVersion: "146788664"
  uid: b386a268-0006-446c-9844-3e004712070f
spec:
  ingressClassName: varnish
  rules:
  - host: foo.bar.com
    http:
      paths:
      - backend:
          service:
            name: media-v1-svc
            port:
              number: 80
        path: /foo
        pathType: Prefix
  - host: qux.bar.com
    http:
      paths:
      - backend:
          service:
            name: media-v2-svc
            port:
              number: 80
        path: /qux
        pathType: Exact
```

yields the following VCL:

```
vcl 4.1;

import directors;
import std;

backend default none;

backend demo-media-media-v1-svc {
  .host = "media-v1-svc.demo.svc.cluster.local";
  .port = "80";
}
  
backend demo-media-media-v2-svc {
  .host = "media-v2-svc.demo.svc.cluster.local";
  .port = "80";
}
  

sub vcl_recv {
 if (req.http.host == "foo.bar.com" && req.url ~ "^/foo") {
        set req.backend_hint = demo-media-media-v1-svc;
    }
 if (req.http.host == "qux.bar.com" && req.url == "/qux") {
        set req.backend_hint = demo-media-media-v2-svc;
    }
}
```

---

### Installation and usage

At the time of writing this, the installation is available only via Helm from your local machine.
Make sure you're connected to a cluster and run the following:

```sh
$ helm package chart/
$ helm upgrade varnish-ingress-controller --install --namespace <your-namespace> --create-namespace ./varnish-ingress-controller-0.1.0.tgz
```

Update the spec of your Ingress(es) with the following requirements:

1. add the following label: ``kubernetes.io/ingress-class: varnish``
2. set the ingress class: ``spec.ingressClassName: varnish``

Investigate the logs of the varnish-controller pod, they should reflect the updates mentioned above on your Ingress object(s):

```sh
$ kubectl -n <your-namespace> logs po/varnish-ingress-controller-xxxxxxxxxx-yyyyy -c varnish-controller
```

A Kubernetes service is available to be used for reaching the Varnish containers. It is up to you whether this service
should be used in conjuction with a load-balancer or not. 
For quick testing and short feedback loops it's easier to just port forward it locally:

```
$ kubectl -n <your-namespace> port-forward svc/varnish-ingress-service 8081:8081
$ curl http://127.1:8081/foo -H "Host: foo.bar.com" -v
```

