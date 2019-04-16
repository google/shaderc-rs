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

mod cmd_finder;

use std::env;
use std::env::consts;
use std::path::{Path, PathBuf};

static COMBINED_LIB: &str = "shaderc_combined";
static COMBINED_LIB_FILE: &str = "libshaderc_combined.a";
static SPIRV_LIB: &str = "libSPIRV.a";

fn build_shaderc(shaderc_dir: &PathBuf, use_ninja: bool) -> PathBuf {
    let mut config = cmake::Config::new(shaderc_dir);
    config
        .profile("Release")
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
    config
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
        .define("CMAKE_INSTALL_LIBDIR", "lib");
    if use_ninja {
        config.generator("Ninja");
    }
    config.build()
}

fn main() {
    // Deprecated --no-defaults path inhereted from before shaderc-rs & shaderc-sys split
    if env::var("CARGO_FEATURE_CHECK_PASSTHROUGH").is_ok()
        && env::var("CARGO_FEATURE_DONT_USE_DEPRECATED").is_err()
    {
        let out_dir = env::var("OUT_DIR").unwrap();
        println!("cargo:warning=USE OF --no-defaults IS DEPRECATED BEHAVIOR.");
        println!("cargo:warning=Requested to skip building native C++ shaderc.");
        println!(
            "cargo:warning=Searching {} for shaderc_combined static lib...",
            out_dir
        );
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=shaderc_combined");
        emit_stdc_link();
        return;
    }

    // Explicit overrides first
    let mut search_dir = if let Ok(lib_dir) = env::var("SHADERC_LIB_DIR") {
        println!("cargo:warning=Using specified pre-built libshaderc");
        Some(lib_dir)
    } else if let Ok(lib_path) = env::var("SHADERC_LIB_PATH") {
        println!("cargo:warning=Using specified pre-built libshaderc");
        Some(lib_path)
    } else if let Ok(lib_path) = env::var("SHADERC_STATIC") {
        println!("cargo:warning=Using specified pre-built libshaderc");
        Some(lib_path)
    } else {
        None
    };

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    // Guess linux path for system library unless requested to do source build
    if search_dir.is_none()
        && target_os == "linux"
        && env::var("CARGO_FEATURE_BUILD_FROM_SOURCE").is_err()
    {
        println!("cargo:warning=Checking for system installed libraries");
        search_dir = Some("/usr/lib/".to_owned());
    }

    // Try to build with the static lib if a path was received or chosen
    if let Some(search_dir) = search_dir {
        let search_path = Path::new(&search_dir);
        let combined_lib_path = search_path.join(COMBINED_LIB_FILE);
        let dylib_name = format!("{}shaderc{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);
        let dylib_path = search_path.join(dylib_name.clone());

        if let Some((lib_dir, lib_name)) = {
            if combined_lib_path.exists() {
                Some((search_dir.clone(), COMBINED_LIB.to_owned()))
            } else if dylib_path.exists() {
                Some((search_dir.clone(), dylib_name))
            } else {
                None
            }
        } {
            match (target_os.as_str(), target_env.as_str()) {
                ("linux", _) => {
                    println!("cargo:rustc-link-search=native={}", lib_dir);
                    let spirv_path = search_path.join(SPIRV_LIB);
                    if spirv_path.exists() {
                        println!("cargo:warning=Found SPIRV.  Linking SPIRV & glslang");
                        println!("cargo:rustc-link-lib=static=SPIRV");
                        println!("cargo:rustc-link-lib=static=SPIRV-Tools-opt");
                        println!("cargo:rustc-link-lib=static=SPIRV-Tools");
                        println!("cargo:rustc-link-lib=glslang");
                    } else {
                        println!("cargo:warning=Only libshaderc library found.  Assuming SPIRV & glslang included.");
                    }
                    println!("cargo:rustc-link-lib=static={}", lib_name);
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("windows", "gnu") => {
                    println!("cargo:warning=Windows MinGW static builds experimental");
                    println!("cargo:rustc-link-search=native={}", lib_dir);
                    println!("cargo:rustc-link-lib=static={}", lib_name);
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("macos", _) => {
                    println!("cargo:warning=MacOS static builds experimental");
                    println!("cargo:rustc-link-search=native={}", lib_dir);
                    println!("cargo:rustc-link-lib=static={}", lib_name);
                    println!("cargo:rustc-link-lib=dylib=c++");
                    return;
                }
                (_, _) => {
                    println!("cargo:warning=Platform unsupported for pre-built libshaderc");
                }
            }
        } else {
            println!("cargo:warning=pre-built library not found");
        }
    }

    println!("cargo:warning=Falling back to from-source build");
    let mut finder = cmd_finder::CommandFinder::new();

    finder.must_have("cmake");
    finder.must_have("git");
    finder.must_have("python");

    let has_ninja = finder.maybe_have("ninja").is_some();

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

    emit_stdc_link();
}

fn emit_stdc_link() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    match (target_os.as_str(), target_env.as_str()) {
        ("linux", _) | ("windows", "gnu") => println!("cargo:rustc-link-lib=dylib=stdc++"),
        ("macos", _) => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => {}
    }
}
