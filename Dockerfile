FROM rust:1.81-alpine AS builder

RUN apk add --no-cache musl-dev build-base

COPY Cargo.toml Cargo.lock ./
COPY src src
COPY template/vcl.hbs template/vcl.hbs

RUN cargo build --release


FROM varnish:7.6-alpine AS release
LABEL maintainers="Varnish-Cache Friends"

USER root
WORKDIR controller

RUN chown -R varnish:varnish /etc/varnish

USER varnish

COPY --from=builder ./template/vcl.hbs template/vcl.hbs
COPY --from=builder ./target/release/vingress vingress-bin

ENTRYPOINT ["./vingress-bin"] 
CMD []
