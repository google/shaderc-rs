// Copyright 2016 Google Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Rust binding for the shaderc library.
//!
//! The [shaderc](https://github.com/google/shaderc) library provides an API
//! for compiling GLSL/HLSL source code to SPIRV modules. It has been shipping
//! in the Android NDK since version r12b.
//!
//! The `shaderc_combined` library (`libshaderc_combined.a`) is required for
//! proper linking. You can compile it by checking out the shaderc project and
//! follow the instructions there. Then place `libshaderc_combined.a` at a path
//! that is scanned by the linker (e.g., the `deps` directory within the
//! `target` directory).
//!
//! # Examples
//!
//! Compile a shader into SPIR-V binary module and assembly text:
//!
//! ```
//! use shaderc;
//!
//! let source = "#version 310 es\n void main() {}";
//!
//! let mut compiler = shaderc::Compiler::new().unwrap();
//! let options = shaderc::CompileOptions::new().unwrap();
//! let binary_result = compiler.compile_into_spirv(
//!     source, shaderc::ShaderKind::GlslVertex,
//!     "shader.glsl", "main", &options);
//!
//! assert_eq!(Some(&0x07230203), binary_result.as_binary().first());
//!
//! let text_result = compiler.compile_into_spirv_assembly(
//!     source, shaderc::ShaderKind::GlslVertex,
//!     "shader.glsl", "main", &options);
//!
//! assert!(text_result.as_text().starts_with("; SPIR-V\n"));
//! ```

extern crate libc;

use libc::{int32_t, uint32_t};
use std::{slice, str};
use std::ffi::{CStr, CString};

mod ffi;

/// Shader kind.
///
/// * The `Glsl<stage>` enumerants are forced shader kinds, which force the
///   compiler to compile the source code as the specified kind of shader,
///   regardless of `#pragma` directives in the source code.
/// * The `GlslDefault<stage>` enumerants are default shader kinds, which
///   allow the compiler to fall back to compile the source code as the
///   specified kind of shader when `#pragma` is not found in the source
///   code.
#[repr(C)]
pub enum ShaderKind {
    GlslVertex,
    GlslFragment,
    GlslCompute,
    GlslGeometry,
    GlslTessControl,
    GlslTessEvaluation,

    /// Deduce the shader kind from `#pragma` directives in the source code.
    ///
    /// Compiler will emit error if `#pragma` annotation is not found.
    InferFromSource,

    GlslDefaultVertex,
    GlslDefaultFragment,
    GlslDefaultCompute,
    GlslDefaultGeometry,
    GlslDefaultTessControl,
    GlslDefaultTessEvaluation,

    SpirvAssembly,
}

/// An opaque object managing all compiler states.
pub struct Compiler {
    raw: *mut ffi::ShadercCompiler,
}

impl Compiler {
    /// Returns an compiler object that can be used to compile SPIR-V modules.
    ///
    /// A return of `None` indicates that there was an error initializing
    /// the underlying compiler.
    pub fn new() -> Option<Compiler> {
        let p = unsafe { ffi::shaderc_compiler_initialize() };
        if p.is_null() {
            None
        } else {
            Some(Compiler { raw: p })
        }
    }

    /// Compiles the given source string `source_text` to a SPIR-V module
    /// according to the given `additional_options`.
    ///
    /// The source string will be compiled into a SPIR-V binary module
    /// contained in a `CompilationResult` object.
    ///
    /// The source string is treated as the given shader kind `shader_kind`.
    /// If `InferFromSource` is given, the compiler will try to deduce the
    /// shader kind from the source string via `#pragma` directives and a
    /// failure in deducing will generate an error. If the shader kind is
    /// set to one of the default shader kinds, the compiler will fall back
    /// to the default shader kind in case it failed to deduce the shader
    /// kind from the source string.
    ///
    /// `input_file_name` is a string used as a tag to identify the source
    /// string in cases like emitting error messages. It doesn't have to be
    /// a canonical "file name".
    ///
    /// `entry_point_name` is a string defines the name of the entry point
    /// to associate with the source string.
    pub fn compile_into_spirv(&mut self,
                              source_text: &str,
                              shader_kind: ShaderKind,
                              input_file_name: &str,
                              entry_point_name: &str,
                              additional_options: &CompileOptions)
                              -> CompilationResult {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source to c string");
        let c_file = CString::new(input_file_name)
                         .expect("cannot convert input_file_name to c string");
        let c_entry_point = CString::new(entry_point_name)
                                .expect("cannot convert entry_point_name to c string");
        let result = unsafe {
            ffi::shaderc_compile_into_spv(self.raw,
                                          c_source.as_ptr(),
                                          source_size,
                                          shader_kind as int32_t,
                                          c_file.as_ptr(),
                                          c_entry_point.as_ptr(),
                                          additional_options.raw)
        };
        CompilationResult::new(result, true)
    }

