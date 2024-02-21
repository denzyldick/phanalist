<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/docs/branding/banner-cropped.png"/>

Performant static analyzer for PHP, which is extremely easy to use. It helps you catch common mistakes in your PHP code.


### Installation

#### Composer

The simplest way is to install it with Composer.
##### Install package
```bash
composer require denzyl/phanalist
```
##### Run executable
```bash
vendor/bin/phanalist -v 
```
#### Docker

Another option is to use [official docker image](https://github.com/denzyldick/phanalist/pkgs/container/phanalist), by running the command at the root of your project:
```bash
docker run -it -v $(pwd):/var/src ghcr.io/denzyldick/phanalist:latest phanalist --src=/var/src
```
#### Pre-compiled architecture-specific binary.
You can also download your platform-specific binary if you aren't using Composer.

- macOS: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-apple-darwin/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-apple-darwin/phanalist)
- Linux MUSL: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-unknown-linux-musl/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-unknown-linux-musl/phanalist)
- Linux GNU: [aarch64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/aarch64-unknown-linux-gnu/phanalist), [x86_64](https://raw.githubusercontent.com/denzyldick/phanalist/main/release/x86_64-unknown-linux-gnu/phanalist) 

#### Compile from source

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

### Usage

To analyze your project sources, you just need to run `phanalist`:
```
$ phanalist
The new ./phanalist.yaml configuration file as been created

Scanning files in ./src ...
██████████████████████████████████████████████████████████████████████████████ 3/3
./src/rules/examples/e2/empty_catch.php, detected 1 violation:
  E0002:	There is an empty catch. It's not recommended to catch an Exception without doing anything with it.
  9:11	|         } catch(Exception $e) {}

+-----------+-------------+------------+
| Rule Code | Description | Violations |
+-----------+-------------+------------+
| E0002     | Empty catch |          1 |
+-----------+-------------+------------+

Analysed 2 files in 1.31ms, memory usage: 4.6 MiB
```

On the first run `phanalist.yaml` will be created with the default configurations. And it will be reused on all the following runs.

There are also a few additional parameters:
- `config`: path to the configuration file, `./phanalist.yaml` is default value.
- `src`: path to project sources, `./src` is default value.
- `output-format`: format used to output the results. Possible options are `text` (default) and `json`.
- `summary-only`: output only amounts of found violations for each rule.
- `quiet`: suppresses all the output.


### Configuration

The possible options are:
- `enabled_rules` contains the list of rules to use. All rules will be used if this list is empty.
- `disable_rules` contains the list of rules to ignore.
- `rules` rule-specific options.

The default configuration file is:
```yaml
enabled_rules: []
disable_rules: []
rules:
  E0007:
    check_constructor: true
    max_parameters: 5
  E0009:
    max_complexity: 10
```

### Rules

The following rules are available:

| Code  | Name                    | Default options                                                |
|:-----:|:------------------------|:---------------------------------------------------------------|
| E0000 | Example rule            |                                                                |
| E0001 | Opening tag position    |                                                                |
| E0002 | Empty catch             |                                                                |
| E0003 | Method modifiers        |                                                                |
| E0004 | Uppercase constants     |                                                                |
| E0005 | Capitalized class name  |                                                                |
| E0006 | Property modifiers      |                                                                |
| E0007 | Method parameters count | check_constructor: true, max_parameters: 5                     |
| E0008 | Return type signature   |                                                                |
| E0009 | Cyclomatic complexity   | max_complexity: 10                                             |
| E0011 | Service properties      | include_namespaces: ["App\\Service\\"], excldue_namespaces: [] |

Adding a new rule is a simple task, and [this tutorial](./docs/adding_new_rule.md) explains how to do it.


### Articles

Read a series of chapters on [https://dev.to/denzyldick](https://dev.to/denzyldick) to understand the project's internals. It 
is a great, easy-to-read introduction to this project. 

1. [Write your own static analyzer for PHP.](https://dev.to/denzyldick/the-beginning-of-my-php-static-analyzer-in-rust-5bp8)
2. [How I made it impossible to write spaghetti code.](https://dev.to/denzyldick/how-i-made-it-impossible-to-write-spaghetti-code-dg4)
3. [Detecting spaghetti code in AST of a PHP source code.](https://dev.to/denzyldick/traversing-an-ast-of-php-source-code-2kee)

