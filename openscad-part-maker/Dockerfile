# ---- Builder -------------------------------------------------
FROM rust:1.91-slim-bullseye AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates git pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo fetch
COPY . .
RUN cargo build --release --locked

# ---- Runtime -------------------------------------------------
FROM debian:bullseye-slim AS runtime
LABEL org.opencontainers.image.source="https://github.com/enigmacurry/openscad-part-maker"
RUN apt-get update && apt-get install -y --no-install-recommends \
        openscad ca-certificates && \
    rm -rf /var/lib/apt/lists/*
ARG USER=appuser UID=1001 GID=1001
RUN addgroup --gid $GID $USER && adduser --uid $UID --gid $GID --disabled-password --gecos "" $USER
COPY --from=builder /app/target/release/openscad-part-maker /usr/local/bin/openscad-part-maker
COPY template /template
USER $USER
ENTRYPOINT ["openscad-part-maker"]
