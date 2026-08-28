# syntax=docker/dockerfile:1.7

FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples
COPY tests ./tests
COPY tools ./tools

RUN cargo build --locked --release -p rivun-cli

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 rivun \
    && useradd --system --uid 10001 --gid rivun --home-dir /var/lib/rivun --shell /usr/sbin/nologin rivun \
    && mkdir -p /etc/rivun /var/lib/rivun /var/log/rivun /opt/rivun/drivers \
    && chown -R rivun:rivun /etc/rivun /var/lib/rivun /var/log/rivun /opt/rivun/drivers

COPY --from=builder /workspace/target/release/rivun /usr/local/bin/rivun

USER rivun
WORKDIR /var/lib/rivun

EXPOSE 7000/udp

ENTRYPOINT ["/usr/bin/tini", "--", "rivun"]
CMD ["run", "--config", "/etc/rivun/rivun.toml"]