    /// Like `compile_into_spirv` but the result contains SPIR-V assembly text
    /// instead of binary module.
    pub fn compile_into_spirv_assembly(&mut self,
                                       source_text: &str,
                                       shader_kind: ShaderKind,
                                       input_file_name: &str,
                                       entry_point_name: &str,
                                       additional_options: &CompileOptions)
                                       -> CompilationResult {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source to c string");
        let c_file = CString::new(input_file_name)
                         .expect("cannot convert input_file_name to c string");
        let c_entry_point = CString::new(entry_point_name)
                                .expect("cannot convert entry_point_name to c string");
        let result = unsafe {
            ffi::shaderc_compile_into_spv_assembly(self.raw,
                                                   c_source.as_ptr(),
                                                   source_size,
                                                   shader_kind as int32_t,
                                                   c_file.as_ptr(),
                                                   c_entry_point.as_ptr(),
                                                   additional_options.raw)
        };
        CompilationResult::new(result, false)
    }
}

impl Drop for Compiler {
    fn drop(&mut self) {
        unsafe { ffi::shaderc_compiler_release(self.raw) }
    }
}

/// An opaque object managing options to compilation.
pub struct CompileOptions {
    raw: *mut ffi::ShadercCompileOptions,
}

impl CompileOptions {
    /// Returns a default-initialized compilation options object.
    ///
    /// A return of `None` indicates that there was an error initializing
    /// the underlying options object.
    pub fn new() -> Option<CompileOptions> {
        let p = unsafe { ffi::shaderc_compile_options_initialize() };
        if p.is_null() {
            None
        } else {
            Some(CompileOptions { raw: p })
        }
    }

    /// Returns a copy of the given compilation options object.
    ///
    /// A return of `None` indicates that there was an error copying
    /// the underlying options object.
    pub fn clone(&self) -> Option<CompileOptions> {
        let p = unsafe { ffi::shaderc_compile_options_clone(self.raw) };
        if p.is_null() {
            None
        } else {
            Some(CompileOptions { raw: p })
        }
    }
}

impl Drop for CompileOptions {
    fn drop(&mut self) {
        unsafe { ffi::shaderc_compile_options_release(self.raw) }
    }
}

/// An opaque object containing the results of compilation.
pub struct CompilationResult {
    raw: *mut ffi::ShadercCompilationResult,
    is_binary: bool,
}

impl CompilationResult {
    fn new(result: *mut ffi::ShadercCompilationResult, is_binary: bool) -> CompilationResult {
        CompilationResult {
            raw: result,
            is_binary: is_binary,
        }
    }

    /// Returns the number of bytes of the compilation output data.
    pub fn len(&self) -> usize {
        unsafe { ffi::shaderc_result_get_length(self.raw) }
    }

    /// Returns the compilation output data as a binary slice.
    ///
    /// # Panics
    ///
    /// This method will panic if the compilation does not generate a
    /// binary output.
    pub fn as_binary(&self) -> &[u32] {
        if !self.is_binary {
            panic!("not binary result")
        }

        assert_eq!(0, self.len() % 4);
        let num_words = self.len() / 4;

        unsafe {
            let p = ffi::shaderc_result_get_bytes(self.raw);
            slice::from_raw_parts(p as *const uint32_t, num_words)
        }
    }

    /// Returns the compilation output data as a text string.
    ///
    /// # Panics
    ///
    /// This method will panic if the compilation does not generate a
    /// text output.
    pub fn as_text(&self) -> String {
        if self.is_binary {
            panic!("not text result")
        }
        unsafe {
            let p = ffi::shaderc_result_get_bytes(self.raw);
            let bytes = CStr::from_ptr(p).to_bytes();
            str::from_utf8(bytes).ok().expect("invalid utf-8 string").to_string()
        }
    }
}

impl Drop for CompilationResult {
    fn drop(&mut self) {
        unsafe { ffi::shaderc_result_release(self.raw) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static VOID_MAIN: &'static str = "#version 310 es\n void main() {}";
    static VOID_MAIN_ASSEMBLY: &'static str = "\
; SPIR-V
; Version: 1.0
; Generator: Google Shaderc over Glslang; 1
; Bound: 6
; Schema: 0
               OpCapability Shader
          %1 = OpExtInstImport \"GLSL.std.450\"
               OpMemoryModel Logical GLSL450
               OpEntryPoint Vertex %main \"main\"
               OpSource ESSL 310
               OpSourceExtension \"GL_GOOGLE_cpp_style_line_directive\"
               OpSourceExtension \"GL_GOOGLE_include_directive\"
               OpName %main \"main\"
       %void = OpTypeVoid
          %3 = OpTypeFunction %void
       %main = OpFunction %void None %3
          %5 = OpLabel
               OpReturn
               OpFunctionEnd
";

    #[test]
    fn compile_vertex_shader_into_spirv() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv(VOID_MAIN,
                                          ShaderKind::GlslVertex,
                                          "shader.glsl",
                                          "main",
                                          &options);
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn compile_vertex_shader_into_spirv_assembly() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv_assembly(VOID_MAIN,
                                                   ShaderKind::GlslVertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options);
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }
}
