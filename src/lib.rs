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

extern crate libc;

use libc::{int32_t, uint32_t};
use std::{slice, str};
use std::ffi::{CStr, CString};

mod ffi;

/// Shader kind.
///
/// * The `Glsl<stage>` enumerants are forced shader kinds, which force the
///   compiler to compile the source code as the specified kind of shader,
///   regardless of `#pragma` annotations in the source code.
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

    /// Deduce the shader kind from `#pragma` annotation in the source code.
    ///
    /// Compiler will emit error if `#pragma` annotation is not found.
    GlslInferFromSource,

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
    pub fn new() -> Compiler {
        Compiler { raw: unsafe { ffi::shaderc_compiler_initialize() } }
    }

    pub fn compile_into_spirv(&mut self,
                              source_text: String,
                              shader_kind: ShaderKind,
                              input_file_name: String,
                              entry_point_name: String,
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

    pub fn compile_into_spirv_assembly(&mut self,
                                       source_text: String,
                                       shader_kind: ShaderKind,
                                       input_file_name: String,
                                       entry_point_name: String,
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
    fn new() -> CompileOptions {
        CompileOptions { raw: unsafe { ffi::shaderc_compile_options_initialize() } }
    }

    fn clone(&self) -> CompileOptions {
        CompileOptions { raw: unsafe { ffi::shaderc_compile_options_clone(self.raw) } }
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

    pub fn len(&self) -> usize {
        unsafe { ffi::shaderc_result_get_length(self.raw) }
    }

    pub fn as_binary(&self) -> &[u32] {
        assert!(self.is_binary);

        let num_words = self.len() / 4;

        unsafe {
            let p = ffi::shaderc_result_get_bytes(self.raw);
            slice::from_raw_parts(p as *const uint32_t, num_words)
        }
    }

    pub fn as_text(&self) -> String {
        assert!(!self.is_binary);
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

    static VOID_MAIN: &'static str = "#version 310 es\nvoid main() {}";
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
        let source = VOID_MAIN.to_string();
        let file = "shader.glsl".to_string();
        let entry_point = "main".to_string();

        let mut c = Compiler::new();
        let options = CompileOptions::new();
        let result = c.compile_into_spirv(source,
                                          ShaderKind::GlslVertex,
                                          file,
                                          entry_point,
                                          &options);
        assert!(result.len() > 20);
        assert!(result.as_binary().first() == Some(&0x07230203));
        let function_end_word: u32 = (1 << 16) | 56;
        assert!(result.as_binary().last() == Some(&function_end_word));
    }

    #[test]
    fn compile_vertex_shader_into_spirv_assembly() {
        let source = VOID_MAIN.to_string();
        let file = "shader.glsl".to_string();
        let entry_point = "main".to_string();

        let mut c = Compiler::new();
        let options = CompileOptions::new();
        let result = c.compile_into_spirv_assembly(source,
                                                   ShaderKind::GlslVertex,
                                                   file,
                                                   entry_point,
                                                   &options);
        assert_eq!(VOID_MAIN_ASSEMBLY, result.as_text());
    }
}
