FROM rust:latest AS builder

COPY Cargo.toml .
COPY src src
COPY template/vcl.hbs template/vcl.hbs

RUN cargo build --release


FROM varnish:7.5 as release
LABEL maintainers="Varnish-Cache friends"

USER root
WORKDIR controller

RUN chown -R varnish:varnish /etc/varnish

USER varnish

COPY --from=builder ./template/vcl.hbs template/vcl.hbs
COPY --from=builder ./target/release/vingress vingress-bin

ENTRYPOINT ["./vingress-bin"] 
CMD []
