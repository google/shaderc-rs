// Copyright 2017 Google Inc.
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

extern crate cmake;

use std::env;
use std::path::{Path, PathBuf};

fn build_shaderc(shaderc_dir: &PathBuf) -> PathBuf {
        cmake::Config::new(shaderc_dir)
            .profile("Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("SPIRV_SKIP_EXECUTABLES", "ON")
            .define("SPIRV_WERROR", "OFF")
            .define("SHADERC_SKIP_TESTS", "ON")
            .define("CMAKE_INSTALL_LIBDIR", "lib")
            .build()
}

fn build_shaderc_msvc(shaderc_dir: &PathBuf) -> PathBuf {
    cmake::Config::new(shaderc_dir)
            .profile("Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("SPIRV_SKIP_EXECUTABLES", "ON")
            .define("SPIRV_WERROR", "OFF")
            .define("SHADERC_SKIP_TESTS", "ON")
            // cmake-rs tries to be clever on Windows by injecting several
            // C/C++ flags, which causes problems. So I have to manually
            // define CMAKE_*_FLAGS_* here to suppress that.
            .define("CMAKE_C_FLAGS", " /nologo /EHsc")
            .define("CMAKE_CXX_FLAGS", " /nologo /EHsc")
            .define("CMAKE_C_FLAGS_RELEASE", " /nologo /EHsc")
            .define("CMAKE_CXX_FLAGS_RELEASE", " /nologo /EHsc")
            .define("CMAKE_INSTALL_LIBDIR", "lib")
            .build()
}

fn main() {
    if env::var("CARGO_FEATURE_BUILD_NATIVE_SHADERC").is_err() {
        println!("cargo:warning=requested to skip building native C++ shaderc");
        return
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let shaderc_dir = Path::new(&manifest_dir).join("build");

    let mut lib_path = if target_env == "msvc" {
        build_shaderc_msvc(&shaderc_dir)
    } else {
        build_shaderc(&shaderc_dir)
    };

    lib_path.push("lib");

    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=shaderc_combined");
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
}
