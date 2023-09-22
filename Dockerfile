FROM rust:latest as builder
RUN apt-get update && apt-get install -y \
    clang \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/phanalist
COPY . .
RUN cargo build --release 

FROM ubuntu:latest
COPY --from=builder /usr/src/phanalist/target/release/phanalist /usr/local/bin/phanalist
WORKDIR /var/src
CMD ["phanalist"]
