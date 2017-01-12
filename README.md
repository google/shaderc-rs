shaderc-rs
==========

[![Build Status](https://travis-ci.org/google/shaderc-rs.svg?branch=master)](https://travis-ci.org/google/shaderc-rs)

Rust bindings for the [shaderc](https://github.com/google/shaderc) library.

### Disclaimer

This is not an official Google product (experimental or otherwise), it is just
code that happens to be owned by Google.

Usage
-----

The `shaderc_combined` library (`libshaderc_combined.a` on Unix-like systems)
is required for proper linking. You can compile it by checking out the shaderc
project and follow the instructions there. Then place `libshaderc_combined.a`
at a path that is scanned by the linker (e.g., the `deps` directory within the
`target` directory).

First add to your `Cargo.toml`:

```toml
[dependencies]
rspirv = "0.1"
```

Then add to your crate root:

```rust
extern crate shaderc;
```

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

### TODO

- [ ] include spport

[me]: https://github.com/antiagainst
