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
use std::process::Command;

static SHADERC_REPO: &'static str = "https://github.com/google/shaderc";
static GLSLANG_REPO: &'static str = "https://github.com/google/glslang";
static SPIRV_TOOLS_REPO: &'static str = "https://github.com/KhronosGroup/SPIRV-Tools";
static SPIRV_HEADERS_REPO: &'static str = "https://github.com/KhronosGroup/SPIRV-Headers";

// Updated on 2017-09-30
static SHADERC_COMMIT: &'static str = "c8ba3e4694ff554f95000ec35e6e55f2ba637fb3";
static GLSLANG_COMMIT: &'static str = "44dd6a00c388ba09f7ac64a3921b4717a89f949c";
static SPIRV_TOOLS_COMMIT: &'static str = "17a843c6b0ac39edce3ca45246b78c8f47c7ebee";
static SPIRV_HEADERS_COMMIT: &'static str = "77240d9e86c6ff135f6de8c7b89a0099a2d90e16";

fn git_clone_or_update(project: &str, url: &str, commit: &str, dir: &PathBuf) {
    if dir.as_path().exists() {
        let status = Command::new("git")
            .arg("fetch")
            .current_dir(dir)
            .status()
            .expect("failed to execute git pull");
        if !status.success() {
            panic!("git pull {} failed", project)
        }
    } else {
        let status = Command::new("git")
            .args(&["clone", url, dir.to_str().unwrap()])
            .status()
            .expect("failed to execute git clone");
        if !status.success() {
            panic!("git clone {} failed", project)
        }
    }

    let status = Command::new("git")
        .args(&["checkout", commit])
        .current_dir(dir)
        .status()
        .expect("failed to execute git checkout");
    if !status.success() {
        panic!("git checkout {} failed", commit)
    }
}

fn build_shaderc(shaderc_dir: &PathBuf) -> PathBuf {
        cmake::Config::new(shaderc_dir)
            .profile("Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("SPIRV_SKIP_EXECUTABLES", "ON")
            .define("SHADERC_SKIP_TESTS", "ON")
            .build()
}

fn build_shaderc_msvc(shaderc_dir: &PathBuf) -> PathBuf {
    cmake::Config::new(shaderc_dir)
            .profile("Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("SPIRV_SKIP_EXECUTABLES", "ON")
            .define("SHADERC_SKIP_TESTS", "ON")
            // cmake-rs tries to be clever on Windows by injecting several
            // C/C++ flags, which causes problems. So I have to manually
            // define CMAKE_*_FLAGS_* here to suppress that.
            .define("CMAKE_C_FLAGS", " /nologo /EHsc")
            .define("CMAKE_CXX_FLAGS", " /nologo /EHsc")
            .define("CMAKE_C_FLAGS_RELEASE", " /nologo /EHsc")
            .define("CMAKE_CXX_FLAGS_RELEASE", " /nologo /EHsc")
            .build()
}

fn main() {
    if env::var("CARGO_FEATURE_BUILD_NATIVE_SHADERC").is_err() {
        println!("cargo:warning=requested to skip building native C++ shaderc");
        return
    }
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_dir = Path::new(&manifest_dir).join("target");
    let shaderc_dir = Path::new(&target_dir).join("native-shaderc");
    let third_party_dir = Path::new(&shaderc_dir).join("third_party");
    let glslang_dir = Path::new(&third_party_dir).join("glslang");
    let spirv_tools_dir = Path::new(&third_party_dir).join("spirv-tools");
    let external_dir = Path::new(&spirv_tools_dir).join("external");
    let spirv_headers_dir = Path::new(&external_dir).join("spirv-headers");

    git_clone_or_update("shaderc", SHADERC_REPO, SHADERC_COMMIT, &shaderc_dir);
    git_clone_or_update("glslang", GLSLANG_REPO, GLSLANG_COMMIT, &glslang_dir);
    git_clone_or_update("spirv-tools", SPIRV_TOOLS_REPO, SPIRV_TOOLS_COMMIT, &spirv_tools_dir);
    git_clone_or_update("spirv-headers", SPIRV_HEADERS_REPO, SPIRV_HEADERS_COMMIT, &spirv_headers_dir);

    let mut lib_path = if target_env == "msvc" {
        build_shaderc_msvc(&shaderc_dir)
    } else {
        build_shaderc(&shaderc_dir)
    };

    lib_path.push("lib");

    println!("cargo:rustc-link-lib=static=shaderc_combined");
    println!("cargo:rustc-link-search=native={}", lib_path.display());
}
