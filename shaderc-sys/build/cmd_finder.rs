// Code from The Rust Project (https://github.com/rust-lang/rust)
//
// https://github.com/rust-lang/rust/blob/master/src/bootstrap/sanity.rs
//
// The Rust Project is dual-licensed under Apache 2.0 and MIT terms.

use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub struct CommandFinder {
    cache: HashMap<OsString, Option<PathBuf>>,
    path: OsString,
}

impl CommandFinder {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            path: env::var_os("PATH").unwrap_or_default(),
        }
    }

    pub fn maybe_have<S: AsRef<OsStr>>(&mut self, cmd: S) -> Option<PathBuf> {
        let cmd: OsString = cmd.as_ref().into();
        let path = self.path.clone();
        self.cache
            .entry(cmd.clone())
            .or_insert_with(|| {
                for path in env::split_paths(&path) {
                    let target = path.join(&cmd);
                    let mut cmd_alt = cmd.clone();
                    cmd_alt.push(".exe");
                    let mut symlink_is_file = false;
                    if let Ok(metadata) = target.with_extension("exe").symlink_metadata() {
                        symlink_is_file = metadata.is_file()
                    }
                    if target.is_file() || // some/path/git
                symlink_is_file ||
                target.with_extension("exe").exists() || // some/path/git.exe
                target.join(&cmd_alt).exists()
                    {
                        // some/path/git/git.exe
                        return Some(target);
                    }
                }
                None
            })
            .clone()
    }

    pub fn must_have<S: AsRef<OsStr>>(&mut self, cmd: S) -> PathBuf {
        self.maybe_have(&cmd).unwrap_or_else(|| {
            panic!("\n\ncouldn't find required command: {:?}\n\n", cmd.as_ref());
        })
    }
}
