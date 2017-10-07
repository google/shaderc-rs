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
But then you will need to place a copy of the `shaderc_combined` library
(on Windows) or the `shaderc_shared` library (on Linux and macOS) to a location
that is scanned by the linker (e.g., the `deps` directory within the `target`
directory).

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
