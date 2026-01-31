FROM rust:1.93-alpine AS builder

RUN apk add --no-cache musl-dev build-base upx

COPY Cargo.toml Cargo.lock ./
COPY src src
COPY template/vcl.hbs template/vcl.hbs

RUN cargo build --release
RUN upx ./target/release/vingress


FROM varnish:8.0-alpine AS release
LABEL maintainers="Varnish-Cache Friends"

USER root
WORKDIR controller

RUN chown -R varnish:varnish /etc/varnish

USER varnish

COPY --from=builder ./template/vcl.hbs template/vcl.hbs
COPY --from=builder ./target/release/vingress vingress-bin

ENTRYPOINT ["./vingress-bin"] 
CMD []
