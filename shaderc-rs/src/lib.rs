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

//! Rust binding for the Shaderc library.
//!
//! This crate contains the higher-level Rust-friendly interface for the
//! Shaderc library. For the lower-level C interface, please see the
//! [shaderc-sys](https://docs.rs/shaderc-sys) crate.
//!
//! The [Shaderc](https://github.com/google/shaderc) library provides an API
//! for compiling GLSL/HLSL source code to SPIRV modules. It has been shipping
//! in the Android NDK since version r12b.
//!
//! The order of preference in which the build script will attempt to obtain
//! Shaderc can be controlled by several options, which are passed through to
//! shaderc-sys when building shaderc-rs:
//!
//! 1. The option `--features build-from-source` will prevent automatic library
//!    detection and force building from source.
//! 2. If the `SHADERC_LIB_DIR` environment variable is set to
//!    `/path/to/shaderc/libs/`, it will take precedence and
//!    `libshaderc_combined.a` (and the glslang and SPIRV libraries on Linux)
//!    will be searched in the `/path/to/shaderc/libs/` directory.
//! 3. On Linux, `/usr/lib/` will be automatically searched for system libraries
//!    if none of the above were given.
//! 4. If no other option was set or succeeded, shaderc-sys will fall back to
//!    checking out and compiling a copy of Shaderc.  This procedure is quite
//!    slow.
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
//! let compiler = shaderc::Compiler::new().unwrap();
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

#[cfg(test)]
#[macro_use]
extern crate assert_matches;
extern crate libc;
extern crate shaderc_sys;

use shaderc_sys as scs;

use libc::{c_char, c_int, c_void, size_t};
use std::any::Any;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::panic;
use std::{error, fmt, ptr, result, slice, str};

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
                if c == 1 {
                    write!(f, "compilation error")?;
                } else {
                    write!(f, "{c} compilation errors")?;
                }

                if !r.is_empty() {
                    write!(f, ":{}{}", if r.contains('\n') { "\n" } else { " " }, r)?;
                }

                Ok(())
            }
            Error::InternalError(ref r) => {
                if r.is_empty() {
                    write!(f, "internal error")
                } else {
                    write!(f, "internal error: {r}")
                }
            }
            Error::InvalidStage(ref r) => {
                if r.is_empty() {
                    write!(f, "invalid stage")
                } else {
                    write!(f, "invalid stage: {r}")
                }
            }
            Error::InvalidAssembly(ref r) => {
                if r.is_empty() {
                    write!(f, "invalid assembly")
                } else {
                    write!(f, "invalid assembly: {r}")
                }
            }
            Error::NullResultObject(ref r) => {
                if r.is_empty() {
                    write!(f, "null result object")
                } else {
                    write!(f, "null result object: {r}")
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

/// Target environment version.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvVersion {
    // For Vulkan, use Vulkan's mapping of version numbers to integers.
    // See vulkan.h
    Vulkan1_0 = 1 << 22,
    Vulkan1_1 = (1 << 22) | (1 << 12),
    Vulkan1_2 = (1 << 22) | (2 << 12),
    Vulkan1_3 = (1 << 22) | (3 << 12),
    // For OpenGL, use the number from #version in shaders.
    // Currently no difference between OpenGL 4.5 and 4.6.
    // See glslang/Standalone/Standalone.cpp
    // Glslang doesn't accept a OpenGL client version of 460.
    OpenGL4_5 = 450,
    // Deprecated, WebGPU env never defined versions
    WebGPU,
}

/// The known versions of SPIR-V.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpirvVersion {
    // Use the values used for word 1 of a SPIR-V binary:
    // - bits 24 to 31: zero
    // - bits 16 to 23: major version number
    // - bits 8 to 15: minor version number
    // - bits 0 to 7: zero
    V1_0 = 0x0001_0000,
    V1_1 = 0x0001_0100,
    V1_2 = 0x0001_0200,
    V1_3 = 0x0001_0300,
    V1_4 = 0x0001_0400,
    V1_5 = 0x0001_0500,
    V1_6 = 0x0001_0600,
}

/// Source language.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceLanguage {
    GLSL,
    HLSL,
}

/// Resource kinds.
///
/// In Vulkan, resources are bound to the pipeline via descriptors with
/// numbered bindings and sets.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    /// Image and image buffer.
    Image,
    /// Pure sampler.
    Sampler,
    /// Sampled texture in GLSL, and Shader Resource View in HLSL.
    Texture,
    /// Uniform Buffer Object (UBO) in GLSL. cbuffer in HLSL.
    Buffer,
    /// Shader Storage Buffer Object (SSBO) in GLSL.
    StorageBuffer,
    /// Unordered Access View in HLSL. (Writable storage image or storage buffer.)
    UnorderedAccessView,
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

    RayGeneration,
    AnyHit,
    ClosestHit,
    Miss,
    Intersection,
    Callable,

    DefaultRayGeneration,
    DefaultAnyHit,
    DefaultClosestHit,
    DefaultMiss,
    DefaultIntersection,
    DefaultCallable,

    Task,
    Mesh,

    DefaultTask,
    DefaultMesh,
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
    Performance,
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
    MaxMeshOutputVerticesNv,
    MaxMeshOutputPrimitivesNv,
    MaxMeshWorkGroupSizeXNv,
    MaxMeshWorkGroupSizeYNv,
    MaxMeshWorkGroupSizeZNv,
    MaxTaskWorkGroupSizeXNv,
    MaxTaskWorkGroupSizeYNv,
    MaxTaskWorkGroupSizeZNv,
    MaxMeshViewCountNv,
    MaxMeshOutputVerticesExt,
    MaxMeshOutputPrimitivesExt,
    MaxMeshWorkGroupSizeXExt,
    MaxMeshWorkGroupSizeYExt,
    MaxMeshWorkGroupSizeZExt,
    MaxTaskWorkGroupSizeXExt,
    MaxTaskWorkGroupSizeYExt,
    MaxTaskWorkGroupSizeZExt,
    MaxMeshViewCountExt,
    MaxDualSourceDrawBuffersExt,
}

