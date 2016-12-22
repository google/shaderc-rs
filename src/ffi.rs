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

    pub fn shaderc_compile_options_add_macro_definition(options: *mut ShadercCompileOptions,
                                                        name: *const c_char,
                                                        name_length: size_t,
                                                        value: *const c_char,
                                                        vaule_length: size_t);
    pub fn shaderc_compile_options_set_source_language(options: *mut ShadercCompileOptions,
                                                       language: int32_t);

    pub fn shaderc_result_release(result: *mut ShadercCompilationResult);
    pub fn shaderc_result_get_compilation_status(result: *const ShadercCompilationResult) -> int32_t;
    pub fn shaderc_result_get_num_errors(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_num_warnings(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_error_message(result: *const ShadercCompilationResult) -> *const c_char;
    pub fn shaderc_result_get_length(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_bytes(result: *const ShadercCompilationResult) -> *const c_char;
}
