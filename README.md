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

The included shaderc-sys crate uses [`build.rs`](shaderc-sys/build/build.rs) to
discover or build a copy of shaderc libraries.  See Setup section.

First add to your `Cargo.toml`:

```toml
[dependencies]
shaderc = "0.5"
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

The order of preference in which the build script will attempt to obtain
shaderc can be controlled by several options, which are passed through to
shaderc-sys when building shaderc-rs:

1. The option `--features build-from-source` will prevent automatic library
   detection and force building from source.
2. If the `SHADERC_LIB_DIR` environment variable is set to
   `/path/to/shaderc/libs/`, it will take precedence and `libshaderc_combined.a`
   (and the glsang and SPIRV libraries on Linux) will be searched in the
   `/path/to/shaderc/libs/` directory.
3. On Linux, `/usr/lib/` will be automatically searched for system libraries
   if none of the above were given.
4. If no other option was set or succeeded, shaderc-sys will fall back to
   checking out and compiling a copy of shaderc.  This procedure is quite slow.

NOTE: `--no-default-features` still works on shaderc-rs, but shaderc-sys
implements this behavior in a deprecated manner, and it will be removed in the
next release.  This method only works with a monolithic `libshaderc_combined.a`.
Refer to pre-0.5 documentation for more information.
Prefer `SHADERC_LIB_DIR="/path/to/shaderc/libs/"`.

Building from Source
--------------------

The shaderc-sys [`build.rs`](shaderc-sys/build/build.rs) will automatically check out and compile a copy of native C++ shaderc and link to the generated artifacts,
which requires `git`, `cmake`, and `python` existing in the `PATH`.

To build your own libshaderc for the shaderc-sys crate, the following tools must be installed and available on `PATH`:
- [CMake](https://cmake.org/)
- [Git](https://git-scm.com/)
- [Python](https://www.python.org/) (works with both Python 2.x and 3.x, on windows the executable must be named `python.exe`)
- a C++11 compiler

Additionally, the build script can auto detect and use the following if they are on `PATH`:
- [Ninja](https://github.com/ninja-build/ninja/releases)

These requirements can be either installed with your favourite package manager or with installers
from the projects' websites. Below are some example ways to get setup.

### windows-msvc Example Setup

1.  `rustup default stable-x86_64-pc-windows-msvc`
2.  Install [Build Tools for Visual Studio 2017](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2017). If you have already been using this toolchain then its probably already installed.
3.  Install [msys2](http://www.msys2.org/), following ALL of the instructions.
4.  Then in the msys2 terminal run: `pacman --noconfirm -Syu mingw-w64-x86_64-cmake mingw-w64-x86_64-python2`
5.  Add the msys2 mingw64 binary path to the PATH environment variable.

NOTE: On Windows if building with MSBuild (the default), it may fail because of
file path too long. That is a [limitation of MSBuild](https://github.com/Microsoft/msbuild/issues/53).
You can work around either by set the target directory to a shallower one using
`cargo --target-dir`, or [download Ninja](https://github.com/ninja-build/ninja/releases)
and make it accessible on `PATH`. The build script will automatically detect
and use Ninja instead of MSBuild.

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

On Arch linux, the [shaderc package](https://www.archlinux.org/packages/extra/x86_64/shaderc/) will include glsang and SPIRV libs in a detectable location.

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
[doc-compiler]: https://docs.rs/shaderc/0.5/shaderc/struct.Compiler.html
[doc-options]: https://docs.rs/shaderc/0.5/shaderc/struct.CompileOptions.html
[doc-artifact]: https://docs.rs/shaderc/0.5/shaderc/struct.CompilationArtifact.html
[me]: https://github.com/antiagainst
