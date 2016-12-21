use libc::{c_char, int32_t, size_t};

pub enum ShadercCompiler {}
pub enum ShadercCompileOptions {}
pub enum ShadercCompilationResult {}

#[link(name = "shaderc_combined")]
#[link(name = "c++")]
extern "C" {
    pub fn shaderc_compiler_initialize() -> *mut ShadercCompiler;
    pub fn shaderc_compiler_release(compiler: *mut ShadercCompiler);

    pub fn shaderc_compile_into_spv(compiler: *const ShadercCompiler,
                                    source_text: *const c_char,
                                    source_size: size_t,
                                    shader_kind: int32_t,
                                    input_file_name: *const c_char,
                                    entry_point_name: *const c_char,
                                    additional_options: *const ShadercCompileOptions)
                                    -> *mut ShadercCompilationResult;
    pub fn shaderc_compile_into_spv_assembly(compiler: *const ShadercCompiler,
                                             source_text: *const c_char,
                                             source_size: size_t,
                                             shader_kind: int32_t,
                                             input_file_name: *const c_char,
                                             entry_point_name: *const c_char,
                                             additional_options: *const ShadercCompileOptions)
                                             -> *mut ShadercCompilationResult;

    pub fn shaderc_compile_options_initialize() -> *mut ShadercCompileOptions;
    pub fn shaderc_compile_options_clone(options: *const ShadercCompileOptions)
                                         -> *mut ShadercCompileOptions;
    pub fn shaderc_compile_options_release(options: *mut ShadercCompileOptions);

    pub fn shaderc_result_release(result: *mut ShadercCompilationResult);
    pub fn shaderc_result_get_length(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_bytes(result: *const ShadercCompilationResult) -> *const c_char;
}
