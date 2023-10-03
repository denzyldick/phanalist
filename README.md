<img src="https://raw.githubusercontent.com/denzyldick/phanalist/main/branding/banner-cropped.png"/>

***_TLDR: A static analyzer for PHP. It helps you catch common mistakes in your PHP code._***


These are the current checks implemented.
- [x] Detect when the cyclomatic complexity of a method is too high. The current threshold is 10. 
- [ ] Extending undefined classes.
- [x] Having a try/catch with an empty catch that doesn't do anything. 
- [x] A method that has more than five parameters. 
- [x] Methods without modifiers(private, public & protected).
- [x] Classes that start with a lowercase.
- [x] Check if a method exists when called inside another method.
- [x] Methods that return a value without defining a return type.
- [x] Constants that have all letters in lowercase.
- [x] Parameters without any type.
- [x] Correct location for the PHP opening tag.

### How to compile and run
To successfully run this project, you must first install the rust toolchain. If everything was
installed successfully, you must download this project and run `cargo build.` This command 
will compile the source code and create an executable. The executable is located inside the 
`target/debug` folder. You can just run this executable inside of your PHP project.

### Container

The fastest way to run is using the official docker image. Run the command at the root
of your project. 
```bash
$ docker run -it -v $(pwd):/var/src ghcr.io/denzyldick/phanalist:latest

```

### 👁 Sneak preview.

To illustrate the performance, I have decided to clone different random PHP projects from Github. With the 
current rules implemented, I could scan many files in just a few seconds.  

<img src=https://github.com/denzyldick/phanalist/blob/main/output.gif/>

