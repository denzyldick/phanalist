FROM rust:slim AS builder
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/phanalist
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl
FROM scratch AS phanalist
COPY --from=builder /usr/phanalist/target/x86_64-unknown-linux-musl/release/phanalist /bin/phanalist
CMD ["phanalist", "--src=/usr/var"]
