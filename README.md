[![Build master](https://github.com/mariusmagureanu/vingress/actions/workflows/rust.yml/badge.svg)](https://github.com/mariusmagureanu/vingress/actions/workflows/rust.yml)
![Audit](https://github.com/mariusmagureanu/vingress/actions/workflows/audit.yaml/badge.svg)
![Clippy Lint](https://github.com/mariusmagureanu/vingress/actions/workflows/clippy.yaml/badge.svg)
![Docker Pulls](https://img.shields.io/docker/pulls/mariusm/vingress)
![Varnish Cache](https://img.shields.io/badge/Varnish-8.0-blue)
[![Rust Version](https://img.shields.io/badge/rustc-1.90-blue.svg)](https://www.rust-lang.org)
[![dependency status](https://deps.rs/repo/github/mariusmagureanu/vingress/status.svg)](https://deps.rs/repo/github/mariusmagureanu/vingress)
[![Maintenance](https://img.shields.io/badge/maintenance-actively%20maintained-green.svg)](https://github.com/mariusmagureanu/vingress)
[![Artifact Hub](https://img.shields.io/endpoint?url=https://artifacthub.io/badge/repository/varnish-ingress-controller)](https://artifacthub.io/packages/search?repo=varnish-ingress-controller)
![License](https://img.shields.io/badge/license-BSD%202--Clause-blue.svg)

### Varnish Ingress controller

Lite implementation of a Varnish Ingress controller.

---

### Table of contents

- [How does it work](#how-does-it-work)
- [Installation and usage](#installation-and-usage)
- [More VCL](#more-vcl)
- [Misc](#misc)

---

### How does it work

The `varnish-ingress-controller` watches over Ingress objects in the cluster. The watcher is configured to
filter through Ingress objects with the following label:

```yaml
kubernetes.io/ingress: varnish
```

**and** Ingress class name:

```yaml
spec.ingressClassName: varnish
```

The spec of the Ingress objects is then translated into Varnish [VCL](https://varnish-cache.org/docs/trunk/users-guide/vcl.html).

The `varnish-ingress-controller` watches over `INIT | ADD | UPDATE | DELETE` Ingress events and updates
the Varnish VCL accordingly. After a succesfull VCL file update, Varnish will reload its VCL just so it becomes aware of the latest configuration.

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

```c
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
Make sure you're connected to a Kubernetes cluster and run the following:

```sh
$ helm package chart/
$ helm upgrade varnish-ingress-controller --install --namespace vingress --create-namespace ./varnish-ingress-controller-0.4.0.tgz -f charts/values.yaml
```

Update the spec of your Ingress(es) with the following requirements:

1. add the following label: `kubernetes.io/ingress: varnish`
2. set the ingress class: `spec.ingressClassName: varnish`

Investigate the logs of the `varnish-ingress-controller` pod, they should reflect the updates mentioned above on your Ingress object(s):

Example:

```sh
$ kubectl -n <your-namespace> logs po/varnish-ingress-controller-xxxxxxxxxx-yyyyy
```

A Kubernetes service is available to be used for reaching the Varnish containers. It is up to you whether this service
should be used in conjuction with a load-balancer or not.
For quick testing and shorter feedback loops it's easier to just port forward it locally:

Example:

```
$ kubectl -n vingress port-forward svc/varnish-ingress-service 6081:80
$ curl http://127.1:6081/foo -H "Host: foo.bar.com" -v
```

---

### More VCL

The `varnish-ingress-controller` translates the Ingress spec into VCL syntax. However, there's often the
case that the generated VCL needs to be extended to accomodate the various use cases.

Check for the `varnish-vcl` configmap in the namespace where the `varnish-ingress-controller` is installed.
The Configmap has the following fields which is watched by the ingress-controller:

- `vcl_recv_snippet`: snippet added in the `vcl_recv` subroutine after the backends selection
- `snippet`: snippet added after the `vcl_rec` subroutine

Whenever these 2 mentioned fields in the Configmap are updated - the following happens:

1.  update the generated VCL file
2.  issue a `varnishreload` command just so Varnish picks up the new updates

Example:

```sh
$ kubectl -n vingress get cm/varnish-vcl -o yaml
```

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  annotations:
    meta.helm.sh/release-name: varnish-ingress-controller
    meta.helm.sh/release-namespace: vingress
  labels:
    app.kubernetes.io/managed-by: Helm
  name: varnish-vcl
  namespace: vingress
  resourceVersion: "154768231"
data:
  vcl_recv_snippet: |
    if (! req.backend_hint) {
      return (synth(200, "We get here now!"));
    }
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
      set resp.http.X-Varnish = "X-Varnish-foo";
    }
```

---

### Misc

This paragraph highlights some assumptions made in this implementation.

- Single container pod, the Varnish process is started within the controller code
- The `vcl_recv` subroutine is configurable only via editing the vcl.hbs template
- There is no fancy editing of the VCL file, when either the Ingress objects or the `varnish-vcl` Configmap changes, then the VCL file is rewritten entirely
