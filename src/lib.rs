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
//! The `shaderc_combined` library (`libshaderc_combined.a` on Unix like
//! systems) is required for proper linking. You can compile it by checking out
//! the shaderc project and follow the instructions there. Then place
//! `libshaderc_combined.a` at a path that is scanned by the linker (e.g., the
//! `deps` directory within the `target` directory).
//!
//! # Examples
//!
//! Compile a shader into SPIR-V binary module and assembly text:
//!
//! ```
//! use shaderc;
//!
//! let source = "#version 310 es\n void EP() {}";
//!
//! let mut compiler = shaderc::Compiler::new().unwrap();
//! let mut options = shaderc::CompileOptions::new().unwrap();
//! options.add_macro_definition("EP", Some("main"));
//! let binary_result = compiler.compile_into_spirv(
//!     source, shaderc::ShaderKind::Vertex,
//!     "shader.glsl", "main", Some(&options)).unwrap();
//!
//! assert_eq!(Some(&0x07230203), binary_result.as_binary().first());
//!
//! let text_result = compiler.compile_into_spirv_assembly(
//!     source, shaderc::ShaderKind::Vertex,
//!     "shader.glsl", "main", Some(&options)).unwrap();
//!
//! assert!(text_result.as_text().starts_with("; SPIR-V\n"));
//! ```

extern crate libc;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use libc::{c_int, int32_t, uint32_t};
use std::{error, fmt, ptr, result, slice, str};
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

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::CompilationError(_, _) => "compilation error",
            Error::InternalError(_) => "internal error",
            Error::InvalidStage(_) => "invalid stage",
            Error::InvalidAssembly(_) => "invalid assembly",
            Error::NullResultObject(_) => "null result object",
        }
    }
}

/// Compilation status.
pub type Result<T> = result::Result<T, Error>;

/// Target environment.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetEnv {
    /// Compile under Vulkan semantics.
    Vulkan,
    /// Compile under OpenGL semantics.
    OpenGL,
    /// Compile under OpenGL semantics, including compatibility profile functions.
    OpenGLCompat,
}

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

/// Resource limit.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    MaxLights,
    MaxClipPlanes,
    MaxTextureUnits,
    MaxTextureCoords,
    MaxVertexAttribs,
    MaxVertexUniformComponents,
    MaxVaryingFloats,
    MaxVertexTextureImageUnits,
    MaxCombinedTextureImageUnits,
    MaxTextureImageUnits,
    MaxFragmentUniformComponents,
    MaxDrawBuffers,
    MaxVertexUniformVectors,
    MaxVaryingVectors,
    MaxFragmentUniformVectors,
    MaxVertexOutputVectors,
    MaxFragmentInputVectors,
    MinProgramTexelOffset,
    MaxProgramTexelOffset,
    MaxClipDistances,
    MaxComputeWorkGroupCountX,
    MaxComputeWorkGroupCountY,
    MaxComputeWorkGroupCountZ,
    MaxComputeWorkGroupSizeX,
    MaxComputeWorkGroupSizeY,
    MaxComputeWorkGroupSizeZ,
    MaxComputeUniformComponents,
    MaxComputeTextureImageUnits,
    MaxComputeImageUniforms,
    MaxComputeAtomicCounters,
    MaxComputeAtomicCounterBuffers,
    MaxVaryingComponents,
    MaxVertexOutputComponents,
    MaxGeometryInputComponents,
    MaxGeometryOutputComponents,
    MaxFragmentInputComponents,
    MaxImageUnits,
    MaxCombinedImageUnitsAndFragmentOutputs,
    MaxCombinedShaderOutputResources,
    MaxImageSamples,
    MaxVertexImageUniforms,
    MaxTessControlImageUniforms,
    MaxTessEvaluationImageUniforms,
    MaxGeometryImageUniforms,
    MaxFragmentImageUniforms,
    MaxCombinedImageUniforms,
    MaxGeometryTextureImageUnits,
    MaxGeometryOutputVertices,
    MaxGeometryTotalOutputComponents,
    MaxGeometryUniformComponents,
    MaxGeometryVaryingComponents,
    MaxTessControlInputComponents,
    MaxTessControlOutputComponents,
    MaxTessControlTextureImageUnits,
    MaxTessControlUniformComponents,
    MaxTessControlTotalOutputComponents,
    MaxTessEvaluationInputComponents,
    MaxTessEvaluationOutputComponents,
    MaxTessEvaluationTextureImageUnits,
    MaxTessEvaluationUniformComponents,
    MaxTessPatchComponents,
    MaxPatchVertices,
    MaxTessGenLevel,
    MaxViewports,
    MaxVertexAtomicCounters,
    MaxTessControlAtomicCounters,
    MaxTessEvaluationAtomicCounters,
    MaxGeometryAtomicCounters,
    MaxFragmentAtomicCounters,
    MaxCombinedAtomicCounters,
    MaxAtomicCounterBindings,
    MaxVertexAtomicCounterBuffers,
    MaxTessControlAtomicCounterBuffers,
    MaxTessEvaluationAtomicCounterBuffers,
    MaxGeometryAtomicCounterBuffers,
    MaxFragmentAtomicCounterBuffers,
    MaxCombinedAtomicCounterBuffers,
    MaxAtomicCounterBufferSize,
    MaxTransformFeedbackBuffers,
    MaxTransformFeedbackInterleavedComponents,
    MaxCullDistances,
    MaxCombinedClipAndCullDistances,
    MaxSamples,
}

