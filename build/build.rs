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

fn git_clone_or_update(project: &str, url: &str, dir: &PathBuf) {
    if dir.as_path().exists() {
        let status = Command::new("git")
            .arg("pull")
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
}

fn build_shaderc(shaderc_dir: &PathBuf) -> PathBuf {
        cmake::Config::new(shaderc_dir)
            .profile("Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("SPIRV_SKIP_EXECUTABLES", "ON")
            .define("SHADERC_SKIP_TESTS", "ON")
            .build()
}

fn main() {
    if env::var("CARGO_FEATURE_BUILD_NATIVE_SHADERC").is_ok() {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let target_dir = Path::new(&manifest_dir).join("target");
        let shaderc_dir = Path::new(&target_dir).join("native-shaderc");
        let third_party_dir = Path::new(&shaderc_dir).join("third_party");
        let glslang_dir = Path::new(&third_party_dir).join("glslang");
        let spirv_tools_dir = Path::new(&third_party_dir).join("spirv-tools");
        let external_dir = Path::new(&spirv_tools_dir).join("external");
        let spirv_headers_dir = Path::new(&external_dir).join("spirv-headers");

        git_clone_or_update("shaderc", SHADERC_REPO, &shaderc_dir);
        git_clone_or_update("glslang", GLSLANG_REPO, &glslang_dir);
        git_clone_or_update("spirv-tools", SPIRV_TOOLS_REPO, &spirv_tools_dir);
        git_clone_or_update("spirv-headers", SPIRV_HEADERS_REPO, &spirv_headers_dir);

        let mut lib_path = build_shaderc(&shaderc_dir);
        lib_path.push("lib");
        println!("cargo:rustc-link-search={}", lib_path.display());
    } else {
        println!("cargo:warning=requested to skip building native C++ shaderc");
    }

    println!("cargo:rustc-link-lib=shaderc_combined");
}
