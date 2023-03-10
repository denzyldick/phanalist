<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/branding/banner-cropped.png"/>

[![Build](https://github.com/denzyldick/phanalist/actions/workflows/build.yml/badge.svg)](https://github.com/denzyldick/phanalist/actions/workflows/build.yml) [![Docker](https://github.com/denzyldick/phanalist/actions/workflows/ci.yml/badge.svg)](https://github.com/denzyldick/phanalist/actions/workflows/ci.yml)

***_TLDR; A static analyzer for PHP. It helps you catch common mistakes in your PHP code._***
 

These are the current checks implemented.
- [ ] Extending undefined classes.
- [x] Methods without modifiers(private, public & protected).
- [x] Classes that start with a lowercase.
- [x] Check if method exists when being called inside another method.
- [x] Methods that return a value without defining a return type.
- [x] Constants that have all letters in lowercase.
- [x] Parameters without any type.
- [x] Correct location for the PHP opening tag.


### How to compile and run
To successfully run this project you will need to first install the rust toolchain. If everything was
installed successfully you will need to download this project and run `cargo build`. This command 
will compile the source code and create an executable. The executable is located inside the 
`target/debug` folder. Run this executable inside of your PHP project.

### Container

You can also use the docker image. Run the docker images inside your PHP project. 

```bash
$ docker run -v $(pwd):/var/src ghcr.io/denzyldick/phanalist:latest

```

### Preview

In this example phanalist is analysing 1 php file. 

<img src=https://github.com/denzyldick/phanalist/blob/main/output.gif/>

