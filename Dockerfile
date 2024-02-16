FROM rust:slim AS builder
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/phanalist
COPY . . 
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN cargo test --release
FROM scratch as phanalist
COPY --from=builder /usr/phanalist/release/x86_64-unknown-linux-musl/phanalist /bin/phanalist
CMD [ "phanalist --src=/usr/var" ]
