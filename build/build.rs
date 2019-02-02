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

mod cmd_finder;

use std::env;
use std::path::{Path, PathBuf};

fn build_shaderc(shaderc_dir: &PathBuf, use_ninja: bool) -> PathBuf {
    let mut config = cmake::Config::new(shaderc_dir);
    config.profile("Release")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("SPIRV_SKIP_EXECUTABLES", "ON")
        .define("SPIRV_WERROR", "OFF")
        .define("SHADERC_SKIP_TESTS", "ON")
        .define("CMAKE_INSTALL_LIBDIR", "lib");
    if use_ninja {
        config.generator("Ninja");
    }
    config.build()
}

fn build_shaderc_msvc(shaderc_dir: &PathBuf, use_ninja: bool) -> PathBuf {
    let mut config = cmake::Config::new(shaderc_dir);
    config.profile("Release")
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
        .define("CMAKE_INSTALL_LIBDIR", "lib");
    if use_ninja {
        config.generator("Ninja");
    }
    config.build()
}

fn main() {
    if env::var("CARGO_FEATURE_BUILD_NATIVE_SHADERC").is_err() {
        let out_dir = env::var("OUT_DIR").unwrap();
        println!("cargo:warning=Requested to skip building native C++ shaderc.");
        println!("cargo:warning=Searching {} for shaderc_combined static lib...", out_dir);
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=shaderc_combined");
        return;
    }

    let mut finder = cmd_finder::CommandFinder::new();

    finder.must_have("cmake");
    finder.must_have("git");
    finder.must_have("python");

    let has_ninja = finder.maybe_have("ninja").is_some();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let shaderc_dir = Path::new(&manifest_dir).join("build");

    let mut lib_path = if target_env == "msvc" {
        build_shaderc_msvc(&shaderc_dir, has_ninja)
    } else {
        build_shaderc(&shaderc_dir, has_ninja)
    };

    lib_path.push("lib");

    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=shaderc_combined");

    match (target_os.as_str(), target_env.as_str()) {
        ("linux", _) | ("windows", "gnu") => println!("cargo:rustc-link-lib=dylib=stdc++"),
        ("macos", _) => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => {}
    }
}
