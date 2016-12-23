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
//!     source, shaderc::ShaderKind::Vertex,
//!     "shader.glsl", "main", &options).unwrap();
//!
//! assert_eq!(Some(&0x07230203), binary_result.as_binary().first());
//!
//! let text_result = compiler.compile_into_spirv_assembly(
//!     source, shaderc::ShaderKind::Vertex,
//!     "shader.glsl", "main", &options).unwrap();
//!
//! assert!(text_result.as_text().starts_with("; SPIR-V\n"));
//! ```

extern crate libc;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use libc::{c_int, int32_t, uint32_t};
use std::{fmt, ptr, result, slice, str};
use std::ffi::{CStr, CString};

mod ffi;

/// Error.
///
/// Each enumerants has an affixed string describing detailed reasons for
/// the error. The string can be empty in cases.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Compilation error.
    ///
    /// Contains the number of errors and detailed error string.
    CompilationError(u32, String),
    InternalError(String),
    InvalidStage(String),
    InvalidAssembly(String),
    NullResultObject(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::CompilationError(c, ref r) => {
                if r.is_empty() {
                    write!(f, "{} compilation error(s)", c)
                } else {
                    write!(f, "{} compilation error(s): {}", c, r)
                }
            }
            Error::InternalError(ref r) => {
                if r.is_empty() {
                    write!(f, "internal error")
                } else {
                    write!(f, "internal error: {}", r)
                }
            }
            Error::InvalidStage(ref r) => {
                if r.is_empty() {
                    write!(f, "invalid stage")
                } else {
                    write!(f, "invalid stage: {}", r)
                }
            }
            Error::InvalidAssembly(ref r) => {
                if r.is_empty() {
                    write!(f, "invalid assembly")
                } else {
                    write!(f, "invalid assembly: {}", r)
                }
            }
            Error::NullResultObject(ref r) => {
                if r.is_empty() {
                    write!(f, "null result object")
                } else {
                    write!(f, "null result object: {}", r)
                }
            }
        }
    }
}

/// Compilation status.
pub type Result<T> = result::Result<T, Error>;

/// Source language.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceLanguage {
    GLSL,
    HLSL,
}

/// Shader kind.
///
/// * The `<stage>` enumerants are forced shader kinds, which force the
///   compiler to compile the source code as the specified kind of shader,
///   regardless of `#pragma` directives in the source code.
/// * The `Default<stage>` enumerants are default shader kinds, which
///   allow the compiler to fall back to compile the source code as the
///   specified kind of shader when `#pragma` is not found in the source
///   code.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderKind {
    Vertex,
    Fragment,
    Compute,
    Geometry,
    TessControl,
    TessEvaluation,

    /// Deduce the shader kind from `#pragma` directives in the source code.
    ///
    /// Compiler will emit error if `#pragma` annotation is not found.
    InferFromSource,

    DefaultVertex,
    DefaultFragment,
    DefaultCompute,
    DefaultGeometry,
    DefaultTessControl,
    DefaultTessEvaluation,

    SpirvAssembly,
}

/// GLSL profile.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlslProfile {
    /// Used iff GLSL version did not specify the profile
    None,
    Core,
    Compatibility,
    Es,
}

