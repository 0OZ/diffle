# The Rust review server plus the web client it serves, in one image.
# Build from the repository root: docker build -f rust/docker/Dockerfile .

FROM node:24-bookworm-slim AS client
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts
COPY vite.config.ts tsconfig.json tsconfig.client.json ./
COPY src ./src
RUN npx vite build

FROM rust:1.97-bookworm AS server
WORKDIR /build
COPY rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml ./
COPY rust/diffle ./diffle
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked --bin diffle \
    && install -D target/release/diffle /out/diffle

FROM debian:bookworm-slim AS runtime
# git: the server reads the repository by running it. curl: the health check.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends git curl ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    # The repository is mounted from the host and owned by whoever runs the container.
    && git config --system --add safe.directory '*'
COPY --from=server /out/diffle /usr/local/bin/diffle
COPY --from=client /build/dist/client /usr/share/diffle/client

# Any uid may run the container: the config lands under a directory it can always write.
ENV DIFFLE_CLIENT_DIR=/usr/share/diffle/client \
    HOME=/tmp \
    XDG_CONFIG_HOME=/tmp/config
WORKDIR /repo
EXPOSE 4966
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl --fail --silent --output /dev/null http://127.0.0.1:4966/
ENTRYPOINT ["diffle"]
# Listening beyond loopback is what makes the published port reachable; compose publishes it on loopback only.
CMD ["--host", "0.0.0.0", "--port", "4966", "--no-open", "--keep-alive"]