/// An opaque object managing all compiler states.
///
/// Creating an `Compiler` object has substantial resource costs; so it is
/// recommended to keep one object around for all tasks.
#[derive(Debug)]
pub struct Compiler {
    raw: *mut scs::ShadercCompiler,
}

unsafe impl Send for Compiler {}
unsafe impl Sync for Compiler {}

fn propagate_panic<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    PANIC_ERROR.with(|panic_error| {
        *panic_error.borrow_mut() = None;
    });
    let result = f();
    let err = PANIC_ERROR.with(|panic_error| panic_error.borrow_mut().take());
    if let Some(err) = err {
        panic::resume_unwind(err)
    } else {
        result
    }
}

/// Returns a valid UTF-8 string from a slice of bytes.
///
/// A few shaderc functions have been observed to return invalid UTF-8 strings as
/// warning/error messages, and instead of panicking and aborting execution this
/// function can be used to convert the valid parts of the byte stream to
/// a UTF-8 string
fn safe_str_from_utf8(bytes: &[u8]) -> String {
    match str::from_utf8(bytes) {
        Ok(str) => str.to_string(),
        Err(err) => {
            if err.valid_up_to() > 0 {
                format!(
                    "{} (followed by invalid UTF-8 characters)",
                    safe_str_from_utf8(&bytes[..err.valid_up_to()])
                )
            } else {
                format!("invalid UTF-8 string: {err}")
            }
        }
    }
}

impl Compiler {
    /// Returns an compiler object that can be used to compile SPIR-V modules.
    ///
    /// A return of `None` indicates that there was an error initializing
    /// the underlying compiler.
    pub fn new() -> Option<Compiler> {
        let p = unsafe { scs::shaderc_compiler_initialize() };
        if p.is_null() {
            None
        } else {
            Some(Compiler { raw: p })
        }
    }

    fn handle_compilation_result(
        result: *mut scs::ShadercCompilationResult,
        is_binary: bool,
    ) -> Result<CompilationArtifact> {
        let status = unsafe { scs::shaderc_result_get_compilation_status(result) };
        if status == 0 {
            Ok(CompilationArtifact::new(result, is_binary))
        } else {
            let num_errors = unsafe { scs::shaderc_result_get_num_errors(result) } as u32;
            let reason = unsafe {
                let p = scs::shaderc_result_get_error_message(result);
                let bytes = CStr::from_ptr(p).to_bytes();
                safe_str_from_utf8(bytes)
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
    pub fn compile_into_spirv(
        &self,
        source_text: &str,
        shader_kind: ShaderKind,
        input_file_name: &str,
        entry_point_name: &str,
        additional_options: Option<&CompileOptions>,
    ) -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source_text to c string");
        let c_file =
            CString::new(input_file_name).expect("cannot convert input_file_name to c string");
        let c_entry_point =
            CString::new(entry_point_name).expect("cannot convert entry_point_name to c string");
        propagate_panic(|| {
            let result = unsafe {
                scs::shaderc_compile_into_spv(
                    self.raw,
                    c_source.as_ptr(),
                    source_size,
                    shader_kind as i32,
                    c_file.as_ptr(),
                    c_entry_point.as_ptr(),
                    additional_options.map_or(ptr::null(), |o| o.raw),
                )
            };
            Compiler::handle_compilation_result(result, true)
        })
    }

    /// Like `compile_into_spirv` but the result contains SPIR-V assembly text
    /// instead of a SPIR-V binary module.
    ///
    /// The output SPIR-V assembly string will be of the format defined in
    /// the [SPIRV-Tools](https://github.com/KhronosGroup/SPIRV-Tools/blob/master/syntax.md)
    /// project.
    pub fn compile_into_spirv_assembly(
        &self,
        source_text: &str,
        shader_kind: ShaderKind,
        input_file_name: &str,
        entry_point_name: &str,
        additional_options: Option<&CompileOptions>,
    ) -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source_text to c string");
        let c_file =
            CString::new(input_file_name).expect("cannot convert input_file_name to c string");
        let c_entry_point =
            CString::new(entry_point_name).expect("cannot convert entry_point_name to c string");
        propagate_panic(|| {
            let result = unsafe {
                scs::shaderc_compile_into_spv_assembly(
                    self.raw,
                    c_source.as_ptr(),
                    source_size,
                    shader_kind as i32,
                    c_file.as_ptr(),
                    c_entry_point.as_ptr(),
                    additional_options.map_or(ptr::null(), |o| o.raw),
                )
            };
            Compiler::handle_compilation_result(result, false)
        })
    }

    /// Like `compile_into_spirv` but the result contains preprocessed source
    /// code instead of a SPIR-V binary module.
    pub fn preprocess(
        &self,
        source_text: &str,
        input_file_name: &str,
        entry_point_name: &str,
        additional_options: Option<&CompileOptions>,
    ) -> Result<CompilationArtifact> {
        let source_size = source_text.len();
        let c_source = CString::new(source_text).expect("cannot convert source to c string");
        let c_file =
            CString::new(input_file_name).expect("cannot convert input_file_name to c string");
        let c_entry_point =
            CString::new(entry_point_name).expect("cannot convert entry_point_name to c string");
        propagate_panic(|| {
            let result = unsafe {
                scs::shaderc_compile_into_preprocessed_text(
                    self.raw,
                    c_source.as_ptr(),
                    source_size,
                    // Stage doesn't matter for preprocess
                    ShaderKind::Vertex as i32,
                    c_file.as_ptr(),
                    c_entry_point.as_ptr(),
                    additional_options.map_or(ptr::null(), |o| o.raw),
                )
            };
            Compiler::handle_compilation_result(result, false)
        })
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
    pub fn assemble(
        &self,
        source_assembly: &str,
        additional_options: Option<&CompileOptions>,
    ) -> Result<CompilationArtifact> {
        let source_size = source_assembly.len();
        let c_source =
            CString::new(source_assembly).expect("cannot convert source_assembly to c string");
        propagate_panic(|| {
            let result = unsafe {
                scs::shaderc_assemble_into_spv(
                    self.raw,
                    c_source.as_ptr(),
                    source_size,
                    additional_options.map_or(ptr::null(), |o| o.raw),
                )
            };
            Compiler::handle_compilation_result(result, true)
        })
    }
}

impl Drop for Compiler {
    fn drop(&mut self) {
        unsafe { scs::shaderc_compiler_release(self.raw) }
    }
}

/// Include callback status.
pub type IncludeCallbackResult = result::Result<ResolvedInclude, String>;

type BoxedIncludeCallback<'a> =
    Box<dyn Fn(&str, IncludeType, &str, usize) -> IncludeCallbackResult + 'a>;

/// An opaque object managing options to compilation.
pub struct CompileOptions<'a> {
    raw: *mut scs::ShadercCompileOptions,
    include_callback_fn: Option<BoxedIncludeCallback<'a>>,
}

/// Identifies the type of include directive. `Relative` is for include directives of the form
/// `#include "..."`, and `Standard` is for include directives of the form `#include <...>`.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone, Debug)]
pub enum IncludeType {
    Relative,
    Standard,
}

/// A representation of a successfully resolved include directive, containing the name of the include
/// and its contents.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub struct ResolvedInclude {
    /// A name uniquely identifying the resolved include. Typically the absolute path of the file.
    ///
    /// This name is used in error messages and to disambiguate different includes.
    ///
    /// This field must not be empty. Compilation will panic if an empty string is provided.
    pub resolved_name: String,
    /// The content of the include to substitute the include directive with.
    pub content: String,
}

