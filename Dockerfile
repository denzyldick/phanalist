FROM rust:slim AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/phanalist
COPY . .
RUN cargo build --release
FROM debian:bookworm-slim AS phanalist
RUN apt-get update \
    && apt-get install -y --no-install-recommends libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/phanalist/target/release/phanalist /bin/phanalist
CMD ["phanalist", "--src=/usr/var"]