/// Optimization level.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimization
    Zero,
    /// Optimize towards reducing code size
    Size,
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

    fn handle_compilation_result(result: *mut ffi::ShadercCompilationResult,
                                 is_binary: bool)
                                 -> Result<CompilationResult> {
        let status = unsafe { ffi::shaderc_result_get_compilation_status(result) };
        if status == 0 {
            Ok(CompilationResult::new(result, is_binary))
        } else {
            let num_errors = unsafe { ffi::shaderc_result_get_num_errors(result) } as u32;
            let reason = unsafe {
                let p = ffi::shaderc_result_get_error_message(result);
                let bytes = CStr::from_ptr(p).to_bytes();
                str::from_utf8(bytes).ok().expect("invalid utf-8 string").to_string()
            };
            match status {
                1 => Err(Error::InvalidStage(reason)),
                2 => Err(Error::CompilationError(num_errors, reason)),
                3 => Err(Error::InternalError(reason)),
                4 => Err(Error::NullResultObject(reason)),
                5 => Err(Error::InvalidAssembly(reason)),
                _ => panic!("unhandled shaderc error case"),
            }
        }
    }

    /// Compiles the given source string `source_text` to a SPIR-V module
    /// according to the given `additional_options`.
    ///
    /// The source string will be compiled into a SPIR-V binary module
    /// contained in a `CompilationResult` object if no error happens.
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
                              -> Result<CompilationResult> {
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
        Compiler::handle_compilation_result(result, true)
    }

    /// Like `compile_into_spirv` but the result contains SPIR-V assembly text
    /// instead of binary module.
    pub fn compile_into_spirv_assembly(&mut self,
                                       source_text: &str,
                                       shader_kind: ShaderKind,
                                       input_file_name: &str,
                                       entry_point_name: &str,
                                       additional_options: &CompileOptions)
                                       -> Result<CompilationResult> {
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
        Compiler::handle_compilation_result(result, false)
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

    /// Adds a predefined macro to the compilation options.
    ///
    /// This has the same effect as passing `-Dname=value` to the command-line
    /// compiler.  If `value` is `None`, it has the same effect as passing
    /// `-Dname` to the command-line compiler. If a macro definition with the
    /// same name has previously been added, the value is replaced with the
    /// new value.
    pub fn add_macro_definition(&mut self, name: &str, value: Option<&str>) {
        let c_name = CString::new(name).expect("cannot convert name to c string");
        if value.is_some() {
            let value = value.unwrap();
            let c_value = CString::new(value).expect("cannot convert value to c string");
            unsafe {
                ffi::shaderc_compile_options_add_macro_definition(self.raw,
                                                                  c_name.as_ptr(),
                                                                  name.len(),
                                                                  c_value.as_ptr(),
                                                                  value.len())
            }
        } else {
            unsafe {
                ffi::shaderc_compile_options_add_macro_definition(self.raw,
                                                                  c_name.as_ptr(),
                                                                  name.len(),
                                                                  ptr::null(),
                                                                  0)
            }
        }
    }

    /// Sets the source language.
    ///
    /// The default is GLSL.
    pub fn set_source_language(&mut self, language: SourceLanguage) {
        unsafe { ffi::shaderc_compile_options_set_source_language(self.raw, language as int32_t) }
    }

    /// Sets the compiler mode to generate debug information in the output.
    pub fn set_generate_debug_info(&mut self) {
        unsafe { ffi::shaderc_compile_options_set_generate_debug_info(self.raw) }
    }

    /// Sets the optimization level to `level`.
    ///
    /// If mulitple invocations for this method, only the last one takes effect.
    pub fn set_optimization_level(&mut self, level: OptimizationLevel) {
        unsafe { ffi::shaderc_compile_options_set_optimization_level(self.raw, level as int32_t) }
    }

    /// Forces the GLSL language `version` and `profile`.
    ///
    /// The version number is the same as would appear in the `#version`
    /// directive in the source. Version and profile specified here
    /// overrides the `#version` directive in the source code. Use
    /// `GlslProfile::None` for GLSL versions that do not define profiles,
    /// e.g., version below 150.
    pub fn set_forced_version_profile(&mut self, version: u32, profile: GlslProfile) {
        unsafe {
            ffi::shaderc_compile_options_set_forced_version_profile(self.raw,
                                                                    version as c_int,
                                                                    profile as int32_t)
        }
    }

    /// Sets the compiler mode to suppress warnings.
    ///
    /// This overrides warnings-as-errors mode: when both suppress-warnings and
    /// warnings-as-errors modes are turned on, warning messages will be
    /// inhibited, and will not be emitted as error messages.
    pub fn set_suppress_warnings(&mut self) {
        unsafe { ffi::shaderc_compile_options_set_suppress_warnings(self.raw) }
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

    /// Returns the number of warnings generated during the compilation.
    pub fn get_num_warnings(&self) -> u32 {
        (unsafe { ffi::shaderc_result_get_num_warnings(self.raw) }) as u32
    }

    /// Returns the detailed warnings as a string.
    pub fn get_warning_messages(&self) -> String {
        unsafe {
            let p = ffi::shaderc_result_get_error_message(self.raw);
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
    static VOID_E: &'static str = "#version 310 es\n void E() {}";
    static EXTRA_E: &'static str = "#version 310 es\n E\n void main() {}";
    static IFDEF_E: &'static str = "#version 310 es\n #ifdef E\n void main() {}\n\
                                    #else\n #error\n #endif";
    static HLSL_VERTEX: &'static str = "float4 main(uint index: SV_VERTEXID): SV_POSITION\n\
                                        { return float4(1., 2., 3., 4.); }";
    static TWO_ERROR: &'static str = "#version 310 es\n #error one\n #error two\n void main() {}";
    static TWO_ERROR_MSG: &'static str = "shader.glsl:2: error: '#error' : one\n\
                                          shader.glsl:3: error: '#error' : two\n";
    static TWO_WARNING: &'static str = "#version 140\n\
                                        attribute float x;\n attribute float y;\n void main() {}";
    static TWO_WARNING_MSG: &'static str = "\
shader.glsl:2: warning: attribute deprecated in version 130; may be removed in future release\n\
shader.glsl:3: warning: attribute deprecated in version 130; may be removed in future release\n";
    static DEBUG_INFO: &'static str = "#version 140\n \
                                       void main() {\n vec2 debug_info_sample = vec2(1.0);\n }";
    static CORE_PROFILE: &'static str = "void main() {\n gl_ClipDistance[0] = 5.;\n }";

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
    fn test_compile_vertex_shader_into_spirv() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv(VOID_MAIN,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options)
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_vertex_shader_into_spirv_assembly() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv_assembly(VOID_MAIN,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_add_macro_definition_normal_value() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some("main"));
        let result = c.compile_into_spirv_assembly(VOID_E,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_add_macro_definition_empty_value() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some(""));
        let result = c.compile_into_spirv_assembly(EXTRA_E,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_add_macro_definition_no_value() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", None);
        let result = c.compile_into_spirv_assembly(IFDEF_E,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_clone() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", None);
        let o = options.clone().unwrap();
        let result = c.compile_into_spirv_assembly(IFDEF_E,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &o)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_set_source_language() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_source_language(SourceLanguage::HLSL);
        let result = c.compile_into_spirv(HLSL_VERTEX,
                                          ShaderKind::Vertex,
                                          "shader.hlsl",
                                          "main",
                                          &options)
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_set_generate_debug_info() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_generate_debug_info();
        let result = c.compile_into_spirv_assembly(DEBUG_INFO,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert!(result.as_text().contains("debug_info_sample"));
    }

    #[test]
    fn test_compile_options_set_optimization_level_zero() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_optimization_level(OptimizationLevel::Zero);
        let result = c.compile_into_spirv_assembly(DEBUG_INFO,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert!(result.as_text().contains("OpName"));
        assert!(result.as_text().contains("OpSource"));
    }

    #[test]
    fn test_compile_options_set_optimization_level_size() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_optimization_level(OptimizationLevel::Size);
        let result = c.compile_into_spirv_assembly(DEBUG_INFO,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   &options)
                      .unwrap();
        assert!(!result.as_text().contains("OpName"));
        assert!(!result.as_text().contains("OpSource"));
    }

    #[test]
    fn test_compile_options_set_forced_version_profile_ok() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_forced_version_profile(450, GlslProfile::Core);
        let result = c.compile_into_spirv(CORE_PROFILE,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options)
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_set_forced_version_profile_err() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_forced_version_profile(310, GlslProfile::Es);
        let result = c.compile_into_spirv(CORE_PROFILE,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options);
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(3, ref s))
                            if s.contains("error: 'gl_ClipDistance' : undeclared identifier"));
    }

    #[test]
    fn test_compile_options_set_suppress_warnings() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_suppress_warnings();
        let result = c.compile_into_spirv(TWO_WARNING,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options)
                      .unwrap();
        assert_eq!(0, result.get_num_warnings());
    }

    #[test]
    fn test_error_compilation_error() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv(TWO_ERROR,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options);
        assert!(result.is_err());
        assert_eq!(Some(Error::CompilationError(2, TWO_ERROR_MSG.to_string())),
                   result.err());
    }

    #[test]
    fn test_error_invalid_stage() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv(VOID_MAIN,
                                          ShaderKind::InferFromSource,
                                          "shader.glsl",
                                          "main",
                                          &options);
        assert!(result.is_err());
        assert_eq!(Some(Error::InvalidStage("".to_string())), result.err());
    }

    #[test]
    fn test_warning() {
        let mut c = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();
        let result = c.compile_into_spirv(TWO_WARNING,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          &options)
                      .unwrap();
        assert_eq!(2, result.get_num_warnings());
        assert_eq!(TWO_WARNING_MSG.to_string(), result.get_warning_messages());
    }
}