thread_local! {
    static PANIC_ERROR: RefCell<Option<Box<dyn Any + Send + 'static>>> = RefCell::new(None);
}

impl<'a> CompileOptions<'a> {
    /// Returns a default-initialized compilation options object.
    ///
    /// The default options are:
    /// * Target environment: Vulkan
    /// * Source language: GLSL
    ///
    /// A return of `None` indicates that there was an error initializing
    /// the underlying options object.
    pub fn new() -> Option<CompileOptions<'a>> {
        let p = unsafe { scs::shaderc_compile_options_initialize() };
        if p.is_null() {
            None
        } else {
            Some(CompileOptions {
                raw: p,
                include_callback_fn: None,
            })
        }
    }

    /// Returns a copy of the given compilation options object.
    ///
    /// A return of `None` indicates that there was an error copying
    /// the underlying options object.
    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> Option<CompileOptions> {
        let p = unsafe { scs::shaderc_compile_options_clone(self.raw) };
        if p.is_null() {
            None
        } else {
            Some(CompileOptions {
                raw: p,
                include_callback_fn: None,
            })
        }
    }

    /// Sets the target enviroment to `env`, affecting which warnings or errors
    /// will be issued.
    ///
    /// The default is Vulkan if not set.
    ///
    /// `version` will be used for distinguishing between different versions
    /// of the target environment.
    /// Note that EnvVersion must be cast to u32 when calling set_target_env.
    /// For example: `options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::Vulkan1_1 as u32);`
    pub fn set_target_env(&mut self, env: TargetEnv, version: u32) {
        unsafe { scs::shaderc_compile_options_set_target_env(self.raw, env as i32, version) }
    }

    /// Sets the target SPIR-V version. The generated module will use this version
    /// of SPIR-V. Each target environment determines what versions of SPIR-V
    /// it can consume. Defaults to the highest version of SPIR-V 1.0 which is
    /// required to be supported by the target environment.  E.g. Default to SPIR-V
    /// 1.0 for Vulkan 1.0 and SPIR-V 1.3 for Vulkan 1.1.
    pub fn set_target_spirv(&mut self, version: SpirvVersion) {
        unsafe { scs::shaderc_compile_options_set_target_spirv(self.raw, version as i32) }
    }

    /// Sets the source language.
    ///
    /// The default is GLSL if not set.
    pub fn set_source_language(&mut self, language: SourceLanguage) {
        unsafe { scs::shaderc_compile_options_set_source_language(self.raw, language as i32) }
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
            scs::shaderc_compile_options_set_forced_version_profile(
                self.raw,
                version as c_int,
                profile as i32,
            )
        }
    }

    /// Sets the callback for handling the `#include` directive.
    ///
    /// The arguments to the callback are the name of the source being requested,
    /// the type of include directive (`Relative` for `#include "foo"`, `Standard` for `#include <foo>`),
    /// the name of the source containing the directive and the current include depth from the original
    /// source.
    ///
    /// The return value of the callback should be `Ok` if the source was successfully found,
    /// and an `Err` containing some suitable error message to display otherwise. If the result is
    /// `Ok`, the `resolved_name` of the `ResolvedInclude` must be non-empty. All strings returned
    /// from the callback must be convertible to `CString`s, i.e. they must not contain the null
    /// character. If these conditions are not met compilation will panic.
    ///
    /// Behaviour note: If `Err` is returned for a `Relative` include request, the callback will be
    /// tried again with `Standard`, which is similar to include directive behaviour in C.
    pub fn set_include_callback<F>(&mut self, f: F)
    where
        F: Fn(&str, IncludeType, &str, usize) -> IncludeCallbackResult + 'a,
    {
        use std::mem;

        let f = Box::new(f);
        let f_ptr = &*f as *const F;
        self.include_callback_fn = Some(f as BoxedIncludeCallback<'a>);
        unsafe {
            scs::shaderc_compile_options_set_include_callbacks(
                self.raw,
                resolver::<'a, F>,
                releaser,
                f_ptr as *const c_void as *mut c_void,
            );
        }

        struct OkResultWrapper {
            source_name: CString,
            content: CString,
            wrapped: scs::shaderc_include_result,
        }

        struct ErrResultWrapper {
            error_message: CString,
            wrapped: scs::shaderc_include_result,
        }

        extern "C" fn resolver<'a, F>(
            user_data: *mut c_void,
            requested_source: *const c_char,
            type_: c_int,
            requesting_source: *const c_char,
            include_depth: size_t,
        ) -> *mut scs::shaderc_include_result
        where
            F: Fn(&str, IncludeType, &str, usize) -> IncludeCallbackResult + 'a,
        {
            let result = panic::catch_unwind(move || {
                let f = unsafe { &*(user_data as *const F) };
                let requested_source =
                    unsafe { CStr::from_ptr(requested_source).to_string_lossy() };
                let type_ = match type_ {
                    0 => IncludeType::Relative,
                    1 => IncludeType::Standard,
                    x => panic!(
                        "include callback: unknown include type returned from libshaderc: {}",
                        x
                    ),
                };
                let requesting_source =
                    unsafe { CStr::from_ptr(requesting_source).to_string_lossy() };
                match f(&requested_source, type_, &requesting_source, include_depth) {
                    Ok(ResolvedInclude {
                        resolved_name,
                        content,
                    }) => {
                        if resolved_name.is_empty() {
                            panic!("include callback: empty strings for resolved include names not allowed");
                        }
                        let mut result = Box::new(OkResultWrapper {
                            source_name: CString::new(resolved_name).expect("include callback: could not convert resolved source name to a c string"),
                            content: CString::new(content).expect("include callback: could not convert content string to a c string"),
                            wrapped: unsafe { mem::zeroed() },
                        });
                        result.wrapped = scs::shaderc_include_result {
                            source_name: result.source_name.as_ptr(),
                            source_name_length: result.source_name.as_bytes().len(),
                            content: result.content.as_ptr(),
                            content_length: result.content.as_bytes().len(),
                            user_data: &mut *result as *mut OkResultWrapper as *mut c_void,
                        };
                        let r = &mut result.wrapped as *mut scs::shaderc_include_result;
                        mem::forget(result);
                        r
                    }
                    Err(error_message) => {
                        let mut result = Box::new(ErrResultWrapper {
                            error_message: CString::new(error_message).expect(
                                "include callback: could not convert error message to a c string",
                            ),
                            wrapped: unsafe { mem::zeroed() },
                        });
                        result.wrapped = scs::shaderc_include_result {
                            source_name: CStr::from_bytes_with_nul(b"\0").unwrap().as_ptr(),
                            source_name_length: 0,
                            content: result.error_message.as_ptr(),
                            content_length: result.error_message.as_bytes().len(),
                            user_data: &mut *result as *mut ErrResultWrapper as *mut c_void,
                        };
                        let r = &mut result.wrapped as *mut scs::shaderc_include_result;
                        mem::forget(result);
                        r
                    }
                }
            });
            match result {
                Ok(r) => r,
                Err(e) => {
                    PANIC_ERROR.with(|panic_error| {
                        *panic_error.borrow_mut() = Some(e);
                    });
                    let mut result = Box::new(ErrResultWrapper {
                        error_message: CString::new("").unwrap(),
                        wrapped: unsafe { mem::zeroed() },
                    });
                    result.wrapped = scs::shaderc_include_result {
                        source_name: CStr::from_bytes_with_nul(b"\0").unwrap().as_ptr(),
                        source_name_length: 0,
                        content: result.error_message.as_ptr(),
                        content_length: 0,
                        user_data: &mut *result as *mut ErrResultWrapper as *mut c_void,
                    };
                    let r = &mut result.wrapped as *mut scs::shaderc_include_result;
                    mem::forget(result);
                    r
                }
            }
        }

        extern "C" fn releaser(_: *mut c_void, include_result: *mut scs::shaderc_include_result) {
            let user_data = unsafe { &*include_result }.user_data;
            if unsafe { &*include_result }.source_name_length == 0 {
                let wrapper = unsafe { Box::from_raw(user_data as *mut ErrResultWrapper) };
                drop(wrapper);
            } else {
                let wrapper = unsafe { Box::from_raw(user_data as *mut OkResultWrapper) };
                drop(wrapper);
            }
        }
    }

    /// Sets the resource `limit` to the given `value`.
    pub fn set_limit(&mut self, limit: Limit, value: i32) {
        unsafe { scs::shaderc_compile_options_set_limit(self.raw, limit as i32, value as c_int) }
    }

    /// Sets whether the compiler should automatically assign bindings to uniforms
    /// that aren't already explicitly bound in the shader source.
    pub fn set_auto_bind_uniforms(&mut self, auto_bind: bool) {
        unsafe {
            scs::shaderc_compile_options_set_auto_bind_uniforms(self.raw, auto_bind);
        }
    }

    /// Sets whether the compiler should automatically remove sampler variables
    /// and convert image variables to combined image-sampler variables.
    pub fn set_auto_combined_image_sampler(&mut self, auto_combine: bool) {
        unsafe {
            scs::shaderc_compile_options_set_auto_combined_image_sampler(self.raw, auto_combine);
        }
    }

    /// Sets whether the compiler should use HLSL IO mapping rules for bindings.
    ///
    /// Defaults to false.
    pub fn set_hlsl_io_mapping(&mut self, hlsl_iomap: bool) {
        unsafe {
            scs::shaderc_compile_options_set_hlsl_io_mapping(self.raw, hlsl_iomap);
        }
    }

    /// Sets whether the compiler should determine block member offsets using HLSL
    /// packing rules instead of standard GLSL rules.
    ///
    /// Defaults to false. Only affects GLSL compilation. HLSL rules are always
    /// used when compiling HLSL.
    pub fn set_hlsl_offsets(&mut self, hlsl_offsets: bool) {
        unsafe {
            scs::shaderc_compile_options_set_hlsl_offsets(self.raw, hlsl_offsets);
        }
    }

    /// Sets the base binding number used for for a resource type when automatically
    /// assigning bindings.
    ///
    /// For GLSL compilation, sets the lowest automatically assigned number.
    /// For HLSL compilation, the regsiter number assigned to the resource is added
    /// to this specified base.
    pub fn set_binding_base(&mut self, resource_kind: ResourceKind, base: u32) {
        unsafe {
            scs::shaderc_compile_options_set_binding_base(self.raw, resource_kind as i32, base);
        }
    }

    /// Like `set_binding_base`, but only takes effect when compiling the given shader stage.
    pub fn set_binding_base_for_stage(
        &mut self,
        shader_kind: ShaderKind,
        resource_kind: ResourceKind,
        base: u32,
    ) {
        unsafe {
            scs::shaderc_compile_options_set_binding_base_for_stage(
                self.raw,
                shader_kind as i32,
                resource_kind as i32,
                base,
            );
        }
    }

    /// Sets a descriptor set and binding for an HLSL register in all shader stages.
    pub fn set_hlsl_register_set_and_binding(&mut self, register: &str, set: &str, binding: &str) {
        let c_register = CString::new(register).expect("cannot convert string to c string");
        let c_set = CString::new(set).expect("cannot convert string to c string");
        let c_binding = CString::new(binding).expect("cannot convert string to c string");
        unsafe {
            scs::shaderc_compile_options_set_hlsl_register_set_and_binding(
                self.raw,
                c_register.as_ptr(),
                c_set.as_ptr(),
                c_binding.as_ptr(),
            );
        }
    }

    /// Automatically assigns locations to shader inputs and outputs.
    pub fn set_auto_map_locations(&mut self, auto_map: bool) {
        unsafe {
            scs::shaderc_compile_options_set_auto_map_locations(self.raw, auto_map);
        }
    }

    /// Like `set_hlsl_register_set_and_binding`, but only takes effect when compiling
    /// the given shader stage.
    pub fn set_hlsl_register_set_and_binding_for_stage(
        &mut self,
        kind: ShaderKind,
        register: &str,
        set: &str,
        binding: &str,
    ) {
        let c_register = CString::new(register).expect("cannot convert string to c string");
        let c_set = CString::new(set).expect("cannot convert string to c string");
        let c_binding = CString::new(binding).expect("cannot convert string to c string");
        unsafe {
            scs::shaderc_compile_options_set_hlsl_register_set_and_binding_for_stage(
                self.raw,
                kind as i32,
                c_register.as_ptr(),
                c_set.as_ptr(),
                c_binding.as_ptr(),
            );
        }
    }

    /// Sets whether the compiler should enable extension SPV_GOOGLE_hlsl_functionality1.
    pub fn set_hlsl_functionality1(&mut self, enable: bool) {
        unsafe {
            scs::shaderc_compile_options_set_hlsl_functionality1(self.raw, enable);
        }
    }

    /// Sets whether the compiler should invert position.Y output in vertex shader.
    pub fn set_invert_y(&mut self, enable: bool) {
        unsafe {
            scs::shaderc_compile_options_set_invert_y(self.raw, enable);
        }
    }

    /// Sets whether the compiler generates code for max and min builtins which,
    /// if given a NaN operand, will return the other operand. Similarly, the clamp
    /// builtin will favour the non-NaN operands, as if clamp were implemented
    /// as a composition of max and min.
    pub fn set_nan_clamp(&mut self, enable: bool) {
        unsafe {
            scs::shaderc_compile_options_set_nan_clamp(self.raw, enable);
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
        if let Some(value) = value {
            let c_value = CString::new(value).expect("cannot convert value to c string");
            unsafe {
                scs::shaderc_compile_options_add_macro_definition(
                    self.raw,
                    c_name.as_ptr(),
                    name.len(),
                    c_value.as_ptr(),
                    value.len(),
                )
            }
        } else {
            unsafe {
                scs::shaderc_compile_options_add_macro_definition(
                    self.raw,
                    c_name.as_ptr(),
                    name.len(),
                    ptr::null(),
                    0,
                )
            }
        }
    }

    /// Sets the optimization level to `level`.
    ///
    /// If mulitple invocations for this method, only the last one takes effect.
    pub fn set_optimization_level(&mut self, level: OptimizationLevel) {
        unsafe { scs::shaderc_compile_options_set_optimization_level(self.raw, level as i32) }
    }

    /// Sets the compiler mode to generate debug information in the output.
    pub fn set_generate_debug_info(&mut self) {
        unsafe { scs::shaderc_compile_options_set_generate_debug_info(self.raw) }
    }

    /// Sets the compiler mode to suppress warnings.
    ///
    /// This overrides warnings-as-errors mode: when both suppress-warnings and
    /// warnings-as-errors modes are turned on, warning messages will be
    /// inhibited, and will not be emitted as error messages.
    pub fn set_suppress_warnings(&mut self) {
        unsafe { scs::shaderc_compile_options_set_suppress_warnings(self.raw) }
    }

    /// Sets the compiler mode to treat all warnings as errors.
    ///
    /// Note that the suppress-warnings mode overrides this.
    pub fn set_warnings_as_errors(&mut self) {
        unsafe { scs::shaderc_compile_options_set_warnings_as_errors(self.raw) }
    }
}

