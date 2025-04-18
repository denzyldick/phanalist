<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/docs/branding/banner-cropped.png"/>

Performant static analyzer for PHP, which is extremely easy to use. It helps you catch common mistakes in your PHP code.

### Installation

The simplest way to install Phanalist is to use the installation script.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh | sh
```

It will automatically download the executable for your platform:

```bash
$ ~/phanalist -V
phanalist 1.0.0
```

There are also [multiple other installation options](./docs/installation.md).

### Usage

To analyze your project sources, you just need to run `phanalist`:

```bash
~/phanalist
```

#### Example

![Example](docs/branding/example.gif)

On the first run `phanalist.yaml` will be created with the default configurations. And it will be reused on all the following runs.

There are also a few additional parameters:

- `config`: path to the configuration file, `./phanalist.yaml` is default value.
- `src`: path to project sources, `./src` is default value.
- `output-format`: format used to output the results. Possible options are `text` (default), `json` and `sarif`.
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
  E0010:
    max_paths: 200
  E0012:
    include_namespaces:
      - "App\\Service\\"
      - "App\\Controller\\"
    exclude_namespaces: []
```

### Rules

The following rules are available:

|                  Code                  | Name                                           | Default options                                                                                              |
| :------------------------------------: | :--------------------------------------------- | :----------------------------------------------------------------------------------------------------------- |
|                 E0000                  | Example rule                                   |                                                                                                              |
| [E0001](/src/rules/examples/e1/e1.md)  | Opening tag position                           |                                                                                                              |
| [E0002](/src/rules/examples/e2/e2.md)  | Empty catch                                    |                                                                                                              |
| [E0003](/src/rules/examples/e3/e3.md)  | Method modifiers                               |                                                                                                              |
|   [E0004](src/rules/examples/e4.md)    | Uppercase constants                            |                                                                                                              |
|   [E0005](src/rules/examples/e5.md)    | Capitalized class name                         |                                                                                                              |
| [E0006](/src/rules/examples/e6/e6.md)  | Property modifiers                             |                                                                                                              |
| [E0007](/src/rules/examples/e7/e7.md)  | Method parameters count                        | check_constructor: true, max_parameters: 5                                                                   |
|  [E0008](src/rules/examples/e8/e8.md)  | Return type signature                          |                                                                                                              |
| [E0009](/src/rules/examples/e9/e9.md)  | Cyclomatic complexity                          | max_complexity: 10                                                                                           |
| [E0010](src/rules/examples/e10/e10.md) | Npath complexity                               | max_complexity: 200                                                                                          |
| [E0011](src/rules/examples/e11/e11.md) | Detect error supression symbol(@)              |                                                                                                              |
|  [E0012](src/rules/examples/e12/e12.md)| Service compatibility with Shared Memory Model | include_namespaces: ["App\\Service\\"],<br/>exclude_namespaces: [],<br/>reset_interfaces: ["ResetInterface"] |
| [E0013](/src/rules/examples/e13/e13.md) | Private method not being used |

Adding a new rule is a simple task, and [this tutorial](./docs/adding_new_rule.md) explains how to do it.

### Articles

Read a series of chapters on [https://dev.to/denzyldick](https://dev.to/denzyldick) to understand the project's internals. It
is a great, easy-to-read introduction to this project.

1. [Write your own static analyzer for PHP.](https://dev.to/denzyldick/the-beginning-of-my-php-static-analyzer-in-rust-5bp8)
2. [How I made it impossible to write spaghetti code.](https://dev.to/denzyldick/how-i-made-it-impossible-to-write-spaghetti-code-dg4)
3. [Detecting spaghetti code in AST of a PHP source code.](https://dev.to/denzyldick/traversing-an-ast-of-php-source-code-2kee)
4. [Getting Symfony app ready for Swoole, RoadRunner, and FrankenPHP (no AI involved).](https://dev.to/sergiid/getting-symfony-app-ready-for-swoole-roadrunner-and-frankenphp-no-ai-involved-2d0g)
5. [Improve your CI output](https://dev.to/denzyldick/improve-your-ci-output-2eg)
6. [Why using unserialize in PHP is a bad idea](https://dev.to/denzyldick/why-is-unserializing-an-object-in-php-a-bad-idea-3odl)

