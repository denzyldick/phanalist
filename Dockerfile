# Start with a rust alpine image
FROM ubuntu:latest as build 
RUN apt update 
RUN apt install -y rustc cargo clang
WORKDIR /app
COPY ./ /app
RUN cargo build --release

FROM alpine:latest  
COPY --from=0 /app/target/release/phanalist .

CMD ["./phanalist","--directory=/var/src"]
