#!/usr/bin/env sh
kubectl config use-context kind-varnish
docker build -t vingress-dev:latest . -f Dockerfile.dev
kind load docker-image vingress-dev:latest -n varnish
kubectl -n vingress rollout restart deploy/varnish-ingress-controller
