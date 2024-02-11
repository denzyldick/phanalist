FROM --platform=linux/aarch64 rust:latest as builder
RUN apt-get update && apt-get install -y clang
RUN rustup target add aarch64-unknown-linux-musl
WORKDIR /usr/src/phanalist
COPY . .
RUN cargo build --target aarch64-unknown-linux-musl --release

FROM --platform=linux/aarch64 alpine:3.14
COPY --from=builder /usr/src/phanalist/target/aarch64-unknown-linux-musl/release/phanalist /usr/local/bin/phanalist
CMD ["phanalist", "-s", "/var/src"]