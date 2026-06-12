# syntax=docker/dockerfile:1.7

FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples

RUN cargo build --locked --release -p zap-cli

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 zap \
    && useradd --system --uid 10001 --gid zap --home-dir /var/lib/zap --shell /usr/sbin/nologin zap \
    && mkdir -p /etc/zap /var/lib/zap /var/log/zap /opt/zap/drivers \
    && chown -R zap:zap /etc/zap /var/lib/zap /var/log/zap /opt/zap/drivers

COPY --from=builder /workspace/target/release/zap /usr/local/bin/zap

USER zap
WORKDIR /var/lib/zap

EXPOSE 7000/udp

ENTRYPOINT ["/usr/bin/tini", "--", "zap"]
CMD ["run", "--config", "/etc/zap/zap.toml"]