impl<'a> Drop for CompileOptions<'a> {
    fn drop(&mut self) {
        unsafe { scs::shaderc_compile_options_release(self.raw) }
    }
}

/// An opaque object containing the results of compilation.
pub struct CompilationArtifact {
    raw: *mut scs::ShadercCompilationResult,
    is_binary: bool,
}

impl CompilationArtifact {
    fn new(result: *mut scs::ShadercCompilationResult, is_binary: bool) -> CompilationArtifact {
        CompilationArtifact {
            raw: result,
            is_binary,
        }
    }

    /// Returns the number of bytes of the compilation output data.
    pub fn len(&self) -> usize {
        unsafe { scs::shaderc_result_get_length(self.raw) }
    }

    /// Returns true if the compilation output data has a length of 0.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
            let p = scs::shaderc_result_get_bytes(self.raw);
            slice::from_raw_parts(p as *const u32, num_words)
        }
    }

    /// Returns the compilation output data as a binary slice.
    /// This method return a &[u8] that implement the Read trait.
    ///
    /// # Panics
    ///
    /// This method will panic if the compilation does not generate a
    /// binary output.
    pub fn as_binary_u8(&self) -> &[u8] {
        if !self.is_binary {
            panic!("not binary result")
        }

        assert_eq!(0, self.len() % 4);

        unsafe {
            let p = scs::shaderc_result_get_bytes(self.raw);
            slice::from_raw_parts(p as *const u8, self.len())
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
            let p = scs::shaderc_result_get_bytes(self.raw);
            let bytes = CStr::from_ptr(p).to_bytes();
            str::from_utf8(bytes)
                .expect("invalid utf-8 string")
                .to_string()
        }
    }

    /// Returns the number of warnings generated during the compilation.
    pub fn get_num_warnings(&self) -> u32 {
        (unsafe { scs::shaderc_result_get_num_warnings(self.raw) }) as u32
    }

    /// Returns the detailed warnings as a string.
    pub fn get_warning_messages(&self) -> String {
        unsafe {
            let p = scs::shaderc_result_get_error_message(self.raw);
            let bytes = CStr::from_ptr(p).to_bytes();
            safe_str_from_utf8(bytes)
        }
    }
}

