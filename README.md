shaderc-rs
==========

[![Version](https://img.shields.io/crates/v/shaderc.svg)](https://crates.io/crates/shaderc)
[![Documentation](https://docs.rs/shaderc/badge.svg)](https://docs.rs/shaderc)
[![Build Status](https://travis-ci.org/google/shaderc-rs.svg?branch=master)](https://travis-ci.org/google/shaderc-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/3la8yk6cgkh4jhu3/branch/master?svg=true)](https://ci.appveyor.com/project/antiagainst/shaderc-rs/branch/master)

Rust bindings for the [shaderc][shaderc] library.

### Disclaimer

This is not an official Google product (experimental or otherwise), it is just
code that happens to be owned by Google.

Usage
-----

This library uses [`build.rs`](build/build.rs) to automatically check out
and compile a copy of native C++ shaderc and link to the generated artifacts,
which requires `git`, `cmake`, and `python` existing in the `PATH`.

To turn off this feature, specify `--no-default-features` when building.
But then you will need to place a copy of the `shaderc_combined` static library
to the location (printed out in the warning message) that is scanned by the
linker.

First add to your `Cargo.toml`:

```toml
[dependencies]
shaderc = "0.3"
```

Then add to your crate root:

```rust
extern crate shaderc;
```

Documentation
-------------

shaderc provides the [`Compiler`][doc-compiler] interface to compile GLSL/HLSL
source code into SPIR-V binary modules or assembly code. It can also assemble
SPIR-V assembly into binary module. Default compilation behavior can be
adjusted using [`CompileOptions`][doc-options]. Successful results are kept in
[`CompilationArtifact`][doc-artifact]s.

Please see
[![Documentation](https://docs.rs/shaderc/badge.svg)](https://docs.rs/shaderc)
for detailed documentation.

Example
-------

Compile a shader into SPIR-V binary module and assembly text:

```rust
use shaderc;

let source = "#version 310 es\n void EP() {}";

let mut compiler = shaderc::Compiler::new().unwrap();
let mut options = shaderc::CompileOptions::new().unwrap();
options.add_macro_definition("EP", Some("main"));
let binary_result = compiler.compile_into_spirv(
    source, shaderc::ShaderKind::Vertex,
    "shader.glsl", "main", Some(&options)).unwrap();

assert_eq!(Some(&0x07230203), binary_result.as_binary().first());

let text_result = compiler.compile_into_spirv_assembly(
    source, shaderc::ShaderKind::Vertex,
    "shader.glsl", "main", Some(&options)).unwrap();

assert!(text_result.as_text().starts_with("; SPIR-V\n"));
```

Setup
-----

To build the shaderc-rs crate, the following tools must be installed and available on `PATH`:
- [CMake](https://cmake.org/)
- [Python](https://www.python.org/) (works with both Python 2.x and 3.x, on windows the executable must be named `python.exe`)
- a C++11 compiler

These requirements can be either installed with your favourite package manager or with installers
from the projects' websites. Below are some example ways to get setup.

### windows-msvc Example Setup

1.  `rustup default stable-x86_64-pc-windows-msvc`
2.  Install [Build Tools for Visual Studio 2017](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2017). If you have already been using this toolchain then its probably already installed.
3.  Install [msys2](http://www.msys2.org/), following ALL of the instructions.
4.  Then in the msys2 terminal run: `pacman --noconfirm -Syu mingw-w64-x86_64-cmake mingw-w64-x86_64-python2`
5.  Add the msys2 mingw64 binary path to the PATH environment variable.

### windows-gnu Example Setup

windows-gnu toolchain is not supported but you can instead cross-compile to windows-gnu from windows-msvc.

Steps 1 and 2 are to workaround https://github.com/rust-lang/rust/issues/49078 by using the same mingw that rust uses.

1.  Download and extract https://s3-us-west-1.amazonaws.com/rust-lang-ci2/rust-ci-mirror/x86_64-6.3.0-release-posix-seh-rt_v5-rev2.7z
2.  Add the absolute path to mingw64\bin to your PATH environment variable.
3.  Run the command: `rustup default stable-x86_64-pc-windows-msvc`
4.  Run the command: `rustup target install x86_64-pc-windows-gnu`
5.  Install [Build Tools for Visual Studio 2017](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2017). If you have already been using this toolchain then its probably already installed.
6.  Install [msys2](http://www.msys2.org/), following ALL of the instructions.
7.  Then in the msys2 terminal run: `pacman --noconfirm -Syu mingw-w64-x86_64-cmake mingw-w64-x86_64-make mingw-w64-x86_64-python2`
8.  Add the msys2 mingw64 binary path to the PATH environment variable.
9.  Any cargo command that builds the project needs to include `--target x86_64-pc-windows-gnu` e.g. to run: `cargo run --target x86_64-pc-windows-gnu`

### Linux Example Setup

Use your package manager to install the required dev-tools

For example on ubuntu:
```
sudo apt-get install build-essential git python cmake
```

### macOS Example Setup

Assuming Homebrew:

```
brew install cmake
```

Contributions
-------------

This project is licensed under the [Apache 2](LICENSE) license. Please see
[CONTRIBUTING](CONTRIBUTING.md) before contributing.

### Authors

This project is initialized and mainly developed by Lei Zhang
([@antiagainst][me]).

[shaderc]: https://github.com/google/shaderc
[doc-compiler]: https://docs.rs/shaderc/0.3/shaderc/struct.Compiler.html
[doc-options]: https://docs.rs/shaderc/0.3/shaderc/struct.CompileOptions.html
[doc-artifact]: https://docs.rs/shaderc/0.3/shaderc/struct.CompilationArtifact.html
[me]: https://github.com/antiagainst
