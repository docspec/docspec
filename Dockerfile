# syntax=docker/dockerfile:1

# ─ Builder ─
FROM rust:alpine3.20 AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build -p docspec-http --bin docspec-http --release && \
  mkdir -p /out && \
  cp target/release/docspec-http /out/docspec-http

# ─ Runtime ─
FROM alpine:3.23

# Binary is dynamically linked against musl. Runtime base MUST remain
# Alpine-compatible (any image shipping `/lib/ld-musl-x86_64.so.1`).
# Do not change to debian-slim or distroless/cc without also switching to
# `--target x86_64-unknown-linux-musl` + `RUSTFLAGS=-C target-feature=+crt-static`
# in the builder.
RUN addgroup -S -g 10001 docspec \
  && adduser -S -D -u 10001 -G docspec docspec

COPY --from=builder /out/docspec-http /usr/local/bin/docspec-http

USER 10001:10001

EXPOSE 3000

STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://127.0.0.1:3000/health || exit 1

ARG IMAGE_VERSION=0.1.0
ARG IMAGE_REVISION=unknown

LABEL org.opencontainers.image.title="docspec-http" \
  org.opencontainers.image.description="HTTP API server for DocSpec markdown to BlockNote JSON conversion" \
  org.opencontainers.image.source="https://github.com/docspec/docspec" \
  org.opencontainers.image.version="${IMAGE_VERSION}" \
  org.opencontainers.image.revision="${IMAGE_REVISION}" \
  org.opencontainers.image.licenses="MIT"

ENTRYPOINT ["/usr/local/bin/docspec-http"]
CMD ["--host", "0.0.0.0", "--port", "3000"]
