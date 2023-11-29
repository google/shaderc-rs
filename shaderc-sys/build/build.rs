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
static SHADERC_STATIC_LIB_FILE_UNIX: &str = "libshaderc_combined.a";
static SHADERC_STATIC_LIB_FILE_WIN: &str = "shaderc_combined.lib";
static MIN_VULKAN_SDK_VERSION: u32 = 182;

fn get_apple_sdk_path() -> Option<PathBuf> {
    let target = std::env::var("TARGET").unwrap();
    use std::process::Command;

    // tvOS (and the simulator) could be added here in the future.
    let sdk = if target == "x86_64-apple-ios"
        || target == "i386-apple-ios"
        || target == "aarch64-apple-ios-sim"
    {
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
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
    {
        out.stdout
    } else {
        return None;
    };
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcrun`");
    Some(PathBuf::from(prefix_str.trim_end().to_string()))
}

fn build_shaderc_unix(shaderc_dir: &PathBuf, use_ninja: bool, target_os: String) -> PathBuf {
    let mut config = cmake::Config::new(shaderc_dir);
    config
        .profile("Release")
        // CMake options
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        // Glslang options
        .define("ENABLE_SPVREMAPPER", "OFF")
        .define("ENABLE_GLSLANG_BINARIES", "OFF")
        // Shaderc options
        .define("SHADERC_SKIP_TESTS", "ON")
        // SPIRV-Tools options
        .define("SPIRV_SKIP_EXECUTABLES", "ON")
        .define("SPIRV_WERROR", "OFF");
    if use_ninja {
        config.generator("Ninja");
    }

    if target_os == "ios" {
        if let Some(path) = get_apple_sdk_path() {
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
        // CMake options
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        // Glslang options
        .define("ENABLE_SPVREMAPPER", "OFF")
        .define("ENABLE_GLSLANG_BINARIES", "OFF")
        // Shaderc options
        .define("SHADERC_SKIP_TESTS", "ON")
        // SPIRV-Tools options
        .define("SPIRV_SKIP_EXECUTABLES", "ON")
        .define("SPIRV_WERROR", "OFF")
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

fn check_vulkan_sdk_version(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let xml = std::fs::read_to_string(
        path.join("share")
            .join("vulkan")
            .join("registry")
            .join("vk.xml"),
    )
    .map_err(|error| format!("could not read vk.xml in $VULKAN_SDK: {error}"))?;
    let tree = roxmltree::Document::parse(&xml)
        .map_err(|error| format!("vk.xml in $VULKAN_SDK is not a valid XML document: {error}"))?;
    let version = tree
        .root()
        .descendants()
        .find(|node| node.has_tag_name("types"))
        .ok_or("invalid vk.xml in $VULKAN_SDK is invalid: missing <types> node")?
        .descendants()
        .find(|node| node.text() == Some("VK_HEADER_VERSION"))
        .ok_or("invalid vk.xml in $VULKAN_SDK is invalid: missing VK_HEADER_VERSION node")?
        .tail()
        .ok_or("invalid vk.xml in $VULKAN_SDK: no vesion string")?
        .trim()
        .parse::<u32>()?;
    if version < MIN_VULKAN_SDK_VERSION {
        return Err(Box::from(format!(
            "requires Vulkan SDK patch version to be at least {MIN_VULKAN_SDK_VERSION}"
        )));
    }
    Ok(())
}

fn host_target() -> String {
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("failed to invoke rustc");

    std::str::from_utf8(&output.stdout)
        .expect("rustc didn't return valid UTF-8")
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .expect("failed to query rustc for the host target triple")
        .to_owned()
}

fn main() {
    // Don't attempt to build shaderc native library on docs.rs when cross-compiling.
    if env::var("DOCS_RS").is_ok() {
        let is_cross_compiling = env::var("TARGET").unwrap() != host_target();

        if is_cross_compiling {
            println!(
                "cargo:warning=shaderc: docs.rs cross-compilation detected, will not attempt to \
                link against shaderc",
            );
            return;
        }
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let config_build_from_source = env::var("CARGO_FEATURE_BUILD_FROM_SOURCE").is_ok();
    let config_prefer_static_linking = env::var("CARGO_FEATURE_PREFER_STATIC_LINKING").is_ok();
    let has_explicit_set_search_dir = env::var("SHADERC_LIB_DIR").is_ok();

    // Initialize explicit shaderc search directory first.
    let mut search_dir = if let Ok(lib_dir) = env::var("SHADERC_LIB_DIR") {
        println!("cargo:warning=shaderc: searching native shaderc libraries in '{lib_dir}'");
        Some(lib_dir)
    } else {
        None
    };

    // Try to find native shaderc library from Vulkan SDK if possible.
    if search_dir.is_none() {
        search_dir = if let Ok(sdk_dir) = env::var("VULKAN_SDK") {
            check_vulkan_sdk_version(Path::new(&sdk_dir)).unwrap();
            println!("cargo:warning=shaderc: searching native shaderc libraries in Vulkan SDK '{sdk_dir}/lib'");
            Some(format!("{sdk_dir}/lib/"))
        } else {
            None
        };
    }

    // If no explicit path is set and no explicit request is made to build from
    // source, check known system locations before falling back to build from source.
    // This set `search_dir` for later usage.
    if search_dir.is_none() && !config_build_from_source {
        println!(
            "cargo:warning=shaderc: searching for native shaderc libraries on system;  \
             use '--features build-from-source' to force building from source code"
        );

        if target_os == "macos" {
            // Vulkan SDK is installed in `/usr/local/` by default on macOS
            let macos_path = "/usr/local/lib/";
            if Path::new(macos_path).exists() {
                search_dir = Some(macos_path.to_owned());
            }
        } else if target_os == "linux" {
            // https://wiki.ubuntu.com/MultiarchSpec
            // https://wiki.debian.org/Multiarch/Implementation
            let debian_arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap() {
                arch if arch == "x86" => "i386".to_owned(),
                arch => arch,
            };
            let debian_triple_path = format!("/usr/lib/{debian_arch}-linux-gnu/");

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
    }

    // Canonicalize the search directory first.
    let search_dir = if let Some(search_dir) = search_dir {
        let path = Path::new(&search_dir);
        let cannonical = fs::canonicalize(path);
        if path.is_relative() {
            println!(
                "cargo:warning=shaderc: the given search path '{path:?}' is relative; \
                 path must be relative to shaderc-sys crate, \
                 likely not your current working directory"
            );
        } else if !path.is_dir() {
            println!("cargo:warning=shaderc: the given search path '{path:?}' is not a directory");
        }
        if (cannonical.is_err()) && has_explicit_set_search_dir {
            println!("cargo:warning=shaderc: {:?}", cannonical.err().unwrap());
            println!(
                "cargo:warning=shaderc: failed to canonicalize the given search path '{path:?}'"
            );
            None
        } else {
            cannonical.ok()
        }
    } else {
        None
    };

    // Try to build with the dynamic or static library if a path was explicit set
    // or implicitly chosen.
    if let Some(search_dir) = search_dir {
        let search_dir_str = search_dir.to_string_lossy();

        let static_lib_path = search_dir.join(if target_os == "windows" && target_env == "msvc" {
            SHADERC_STATIC_LIB_FILE_WIN
        } else {
            SHADERC_STATIC_LIB_FILE_UNIX
        });

        let dylib_name = format!(
            "{}{}{}",
            consts::DLL_PREFIX,
            SHADERC_SHARED_LIB,
            consts::DLL_SUFFIX
        );
        let dylib_path = search_dir.join(dylib_name);

        if let Some((lib_name, lib_kind)) = {
            match (
                dylib_path.exists(),
                static_lib_path.exists(),
                config_prefer_static_linking,
            ) {
                // If dylib not exist OR prefer static lib and static lib exist, static.
                (false, true, _) | (_, true, true) => Some((SHADERC_STATIC_LIB, "static")),
                // Otherwise, if dylib exist, dynamic.
                (true, _, _) => Some((SHADERC_SHARED_LIB, "dylib")),
                // Neither dylib nor static lib exist.
                _ => None,
            }
        } {
            match (target_os.as_str(), target_env.as_str()) {
                ("linux", _) => {
                    println!("cargo:rustc-link-search=native={search_dir_str}");
                    println!("cargo:rustc-link-lib={lib_kind}={lib_name}");
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("windows", "msvc") => {
                    println!("cargo:warning=shaderc: Windows MSVC static build is experimental");
                    println!("cargo:rustc-link-search=native={search_dir_str}");
                    println!("cargo:rustc-link-lib={lib_kind}={lib_name}");
                    return;
                }
                ("windows", "gnu") => {
                    println!("cargo:warning=shaderc: Windows MinGW static build is experimental");
                    println!("cargo:rustc-link-search=native={search_dir_str}");
                    println!("cargo:rustc-link-lib={lib_kind}={lib_name}");
                    println!("cargo:rustc-link-lib=dylib=stdc++");
                    return;
                }
                ("macos", _) => {
                    println!("cargo:warning=shaderc: macOS static build is experimental");
                    println!("cargo:rustc-link-search=native={search_dir_str}");
                    println!("cargo:rustc-link-lib={lib_kind}={lib_name}");
                    println!("cargo:rustc-link-lib=dylib=c++");
                    return;
                }
                ("ios", _) => {
                    println!("cargo:warning=shaderc: macOS static build is experimental");
                    println!("cargo:rustc-link-search=native={search_dir_str}");
                    println!("cargo:rustc-link-lib={lib_kind}={lib_name}");
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
    finder
        .maybe_have("python3")
        .or(finder.maybe_have("python"))
        .unwrap_or_else(|| {
            panic!("Build requires one of `python3` or `python`");
        });

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let shaderc_dir = Path::new(&manifest_dir).join("build");

    let mut lib_path = if target_env == "msvc" {
        finder.must_have("ninja");
        build_shaderc_msvc(&shaderc_dir)
    } else {
        let has_ninja = finder.maybe_have("ninja").is_some();
        build_shaderc_unix(&shaderc_dir, has_ninja, target_os)
    };

    lib_path.push("lib");
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static={SHADERC_STATIC_LIB}");

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
