use std::ffi::OsString;
use std::path::Path;

use glob::Pattern;

#[derive(PartialEq, Eq, Hash)]
pub enum PathMatch {
    Extension(OsString),
    Name(OsString),
    Glob(Pattern),
}

impl PathMatch {
    pub fn matches(&self, path: &Path) -> bool {
        match self {
            PathMatch::Extension(ext) => {
                if let Some(e) = path.extension() {
                    e == ext
                } else {
                    false
                }
            }
            PathMatch::Name(ful) => {
                if let Some(f) = path.file_name() {
                    f == ful
                } else {
                    false
                }
            }
            PathMatch::Glob(glb) => glb.matches_path(path),
        }
    }
}
