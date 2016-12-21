extern crate libc;

use libc::{int32_t, uint32_t};
use std::{slice, str};
use std::ffi::{CStr, CString};

mod ffi;

#[repr(C)]
pub enum ShaderKind {
    GlslVertex,
    GlslFragment,
    GlslCompute,
    GlslGeometry,
    GlslTessControl,
    GlslTessEvaluation,

    GlslInferFromSource,

    GlslDefaultVertex,
    GlslDefaultFragment,
    GlslDefaultCompute,
    GlslDefaultGeometry,
    GlslDefaultTessControl,
    GlslDefaultTessEvaluation,

    SpirvAssembly,
}

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
