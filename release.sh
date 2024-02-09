#!/usr/bin/env bash

APP_BIN="phanalist"

echo "Running tests ..."
RUST_MIN_STACK=8388608 cargo test || exit 1

declare -a LOCAL_PLATFORMS=("aarch64-apple-darwin" "x86_64-apple-darwin")
declare -a DOCKER_PLATFORMS=(
  "aarch64-unknown-linux-musl"
  "aarch64-unknown-linux-gnu"
  "x86_64-unknown-linux-musl"
  "x86_64-unknown-linux-gnu"
)

for PLATFORM in "${LOCAL_PLATFORMS[@]}"
do
  echo "Releasing ${PLATFORM} ..."
  cargo build --target ${PLATFORM} --release

  RELEASE_PATH="./release/${PLATFORM}"
  mkdir -p "${RELEASE_PATH}"
  cp target/${PLATFORM}/release/${APP_BIN} ${RELEASE_PATH}/${APP_BIN}
done

for PLATFORM in "${DOCKER_PLATFORMS[@]}"
do
  echo "Releasing ${PLATFORM} ..."

  IMAGE="phanalist-${PLATFORM}"
  docker build -f docker/${PLATFORM} -t ${IMAGE} .

  TRANSFER_PATH="/tmp/release"
  COMPILED_PATH="/usr/src/phanalist/target/${PLATFORM}/release/${APP_BIN}"
  RELEASE_PATH="./release/${PLATFORM}"
  mkdir -p "${RELEASE_PATH}"
  docker run -v ${RELEASE_PATH}:${TRANSFER_PATH} --rm ${IMAGE} cp ${COMPILED_PATH} ${TRANSFER_PATH}/${APP_BIN}
done
