## Installation

### Installation script

The simplest way to install phanalist is to use the installation script.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh | sh
```

It will automatically download executable for your platform.
```bash
~/phanalist -V
phanalist 1.0.0
```

### Pre-compiled architecture-specific binary.

You can also manually download your platform-specific binary.

- macOS: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-apple-darwin/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-apple-darwin/phanalist)
- Linux MUSL: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-unknown-linux-musl/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-unknown-linux-musl/phanalist)
- Linux GNU: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-unknown-linux-gnu/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-unknown-linux-gnu/phanalist)

### Compile from source

Alternatively, you can compile it from sources on your local:
```bash
# Install RUST
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Get the latest sources
git clone git@github.com:denzyldick/phanalist.git && cd phanalist
# Compile
cargo build -r
# Run the compiled executable
./target/release/phanalist -V
```


### Composer

Also, you can install phanalist with Composer. 
```bash
# Install package
composer require denzyl/phanalist
# Run executable
vendor/bin/phanalist -v
```

### Docker

Another option is to use [official docker image](https://github.com/denzyldick/phanalist/pkgs/container/phanalist), by running the command at the root of your project:
```bash
docker run -it -v $(pwd):/var/src ghcr.io/denzyldick/phanalist:latest phanalist --src=/var/src
```