/// An opaque object managing all compiler states.
///
/// Creating an `Compiler` object has substantial resource costs; so it is
/// recommended to keep one object around for all tasks.
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
                                 -> Result<CompilationArtifact> {
        let status = unsafe { ffi::shaderc_result_get_compilation_status(result) };
        if status == 0 {
            Ok(CompilationArtifact::new(result, is_binary))
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

    /// Compiles the given source string `source_text` to a SPIR-V binary
    /// module according to the given `additional_options`.
    ///
    /// The source string will be compiled into a SPIR-V binary module
    /// contained in a `CompilationArtifact` object if no error happens.
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
                              additional_options: Option<&CompileOptions>)
                              -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source_text to c string");
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
                                          additional_options.map_or(ptr::null(), |ref o| o.raw))
        };
        Compiler::handle_compilation_result(result, true)
    }

    /// Like `compile_into_spirv` but the result contains SPIR-V assembly text
    /// instead of a SPIR-V binary module.
    ///
    /// The output SPIR-V assembly string will be of the format defined in
    /// the [SPIRV-Tools](https://github.com/KhronosGroup/SPIRV-Tools/blob/master/syntax.md)
    /// project.
    pub fn compile_into_spirv_assembly(&mut self,
                                       source_text: &str,
                                       shader_kind: ShaderKind,
                                       input_file_name: &str,
                                       entry_point_name: &str,
                                       additional_options: Option<&CompileOptions>)
                                       -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source_text to c string");
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
                                                   additional_options.map_or(ptr::null(),
                                                                             |ref o| o.raw))
        };
        Compiler::handle_compilation_result(result, false)
    }

    /// Like `compile_into_spirv` but the result contains preprocessed source
    /// code instead of a SPIR-V binary module.
    pub fn preprocess(&mut self,
                      source_text: &str,
                      input_file_name: &str,
                      entry_point_name: &str,
                      additional_options: Option<&CompileOptions>)
                      -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source to c string");
        let c_file = CString::new(input_file_name)
                         .expect("cannot convert input_file_name to c string");
        let c_entry_point = CString::new(entry_point_name)
                                .expect("cannot convert entry_point_name to c string");
        let result = unsafe {
            ffi::shaderc_compile_into_preprocessed_text(self.raw,
                                                        c_source.as_ptr(),
                                                        source_size,
                                                        // Stage doesn't matter for preprocess
                                                        ShaderKind::Vertex as int32_t,
                                                        c_file.as_ptr(),
                                                        c_entry_point.as_ptr(),
                                                        additional_options.map_or(ptr::null(),
                                                                                  |ref o| o.raw))
        };
        Compiler::handle_compilation_result(result, false)
    }

    /// Assembles the given SPIR-V assembly string `source_assembly` into a
    /// SPIR-V binary module according to the given `additional_options`.
    ///
    /// The input SPIR-V assembly string should be of the format defined in
    /// the [SPIRV-Tools](https://github.com/KhronosGroup/SPIRV-Tools/blob/master/syntax.md)
    /// project.
    ///
    /// For options specified in `additional_options`, the assembling will
    /// only pick those ones suitable for assembling.
    pub fn assemble(&mut self,
                    source_assembly: &str,
                    additional_options: Option<&CompileOptions>)
                    -> Result<CompilationArtifact> {
        let source_size = source_assembly.len();
        let c_source = CString::new(source_assembly)
                           .expect("cannot convert source_assembly to c string");
        let result = unsafe {
            ffi::shaderc_assemble_into_spv(self.raw,
                                           c_source.as_ptr(),
                                           source_size,
                                           additional_options.map_or(ptr::null(), |ref o| o.raw))
        };
        Compiler::handle_compilation_result(result, true)
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
    /// The default options are:
    /// * Target environment: Vulkan
    /// * Source language: GLSL
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

    /// Sets the target enviroment to `env`, affecting which warnings or errors
    /// will be issued.
    ///
    /// The default is Vulkan if not set.
    ///
    /// `version` will be used for distinguishing between different versions
    /// of the target environment. "0" is the only supported value right now.
    pub fn set_target_env(&mut self, env: TargetEnv, version: u32) {
        unsafe { ffi::shaderc_compile_options_set_target_env(self.raw, env as int32_t, version) }
    }

    /// Sets the source language.
    ///
    /// The default is GLSL if not set.
    pub fn set_source_language(&mut self, language: SourceLanguage) {
        unsafe { ffi::shaderc_compile_options_set_source_language(self.raw, language as int32_t) }
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

    /// Sets the resource `limit` to the given `value`.
    pub fn set_limit(&mut self, limit: Limit, value: i32) {
        unsafe {
            ffi::shaderc_compile_options_set_limit(self.raw, limit as int32_t, value as c_int)
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

    /// Sets the optimization level to `level`.
    ///
    /// If mulitple invocations for this method, only the last one takes effect.
    pub fn set_optimization_level(&mut self, level: OptimizationLevel) {
        unsafe { ffi::shaderc_compile_options_set_optimization_level(self.raw, level as int32_t) }
    }

    /// Sets the compiler mode to generate debug information in the output.
    pub fn set_generate_debug_info(&mut self) {
        unsafe { ffi::shaderc_compile_options_set_generate_debug_info(self.raw) }
    }

    /// Sets the compiler mode to suppress warnings.
    ///
    /// This overrides warnings-as-errors mode: when both suppress-warnings and
    /// warnings-as-errors modes are turned on, warning messages will be
    /// inhibited, and will not be emitted as error messages.
    pub fn set_suppress_warnings(&mut self) {
        unsafe { ffi::shaderc_compile_options_set_suppress_warnings(self.raw) }
    }

    /// Sets the compiler mode to treat all warnings as errors.
    ///
    /// Note that the suppress-warnings mode overrides this.
    pub fn set_warnings_as_errors(&mut self) {
        unsafe { ffi::shaderc_compile_options_set_warnings_as_errors(self.raw) }
    }
}

impl Drop for CompileOptions {
    fn drop(&mut self) {
        unsafe { ffi::shaderc_compile_options_release(self.raw) }
    }
}

/// An opaque object containing the results of compilation.
pub struct CompilationArtifact {
    raw: *mut ffi::ShadercCompilationResult,
    is_binary: bool,
}

impl CompilationArtifact {
    fn new(result: *mut ffi::ShadercCompilationResult, is_binary: bool) -> CompilationArtifact {
        CompilationArtifact {
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

impl Drop for CompilationArtifact {
    fn drop(&mut self) {
        unsafe { ffi::shaderc_result_release(self.raw) }
    }
}

/// Returns the version and revision of the SPIR-V generated by this library.
///
/// The version number is a 32-bit word with the following four bytes
/// (high-order to low-order): `0 | Major Number | Minor Number | 0`.
/// So version 1.00 is of value 0x00010000.
pub fn get_spirv_version() -> (u32, u32) {
    let mut version: i32 = 0;
    let mut revision: i32 = 0;
    unsafe { ffi::shaderc_get_spv_version(&mut version, &mut revision) };
    (version as u32, revision as u32)
}

/// Parses the version and profile from the given `string`.
///
/// The string should contain both version and profile, like: `450core`.
/// Returns `None` if the string can not be parsed.
pub fn parse_version_profile(string: &str) -> Option<(u32, GlslProfile)> {
    let mut version: i32 = 0;
    let mut profile: i32 = 0;
    let c_string = CString::new(string).expect("cannot convert string to c string");
    let result = unsafe {
        ffi::shaderc_parse_version_profile(c_string.as_ptr(), &mut version, &mut profile)
    };
    if result == false {
        None
    } else {
        let p = match profile {
            0 => GlslProfile::None,
            1 => GlslProfile::Core,
            2 => GlslProfile::Compatibility,
            3 => GlslProfile::Es,
            _ => panic!("internal error: unhandled profile"),
        };
        Some((version as u32, p))
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
shader.glsl:2: warning: attribute deprecated in version 130; may be removed in future release
shader.glsl:3: warning: attribute deprecated in version 130; may be removed in future release
";
    static DEBUG_INFO: &'static str = "#version 140\n \
                                       void main() {\n vec2 debug_info_sample = vec2(1.0);\n }";
    static CORE_PROFILE: &'static str = "void main() {\n gl_ClipDistance[0] = 5.;\n }";

    /// A shader that compiles under OpenGL compatibility but not core profile rules.
    static COMPAT_FRAG: &'static str = "\
#version 100
uniform highp sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, vec2(0.));
}";

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
        let result = c.compile_into_spirv(VOID_MAIN,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          None)
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_vertex_shader_into_spirv_assembly() {
        let mut c = Compiler::new().unwrap();
        let result = c.compile_into_spirv_assembly(VOID_MAIN,
                                                   ShaderKind::Vertex,
                                                   "shader.glsl",
                                                   "main",
                                                   None)
                      .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_preprocess() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some("main"));
        let result = c.preprocess(VOID_E, "shader.glsl", "main", Some(&options))
                      .unwrap();
        assert_eq!("#version 310 es\n void main(){ }\n", result.as_text());
    }

    #[test]
    fn test_assemble() {
        let mut c = Compiler::new().unwrap();
        let result = c.assemble(VOID_MAIN_ASSEMBLY, None)
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
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
                                                   Some(&options))
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
                                                   Some(&options))
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
                                                   Some(&options))
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
                                                   Some(&o))
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
                                          Some(&options))
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
                                                   Some(&options))
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
                                                   Some(&options))
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
                                                   Some(&options))
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
                                          Some(&options))
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
                                          Some(&options));
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
                                          Some(&options))
                      .unwrap();
        assert_eq!(0, result.get_num_warnings());
    }

    #[test]
    fn test_compile_options_set_warnings_as_errors() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_warnings_as_errors();
        let result = c.compile_into_spirv(TWO_WARNING,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          Some(&options));
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(2, ref s))
                            if s.contains("error: attribute deprecated in version 130;"));
    }

    #[test]
    fn test_compile_options_set_target_env_ok() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_target_env(TargetEnv::OpenGLCompat, 0);
        let result = c.compile_into_spirv(COMPAT_FRAG,
                                          ShaderKind::Fragment,
                                          "shader.glsl",
                                          "main",
                                          Some(&options))
                      .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_set_target_env_err_vulkan() {
        let mut c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(COMPAT_FRAG,
                                          ShaderKind::Fragment,
                                          "shader.glsl",
                                          "main",
                                          None);
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(4, ref s))
                            if s.contains("error: #version: ES shaders for Vulkan SPIR-V \
                                           require version 310 or higher"));
    }

    #[test]
    fn test_compile_options_set_target_env_err_opengl() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_target_env(TargetEnv::OpenGL, 0);
        let result = c.compile_into_spirv(COMPAT_FRAG,
                                          ShaderKind::Fragment,
                                          "shader.glsl",
                                          "main",
                                          Some(&options));
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(3, ref s))
                            if s.contains("error: #version: ES shaders for OpenGL SPIR-V \
                                           are not supported"));
    }

    /// Returns a fragment shader accessing a texture with the given offset.
    macro_rules! texture_offset {
        ($offset:expr) => ({
            let mut s = "#version 150
                         uniform sampler1D tex;
                         void main() {
                            vec4 x = textureOffset(tex, 1., ".to_string();
            s.push_str(stringify!($offset));
            s.push_str(");\n}");
            s
        })
    }

    #[test]
    fn test_compile_options_set_limit() {
        let mut c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        assert!(c.compile_into_spirv(&texture_offset!(7),
                                     ShaderKind::Fragment,
                                     "shader.glsl",
                                     "main",
                                     Some(&options))
                 .is_ok());
        assert!(c.compile_into_spirv(&texture_offset!(8),
                                     ShaderKind::Fragment,
                                     "shader.glsl",
                                     "main",
                                     Some(&options))
                 .is_err());
        options.set_limit(Limit::MaxProgramTexelOffset, 10);
        assert!(c.compile_into_spirv(&texture_offset!(8),
                                     ShaderKind::Fragment,
                                     "shader.glsl",
                                     "main",
                                     Some(&options))
                 .is_ok());
        assert!(c.compile_into_spirv(&texture_offset!(10),
                                     ShaderKind::Fragment,
                                     "shader.glsl",
                                     "main",
                                     Some(&options))
                 .is_ok());
        assert!(c.compile_into_spirv(&texture_offset!(11),
                                     ShaderKind::Fragment,
                                     "shader.glsl",
                                     "main",
                                     Some(&options))
                 .is_err());
    }

    #[test]
    fn test_error_compilation_error() {
        let mut c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(TWO_ERROR,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          None);
        assert!(result.is_err());
        assert_eq!(Some(Error::CompilationError(2, TWO_ERROR_MSG.to_string())),
                   result.err());
    }

    #[test]
    fn test_error_invalid_stage() {
        let mut c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(VOID_MAIN,
                                          ShaderKind::InferFromSource,
                                          "shader.glsl",
                                          "main",
                                          None);
        assert!(result.is_err());
        assert_eq!(Some(Error::InvalidStage("".to_string())), result.err());
    }

    #[test]
    fn test_warning() {
        let mut c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(TWO_WARNING,
                                          ShaderKind::Vertex,
                                          "shader.glsl",
                                          "main",
                                          None)
                      .unwrap();
        assert_eq!(2, result.get_num_warnings());
        assert_eq!(TWO_WARNING_MSG.to_string(), result.get_warning_messages());
    }

    #[test]
    fn test_get_spirv_version() {
        let (version, _) = get_spirv_version();
        assert_eq!(0x10000, version);
    }

    #[test]
    fn test_parse_version_profile() {
        assert_eq!(Some((310, GlslProfile::Es)), parse_version_profile("310es"));
        assert_eq!(Some((450, GlslProfile::Compatibility)),
                   parse_version_profile("450compatibility"));
        assert_eq!(Some((140, GlslProfile::None)), parse_version_profile("140"));
        assert_eq!(None, parse_version_profile("something"));
        assert_eq!(None, parse_version_profile(""));
    }
}
