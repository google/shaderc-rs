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
use std::fs;
use std::path::{Path, PathBuf};

static SHADERC_STATIC_LIB: &str = "shaderc_combined";
static SHADERC_SHARED_LIB: &str = "shaderc_shared";
static SHADERC_STATIC_LIB_FILE: &str = "libshaderc_combined.a";
static SHADERC_STATIC_LIB_FILE_MSVC: &str = "shaderc_combined.lib";

fn sdk_path() -> Option<PathBuf> {
    let target = std::env::var("TARGET").unwrap();
    use std::process::Command;

    // tvOS (and the simulator) could be added here in the future.
    let sdk = if target == "x86_64-apple-ios" || target == "i386-apple-ios" {
        "iphonesimulator"
    } else if target == "aarch64-apple-ios"
        || target == "armv7-apple-ios"
        || target == "armv7s-apple-ios"
    {
        "iphoneos"
    } else {
        return None;
    };

    let output = if let Ok(out) = Command::new("xcrun")
        .args(&["--sdk", sdk, "--show-sdk-path"])
        .output()
    {
        out.stdout
    } else {
        return None;
    };
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcrun`");
    Some(PathBuf::from(prefix_str.trim_end().to_string()))
}

fn build_shaderc(shaderc_dir: &PathBuf, use_ninja: bool, target_os: String) -> PathBuf {
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

    if target_os == "ios" {
        if let Some(path) = sdk_path() {
            config.define("CMAKE_OSX_SYSROOT", path);
        }
    }

    config.build()
}

fn build_shaderc_msvc(shaderc_dir: &PathBuf) -> PathBuf {
    let linkage = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();

    let mut config = cmake::Config::new(shaderc_dir);
    config
        .profile("Release")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("SPIRV_SKIP_EXECUTABLES", "ON")
        .define("SPIRV_WERROR", "OFF")
        .define("SHADERC_SKIP_TESTS", "ON")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .generator("Ninja");

    // cmake-rs tries to be clever on Windows by injecting several
    // C/C++ flags, which causes problems. So I have to manually
    // define CMAKE_*_FLAGS_* here to suppress that.
    let config = if linkage.contains("crt-static") {
        // statically-linked CRT
        config
            .define("CMAKE_C_FLAGS", " /nologo /EHsc /MT")
            .define("CMAKE_CXX_FLAGS", " /nologo /EHsc /MT")
            .define("CMAKE_C_FLAGS_RELEASE", " /nologo /EHsc /MT")
            .define("CMAKE_CXX_FLAGS_RELEASE", " /nologo /EHsc /MT")
    } else {
        // dynamically-linked CRT
        config
            .define("CMAKE_C_FLAGS", " /nologo /EHsc /MD")
            .define("CMAKE_CXX_FLAGS", " /nologo /EHsc /MD")
            .define("CMAKE_C_FLAGS_RELEASE", " /nologo /EHsc /MD")
            .define("CMAKE_CXX_FLAGS_RELEASE", " /nologo /EHsc /MD")
            // prevent shaderc's cmake script messes with crt flags
            .define("SHADERC_ENABLE_SHARED_CRT", "ON")
    };

    config.build()
}

fn probe_linux_lib_kind(lib_name: &str, search_dir: &PathBuf) -> Option<&'static str> {
    if search_dir.join(&format!("lib{}.so", lib_name)).exists() {
        Some("dylib")
    } else if search_dir.join(&format!("lib{}.a", lib_name)).exists() {
        Some("static")
    } else {
        None
    }
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let config_build_from_source = env::var("CARGO_FEATURE_BUILD_FROM_SOURCE").is_ok();
    let explicit_lib_dir_set = env::var("SHADERC_LIB_DIR").is_ok();

    // Initialize explicit libshaderc search directory first
    let mut search_dir = if let Ok(lib_dir) = env::var("SHADERC_LIB_DIR") {
        println!(
            "cargo:warning=shaderc: searching native shaderc libraries in '{}'",
            lib_dir
        );
        Some(lib_dir)
    } else {
        None
    };

    // If no explicit path is set and no explicit request is made to build from
    // source, check known locations before falling back to from-source-build
    if search_dir.is_none() && target_os == "linux" && !config_build_from_source {
        println!(
            "cargo:warning=shaderc: searching for native shaderc libraries on system;  \
             use '--features build-from-source' to force building from source code"
        );

        // https://wiki.ubuntu.com/MultiarchSpec
        // https://wiki.debian.org/Multiarch/Implementation
        let debian_arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap() {
            arch if arch == "x86" => "i386".to_owned(),
            arch => arch,
        };
        let debian_triple_path = format!("/usr/lib/{}-linux-gnu/", debian_arch);

        search_dir = if Path::new(&debian_triple_path).exists() {
            // Debian, Ubuntu and their derivatives.
            Some(debian_triple_path)
        } else if env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "x86_64"
            && Path::new("/usr/lib64/").exists()
        {
            // Other distributions running on x86_64 usually use this path.
            Some("/usr/lib64/".to_owned())
        } else {
            // Other distributions, not x86_64.
            Some("/usr/lib/".to_owned())
        };
    }

    // Try to build with the static lib if a path was received or chosen
    let search_dir = if let Some(search_dir) = search_dir {
        let path = Path::new(&search_dir);
        let cannonical = fs::canonicalize(&path);
        if path.is_relative() {
            println!(
                "cargo:warning=shaderc: the given search path '{:?}' is relative; \
                 path must be relative to shaderc-sys crate, \
                 likely not your current working directory",
                &path
            );
        } else if !path.is_dir() {
            println!(
                "cargo:warning=shaderc: the given search path '{:?}' is not a directory",
                &path
            );
        }
        if (cannonical.is_err()) && explicit_lib_dir_set {
            println!("cargo:warning=shaderc: {:?}", cannonical.err().unwrap());
            println!(
                "cargo:warning=shaderc: failed to canonicalize the given search path '{:?}'",
                &path
            );
            None
        } else {
            cannonical.ok()
        }
    } else {
        None
    };

    if let Some(search_dir) = search_dir {
        let search_dir_str = search_dir.to_string_lossy();

        let combined_lib_path =
            search_dir.join(if target_os == "windows" && target_env == "msvc" {
                SHADERC_STATIC_LIB_FILE_MSVC
            } else {
                SHADERC_STATIC_LIB_FILE
            });

        let dylib_name = format!(
            "{}{}{}",
            consts::DLL_PREFIX,
            SHADERC_SHARED_LIB,
            consts::DLL_SUFFIX
        );
        let dylib_path = search_dir.join(dylib_name);

        if let Some((lib_name, kind)) = {
            if combined_lib_path.exists() {
                Some((SHADERC_STATIC_LIB, "static"))
            } else if dylib_path.exists() {
                Some((SHADERC_SHARED_LIB, "dylib"))
            } else {
                None
            }
        } {
            match (target_os.as_str(), target_env.as_str()) {
                ("linux", _) => {
                    println!("cargo:rustc-link-search=native={}", search_dir_str);

                    // Libraries from the SPIRV-Tools project
                    let spirv_tools_lib = probe_linux_lib_kind("SPIRV-Tools", &search_dir);
                    let spirv_tools_opt_lib = probe_linux_lib_kind("SPIRV-Tools-opt", &search_dir);
                    // Libraries from the Glslang project
                    let spirv_lib = probe_linux_lib_kind("SPIRV", &search_dir);
                    let glslang_lib = probe_linux_lib_kind("glslang", &search_dir);

                    match (spirv_tools_lib, spirv_tools_opt_lib, spirv_lib, glslang_lib) {
                        (Some(spirv_tools), Some(spirv_tools_opt), Some(spirv), Some(glslang)) => {
                            println!(
                                "cargo:warning=shaderc: found and linking glslang and spirv-tools \
                                 libraries installed on system"
                            );
                            println!("cargo:rustc-link-lib={}=glslang", glslang);
                            println!("cargo:rustc-link-lib={}=SPIRV", spirv);
                            println!("cargo:rustc-link-lib={}=SPIRV-Tools", spirv_tools);
                            println!("cargo:rustc-link-lib={}=SPIRV-Tools-opt", spirv_tools_opt);
                        }
                        _ => {
                            println!(
                                "cargo:warning=shaderc: only found the shaderc library; \
                                 assuming libraries it depends on are built into the shaderc library."
                            );
                        }
                    }
                    println!("cargo:rustc-link-lib={}={}", kind, lib_name);
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("windows", "msvc") => {
                    println!("cargo:warning=shaderc: Windows MSVC static build is experimental");
                    println!("cargo:rustc-link-search=native={}", search_dir_str);
                    println!("cargo:rustc-link-lib={}={}", kind, lib_name);
                    return;
                }
                ("windows", "gnu") => {
                    println!("cargo:warning=shaderc: Windows MinGW static build is experimental");
                    println!("cargo:rustc-link-search=native={}", search_dir_str);
                    println!("cargo:rustc-link-lib={}={}", kind, lib_name);
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("macos", _) => {
                    println!("cargo:warning=shaderc: macOS static build is experimental");
                    println!("cargo:rustc-link-search=native={}", search_dir_str);
                    println!("cargo:rustc-link-lib={}={}", kind, lib_name);
                    println!("cargo:rustc-link-lib=dylib=c++");
                    return;
                }
                ("ios", _) => {
                    println!("cargo:warning=shaderc: macOS static build is experimental");
                    println!("cargo:rustc-link-search=native={}", search_dir_str);
                    println!("cargo:rustc-link-lib={}={}", kind, lib_name);
                    println!("cargo:rustc-link-lib=dylib=c++");
                    return;
                }
                (_, _) => {
                    println!(
                        "cargo:warning=shaderc: unsupported platform for linking against \
                         native shaderc libraries installed on system"
                    );
                }
            }
        }
    }

    if config_build_from_source {
        println!("cargo:warning=shaderc: requested to build from source");
    } else {
        println!(
            "cargo:warning=shaderc: cannot find native shaderc library on system; \
             falling back to build from source"
        );
    }

    let mut finder = cmd_finder::CommandFinder::new();

    finder.must_have("cmake");
    finder.must_have("git");
    finder.must_have("python");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let shaderc_dir = Path::new(&manifest_dir).join("build");

    let mut lib_path = if target_env == "msvc" {
        finder.must_have("ninja");
        build_shaderc_msvc(&shaderc_dir)
    } else {
        let has_ninja = finder.maybe_have("ninja").is_some();
        build_shaderc(&shaderc_dir, has_ninja, target_os)
    };

    lib_path.push("lib");
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=shaderc_combined");

    emit_std_cpp_link();
}

fn emit_std_cpp_link() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    match (target_os.as_str(), target_env.as_str()) {
        ("linux", _) | ("windows", "gnu") => println!("cargo:rustc-link-lib=dylib=stdc++"),
        ("macos", _) => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => {}
    }
}
