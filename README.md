shaderc-rs
==========

[![Version](https://img.shields.io/crates/v/shaderc.svg)](https://crates.io/crates/shaderc)
[![Documentation](https://docs.rs/shaderc/badge.svg)](https://docs.rs/shaderc)

Rust bindings for the [shaderc][shaderc] library.

### Disclaimer

This is not an official Google product (experimental or otherwise), it is just
code that happens to be owned by Google.

Usage
-----

The included shaderc-sys crate uses [`build.rs`](shaderc-sys/build/build.rs) to
discover or build a copy of shaderc libraries.  See Setup section.

First add to your `Cargo.toml`:

```toml
[dependencies]
shaderc = "0.8"
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

shaderc-rs needs the C++ [shaderc library](https://github.com/google/shaderc).
It's shipped inside the [Vulkan SDK](https://www.lunarg.com/vulkan-sdk/).
You may be able to install it directly on some Linux distro's using the package
manager. The C++ shaderc project provides [artifacts
downloads](https://github.com/google/shaderc#downloads). You can also
[build it from source](#building-from-source).

The order of preference in which the [build script](shaderc-sys/build/build.rs)
attempts to obtain native shaderc can be controlled by several options, which
are passed through to shaderc-sys when building shaderc-rs:

1. Building from source, if option `--features build-from-source` is specified.
1. If the `SHADERC_LIB_DIR` environment variable is set to
   `/path/to/shaderc/libs/`, that path will be searched for native dynamic or
   static shaderc library.
1. If the `VULKAN_SDK` environment variable is set, then `$VULKAN_SDK/lib` will
   be searched for native dynamic or static shaderc library.
1. On Linux, system library paths like `/usr/lib/` will additionally be searched
   for native dynamic or shaderc library, if the `SHADERC_LIB_DIR` is not set.
1. Building from source, if the native shaderc library is not found via the
   above steps.

For each library directory, the build script will try to find and link to the
dynamic native shaderc library `shaderc_shared` first and the static native
shaderc library `shaderc_combined` next. To prefer searching for the static
library first and the dynamic library next, the option
`--features prefer-static-linking` may be used.

Building from Source
--------------------

The shaderc-sys [`build.rs`](shaderc-sys/build/build.rs) will automatically
check out and compile a copy of native C++ shaderc and link to the generated
artifacts, which requires `git`, `cmake`, and `python` existing in the `PATH`:

- [CMake](https://cmake.org/)
- [Git](https://git-scm.com/)
- [Python](https://www.python.org/) (only works with Python 3, on Windows
  the executable must be named `python.exe`)
- a C++11 compiler

Additionally:
- [Ninja](https://github.com/ninja-build/ninja/releases) is required on
  windows-msvc, but optional on all other platforms.

These requirements can be either installed with your favourite package manager
or with installers from the projects' websites. Below are some example ways
to get setup.

### windows-msvc Example Setup

1. `rustup default stable-x86_64-pc-windows-msvc`
2. Install [Build Tools for Visual Studio 2017](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2017).
   If you have already been using this toolchain then its probably already
   installed.
3. Install the necessary tools as listed in the above and add their paths
   to the `PATH` environment variable.

### windows-gnu Example Setup

windows-gnu toolchain is not supported but you can instead cross-compile to
windows-gnu from windows-msvc.

Steps 1 and 2 are to workaround https://github.com/rust-lang/rust/issues/49078
by using the same mingw that rust uses.

1. Download and extract https://s3-us-west-1.amazonaws.com/rust-lang-ci2/rust-ci-mirror/x86_64-6.3.0-release-posix-seh-rt_v5-rev2.7z
2. Add the absolute path to mingw64\bin to your PATH environment variable.
3. Run the command: `rustup default stable-x86_64-pc-windows-msvc`
4. Run the command: `rustup target install x86_64-pc-windows-gnu`
5. Install [Build Tools for Visual Studio 2017](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2017).
   If you have already been using this toolchain then its probably already
   installed.
6. Install [msys2](http://www.msys2.org/), following ALL of the instructions.
7. Then in the msys2 terminal run: `pacman --noconfirm -Syu mingw-w64-x86_64-cmake mingw-w64-x86_64-make mingw-w64-x86_64-python3 mingw-w64-x86_64-ninja`
8. Add the msys2 mingw64 binary path to the PATH environment variable.
9. Any cargo command that builds the project needs to include
   `--target x86_64-pc-windows-gnu` e.g. to run: `cargo run --target x86_64-pc-windows-gnu`

### Linux Example Setup

Use your package manager to install the required dev-tools

For example on ubuntu:
```
sudo apt-get install build-essential cmake git ninja python3
```

On Arch linux, you can directly install the [shaderc package](https://www.archlinux.org/packages/extra/x86_64/shaderc/).

### macOS Example Setup

Assuming Homebrew:

```
brew install git cmake ninja python@3.8
```

Contributions
-------------

This project is licensed under the [Apache 2](LICENSE) license. Please see
[CONTRIBUTING](CONTRIBUTING.md) before contributing.

### Authors

This project is initialized and mainly developed by Lei Zhang
([@antiagainst][me]).

[shaderc]: https://github.com/google/shaderc
[doc-compiler]: https://docs.rs/shaderc/0.7/shaderc/struct.Compiler.html
[doc-options]: https://docs.rs/shaderc/0.7/shaderc/struct.CompileOptions.html
[doc-artifact]: https://docs.rs/shaderc/0.7/shaderc/struct.CompilationArtifact.html
[me]: https://github.com/antiagainst
