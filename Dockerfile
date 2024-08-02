FROM rust:latest AS builder

COPY Cargo.toml .
COPY src src
COPY template/vcl.hbs template/vcl.hbs

RUN cargo build --release


FROM debian:bookworm-slim as release
LABEL maintainers="Varnish-Cache friends"

RUN apt update && apt -y install varnish

WORKDIR controller

COPY --from=builder ./template/vcl.hbs template/vcl.hbs
COPY --from=builder ./target/release/vingress vingress-bin

ENTRYPOINT ["./vingress-bin"] 
CMD []
