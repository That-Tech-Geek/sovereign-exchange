FROM rust:1.98-bookworm AS builder
WORKDIR /src
COPY exchange_core/Cargo.toml exchange_core/Cargo.lock exchange_core/
COPY exchange_core/src exchange_core/src
COPY exchange_core/tests exchange_core/tests
RUN cargo build --release --manifest-path exchange_core/Cargo.toml

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin exchange
RUN mkdir -p /var/lib/sovereign-exchange && chown -R exchange:exchange /var/lib/sovereign-exchange
COPY --from=builder /src/exchange_core/target/release/exchange_core /usr/local/bin/exchange_core
USER exchange
WORKDIR /var/lib/sovereign-exchange
ENV SE_JOURNAL_PATH=/var/lib/sovereign-exchange/commands.journal
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/exchange_core"]
