### Synopsis

This Helm chart installs an Ingress Controller based on Varnish Cache.


---

### Table of contents

- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Add custom behaviour with VCL](#add-custom-behaviour-with-vcl)

---
### Installation

Connect to a Kubernetes cluster and run the following:


Add this repository to your Helm:

```sh
$ helm repo add varnish-ingress-controller https://mariusmagureanu.github.io/vingress/charts
```

Install the Chart:

```sh
$ helm install vingress --create-namespace -n varnish-ingress  varnish-ingress-controller/varnish-ingress-controller --version 0.1.0
```

---

### Usage

Update the spec of your Ingress(es) with the following requirements:

1. Add the following label: ``kubernetes.io/ingress: varnish``
2. Set the ingress class: ``spec.ingressClassName: varnish``

The Varnish Ingress controller watches only over Ingress objects which meet both of the conditions mentioned above.

Example:

The following Ingress spec:

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

```C
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

### Configuration

The Helm chart comes along with pretty much sane defaults. 
However, depending on the use case - the resource requests and/or limits need to be tweaked accordingly.


Snippet from a ``values.yaml`` file:

```yaml
resources:
  requests:
    memory: "4096Mi"
    cpu: "250m"
  limits:
    memory: "4096Mi"
    cpu: "1"
```

Pass any parameters to varnish using a similar snippet like the one below.

Snippet from a ``values.yaml`` file:

```yaml
varnish:
  httpPort: "6081"
  vclFile: "/etc/varnish/default.vcl"
  workFolder: "/etc/varnish"
  params: "-p thread_pool_min=5 -p thread_pool_max=500 -p thread_pool_timeout=300"
  defaultTtl: "240s"
```

---

### Add custom behaviour with [VCL](https://varnish-cache.org/docs/7.3/users-guide/vcl.html)

By default, the Ingress controller translates the Ingress spec into VCL. This translation results with 
the creation of Varnish [backends](https://varnish-cache.org/docs/trunk/users-guide/vcl-backends.html) and the
generation of the [vcl_recv](https://varnish-cache.org/docs/7.3/users-guide/vcl-built-in-subs.html#vcl-recv) subroutine.

In order to extend the generated VCL, use the ``varnish-vcl`` Configmap found in the namespace where the Ingress controller is installed.

The content of the ``snippet`` field from the above mentioned Configmap is appended in the generated VCL.

Example:

```sh
$ kubectl -n varnish-ingress get cm/varnish-vcl -o yaml
```

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  annotations:
    meta.helm.sh/release-name: varnish-ingress-controller
    meta.helm.sh/release-namespace: varnish-ingress
  labels:
    app.kubernetes.io/managed-by: Helm
  name: varnish-vcl
  namespace: varnish-ingress
  resourceVersion: "154768231"
data:
  snippet: |
    sub vcl_backend_response {
      if (beresp.status == 200) {
          set beresp.ttl = 5m; 
      }
      unset beresp.http.Cache-Control;
    }

    sub vcl_deliver {
      if (obj.hits > 0) {
          set resp.http.X-Cache = "HIT"; 
      } else {
          set resp.http.X-Cache = "MISS";
      }
      set resp.http.X-Varnish = "X-Varnish-Foo";
    }
```

The ``snippet`` must contain valid VCL, otherwise it will fail against Varnish's compilation.


Varnish will automatically have its VCL reloaded everytime either Ingress objects or the ``varnish-vcl`` Configmap have been altered.
