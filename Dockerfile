# syntax=docker/dockerfile:1.7

FROM rust:1.85-bookworm AS rust-builder
WORKDIR /build
COPY desktop/rust/Cargo.toml desktop/rust/Cargo.lock* ./desktop/rust/
COPY desktop/rust/src ./desktop/rust/src
RUN cd desktop/rust && cargo build --release

FROM python:3.11-slim-bookworm AS python-runtime
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1
WORKDIR /app
COPY desktop/python/requirements.txt ./desktop/python/requirements.txt
RUN pip install --no-cache-dir -r desktop/python/requirements.txt
COPY desktop/python ./desktop/python
COPY --from=rust-builder /build/desktop/rust/target/release/shard-daemon /usr/local/bin/shard-daemon
COPY scripts/docker-entrypoint.sh /usr/local/bin/shard-entrypoint
RUN chmod +x /usr/local/bin/shard-entrypoint
EXPOSE 8000 9091 4001 4101
ENTRYPOINT ["shard-entrypoint"]