impl Drop for CompilationArtifact {
    fn drop(&mut self) {
        unsafe { scs::shaderc_result_release(self.raw) }
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
    unsafe { scs::shaderc_get_spv_version(&mut version, &mut revision) };
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
        scs::shaderc_parse_version_profile(c_string.as_ptr(), &mut version, &mut profile)
    };
    if !result {
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

    static VOID_MAIN: &str = "#version 310 es\n void main() {}";
    static VOID_E: &str = "#version 310 es\n void E() {}";
    static EXTRA_E: &str = "#version 310 es\n E\n void main() {}";
    static IFDEF_E: &str = "#version 310 es\n #ifdef E\n void main() {}\n\
                                    #else\n #error\n #endif";
    static HLSL_VERTEX: &str = "float4 main(uint index: SV_VERTEXID): SV_POSITION\n\
                                        { return float4(1., 2., 3., 4.); }";
    static TWO_ERROR: &str = "#version 310 es\n #error one\n #error two\n void main() {}";
    static TWO_ERROR_MSG: &str = "shader.glsl:2: error: '#error' : one\n\
                                          shader.glsl:3: error: '#error' : two\n";
    static ONE_WARNING: &str = "#version 400\n\
                                        layout(location = 0) attribute float x;\n void main() {}";
    static ONE_WARNING_MSG: &str = "\
shader.glsl:2: warning: attribute deprecated in version 130; may be removed in future release
";
    static DEBUG_INFO: &str = "#version 140\n \
                                       void main() {\n vec2 debug_info_sample = vec2(1.0);\n }";
    static CORE_PROFILE: &str = "void main() {\n gl_ClipDistance[0] = 5.;\n }";

    static TWO_FN: &str = "\
#version 450
layout(location=0) in  int inVal;
layout(location=0) out int outVal;
int foo(int a) { return a; }
void main() { outVal = foo(inVal); }";

    /// A shader that compiles under OpenGL compatibility but not core profile rules.
    static COMPAT_FRAG: &str = "\
#version 100
layout(binding = 0) uniform highp sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, vec2(0.));
}";

    static VOID_MAIN_ASSEMBLY: &str = "\
; SPIR-V
; Version: 1.0
; Generator: Google Shaderc over Glslang; 11
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

    static UNIFORMS_NO_BINDINGS: &str = "\
#version 450
#extension GL_ARB_sparse_texture2 : enable
uniform texture2D my_tex;
uniform sampler my_sam;
layout(rgba32f) uniform image2D my_img;
layout(rgba32f) uniform imageBuffer my_imbuf;
uniform block { float x; float y; } my_ubo;
void main() {
  texture(sampler2D(my_tex,my_sam),vec2(1.0));
  vec4 t;
  sparseImageLoadARB(my_img,ivec2(0),t);
  imageLoad(my_imbuf,42);
  float x = my_ubo.x;
}";

    static GLSL_EXPLICT_BINDING: &str = "\
#version 450
layout(set=0, binding=0)
buffer B { float x; vec3 y; } my_ssbo;
void main() { my_ssbo.x = 1.0; }";

    #[test]
    fn test_compile_vertex_shader_into_spirv() {
        let c = Compiler::new().unwrap();
        let result = c
            .compile_into_spirv(VOID_MAIN, ShaderKind::Vertex, "shader.glsl", "main", None)
            .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x0723_0203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_vertex_shader_into_spirv_assembly() {
        let c = Compiler::new().unwrap();
        let result = c
            .compile_into_spirv_assembly(VOID_MAIN, ShaderKind::Vertex, "shader.glsl", "main", None)
            .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_preprocess() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some("main"));
        let result = c
            .preprocess(VOID_E, "shader.glsl", "main", Some(&options))
            .unwrap();
        assert_eq!("#version 310 es\n void main(){ }\n", result.as_text());
    }

    #[test]
    fn test_assemble() {
        let c = Compiler::new().unwrap();
        let result = c.assemble(VOID_MAIN_ASSEMBLY, None).unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x0723_0203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_add_macro_definition_normal_value() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some("main"));
        let result = c
            .compile_into_spirv_assembly(
                VOID_E,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_add_macro_definition_empty_value() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", Some(""));
        let result = c
            .compile_into_spirv_assembly(
                EXTRA_E,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_add_macro_definition_no_value() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", None);
        let result = c
            .compile_into_spirv_assembly(
                IFDEF_E,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_clone() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.add_macro_definition("E", None);
        let o = options.clone().unwrap();
        let result = c
            .compile_into_spirv_assembly(
                IFDEF_E,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&o),
            )
            .unwrap();
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }

    #[test]
    fn test_compile_options_set_source_language() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_source_language(SourceLanguage::HLSL);
        let result = c
            .compile_into_spirv(
                HLSL_VERTEX,
                ShaderKind::Vertex,
                "shader.hlsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x0723_0203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_set_generate_debug_info() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_generate_debug_info();
        let result = c
            .compile_into_spirv_assembly(
                DEBUG_INFO,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(result.as_text().contains("debug_info_sample"));
    }

    #[test]
    fn test_compile_options_set_optimization_level_zero() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_optimization_level(OptimizationLevel::Zero);
        let result = c
            .compile_into_spirv_assembly(
                DEBUG_INFO,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(result.as_text().contains("OpName"));
        assert!(result.as_text().contains("OpSource"));
    }

    #[test]
    fn test_compile_options_set_optimization_level_size() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_optimization_level(OptimizationLevel::Size);
        let result = c
            .compile_into_spirv_assembly(
                TWO_FN,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(!result.as_text().contains("OpFunctionCall"));
    }

    #[test]
    fn test_compile_options_set_optimization_level_performance() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_optimization_level(OptimizationLevel::Performance);
        let result = c
            .compile_into_spirv_assembly(
                TWO_FN,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(!result.as_text().contains("OpFunctionCall"));
    }

    #[test]
    fn test_compile_options_set_forced_version_profile_ok() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_forced_version_profile(450, GlslProfile::Core);
        let result = c
            .compile_into_spirv(
                CORE_PROFILE,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x0723_0203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn test_compile_options_set_forced_version_profile_err() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_forced_version_profile(310, GlslProfile::Es);
        let result = c.compile_into_spirv(
            CORE_PROFILE,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(3, ref s))
                            if s.contains("error: 'gl_ClipDistance' : undeclared identifier"));
    }

    #[test]
    #[should_panic(expected = "Panic in include resolver!")]
    fn test_include_directive_panic() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_include_callback(|_, _, _, _| panic!("Panic in include resolver!"));
        drop(c.compile_into_spirv_assembly(
            r#"
            #version 400
            #include "foo.glsl"
            "#,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        ));
    }

    #[test]
    fn test_include_directive_err() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options
            .set_include_callback(|name, _, _, _| Err(format!("Couldn't find header \"{name}\"")));
        let result = c.compile_into_spirv_assembly(
            r#"
            #version 400
            #include "foo.glsl"
            "#,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
            Some(Error::CompilationError(1, ref s))
            if s.contains("Couldn't find header \"foo.glsl\""));
    }

    #[test]
    fn test_include_directive_success() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_include_callback(|name, type_, _, _| {
            if name == "foo.glsl" && type_ == IncludeType::Relative {
                Ok(ResolvedInclude {
                    resolved_name: "std/foo.glsl".to_string(),
                    content: r#"
                    #ifndef FOO_H
                    #define FOO_H
                    void main() {}
                    #endif
                    "#
                    .to_string(),
                })
            } else {
                Err(format!("Couldn't find header \"{name}\""))
            }
        });
        let result = c.compile_into_spirv_assembly(
            r#"
            #version 400
            #include "foo.glsl"
            #include "foo.glsl"
            "#,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert_matches!(result.err(), None);
    }

    #[test]
    fn test_compile_options_set_suppress_warnings() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_suppress_warnings();
        let result = c
            .compile_into_spirv(
                ONE_WARNING,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        assert_eq!(0, result.get_num_warnings());
    }

    #[test]
    fn test_compile_options_set_warnings_as_errors() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_warnings_as_errors();
        let result = c.compile_into_spirv(
            ONE_WARNING,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(1, ref s))
                            if s.contains("error: attribute deprecated in version 130;"));
    }

    #[test]
    fn test_compile_options_set_target_env_err_vulkan() {
        let c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(
            COMPAT_FRAG,
            ShaderKind::Fragment,
            "shader.glsl",
            "main",
            None,
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(4, ref s))
                            if s.contains("error: #version: ES shaders for SPIR-V \
                                           require version 310 or higher"));
    }

    #[test]
    fn test_compile_options_set_target_env_err_opengl() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_target_env(TargetEnv::OpenGL, 0);
        let result = c.compile_into_spirv(
            COMPAT_FRAG,
            ShaderKind::Fragment,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(3, ref s))
                            if s.contains("error: #version: ES shaders for SPIR-V require \
                                           version 310 or higher"));
    }

    /// Returns a fragment shader accessing a texture with the given offset.
    macro_rules! texture_offset {
        ($offset:expr) => {{
            let mut s = "#version 450
                         layout (binding=0) uniform sampler1D tex;
                         void main() {
                            vec4 x = textureOffset(tex, 1., "
                .to_string();
            s.push_str(stringify!($offset));
            s.push_str(");\n}");
            s
        }};
    }

    #[test]
    fn test_compile_options_set_limit() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        assert!(c
            .compile_into_spirv(
                &texture_offset!(7),
                ShaderKind::Fragment,
                "shader.glsl",
                "main",
                Some(&options)
            )
            .is_ok());
        assert!(c
            .compile_into_spirv(
                &texture_offset!(8),
                ShaderKind::Fragment,
                "shader.glsl",
                "main",
                Some(&options)
            )
            .is_err());
        options.set_limit(Limit::MaxProgramTexelOffset, 10);
        assert!(c
            .compile_into_spirv(
                &texture_offset!(8),
                ShaderKind::Fragment,
                "shader.glsl",
                "main",
                Some(&options)
            )
            .is_ok());
        assert!(c
            .compile_into_spirv(
                &texture_offset!(10),
                ShaderKind::Fragment,
                "shader.glsl",
                "main",
                Some(&options)
            )
            .is_ok());
        assert!(c
            .compile_into_spirv(
                &texture_offset!(11),
                ShaderKind::Fragment,
                "shader.glsl",
                "main",
                Some(&options)
            )
            .is_err());
    }

    #[test]
    fn test_compile_options_set_auto_bind_uniforms_false() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_auto_bind_uniforms(false);
        let result = c.compile_into_spirv_assembly(
            UNIFORMS_NO_BINDINGS,
            ShaderKind::Vertex,
            "shader.glsl",
            "main",
            Some(&options),
        );
        assert!(result.is_err());
        assert_matches!(result.err(),
                        Some(Error::CompilationError(_, ref s))
                            if s.contains("error: 'binding' : sampler/texture/image requires layout(binding=X)"));
    }

    #[test]
    fn test_compile_options_set_auto_bind_uniforms_true() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_auto_bind_uniforms(true);
        let result = c
            .compile_into_spirv_assembly(
                UNIFORMS_NO_BINDINGS,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpDecorate %my_tex Binding 0"));
        assert!(result.contains("OpDecorate %my_sam Binding 1"));
        assert!(result.contains("OpDecorate %my_img Binding 2"));
        assert!(result.contains("OpDecorate %my_imbuf Binding 3"));
        assert!(result.contains("OpDecorate %my_ubo Binding 4"));
    }

    #[test]
    fn test_compile_options_set_hlsl_offsets_false() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_hlsl_offsets(false);
        let result = c
            .compile_into_spirv_assembly(
                GLSL_EXPLICT_BINDING,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpMemberDecorate %B 1 Offset 16"));
    }

    #[test]
    fn test_compile_options_set_hlsl_offsets_true() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_hlsl_offsets(true);
        let result = c
            .compile_into_spirv_assembly(
                GLSL_EXPLICT_BINDING,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpMemberDecorate %B 1 Offset 4"));
    }

    #[test]
    fn test_compile_options_set_binding_base() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_auto_bind_uniforms(true);
        options.set_binding_base(ResourceKind::Image, 44);
        let result = c
            .compile_into_spirv_assembly(
                UNIFORMS_NO_BINDINGS,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpDecorate %my_tex Binding 0"));
        assert!(result.contains("OpDecorate %my_sam Binding 1"));
        assert!(result.contains("OpDecorate %my_img Binding 44"));
        assert!(result.contains("OpDecorate %my_imbuf Binding 45"));
        assert!(result.contains("OpDecorate %my_ubo Binding 2"));
    }

    #[test]
    fn test_compile_options_set_binding_base_for_stage_effective() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_auto_bind_uniforms(true);
        options.set_binding_base_for_stage(ShaderKind::Vertex, ResourceKind::Texture, 100);
        let result = c
            .compile_into_spirv_assembly(
                UNIFORMS_NO_BINDINGS,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpDecorate %my_tex Binding 100"));
        assert!(result.contains("OpDecorate %my_sam Binding 0"));
        assert!(result.contains("OpDecorate %my_img Binding 1"));
        assert!(result.contains("OpDecorate %my_imbuf Binding 2"));
        assert!(result.contains("OpDecorate %my_ubo Binding 3"));
    }

    #[test]
    fn test_compile_options_set_binding_base_for_stage_ignore() {
        let c = Compiler::new().unwrap();
        let mut options = CompileOptions::new().unwrap();
        options.set_auto_bind_uniforms(true);
        options.set_binding_base_for_stage(ShaderKind::Fragment, ResourceKind::Texture, 100);
        let result = c
            .compile_into_spirv_assembly(
                UNIFORMS_NO_BINDINGS,
                ShaderKind::Vertex,
                "shader.glsl",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_text();
        assert!(result.contains("OpDecorate %my_tex Binding 0"));
        assert!(result.contains("OpDecorate %my_sam Binding 1"));
        assert!(result.contains("OpDecorate %my_img Binding 2"));
        assert!(result.contains("OpDecorate %my_imbuf Binding 3"));
        assert!(result.contains("OpDecorate %my_ubo Binding 4"));
    }

    #[test]
    fn test_error_compilation_error() {
        let c = Compiler::new().unwrap();
        let result =
            c.compile_into_spirv(TWO_ERROR, ShaderKind::Vertex, "shader.glsl", "main", None);
        assert!(result.is_err());
        assert_eq!(
            Some(Error::CompilationError(2, TWO_ERROR_MSG.to_string())),
            result.err()
        );
    }

    #[test]
    fn test_error_invalid_stage() {
        let c = Compiler::new().unwrap();
        let result = c.compile_into_spirv(
            VOID_MAIN,
            ShaderKind::InferFromSource,
            "shader.glsl",
            "main",
            None,
        );
        assert!(result.is_err());
        assert_eq!(Some(Error::InvalidStage("".to_string())), result.err());
    }

    #[test]
    fn test_warning() {
        let c = Compiler::new().unwrap();
        let result = c
            .compile_into_spirv(ONE_WARNING, ShaderKind::Vertex, "shader.glsl", "main", None)
            .unwrap();
        assert_eq!(1, result.get_num_warnings());
        assert_eq!(ONE_WARNING_MSG.to_string(), result.get_warning_messages());
    }

    #[test]
    fn test_get_spirv_version() {
        let (version, _) = get_spirv_version();
        assert_eq!((1 << 16) + (6 << 8), version);
    }

    #[test]
    fn test_parse_version_profile() {
        assert_eq!(Some((310, GlslProfile::Es)), parse_version_profile("310es"));
        assert_eq!(
            Some((450, GlslProfile::Compatibility)),
            parse_version_profile("450compatibility")
        );
        assert_eq!(Some((140, GlslProfile::None)), parse_version_profile("140"));
        assert_eq!(None, parse_version_profile("something"));
        assert_eq!(None, parse_version_profile(""));
    }
}
