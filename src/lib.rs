#![feature(test)]

#[macro_use] extern crate nom;
#[macro_use] extern crate lazy_static;

extern crate regex;
extern crate colored;

pub mod test;
pub mod types;
pub mod error;
pub mod cli_helpers;

pub mod prelude {

    use std::fs;
    use std::path::PathBuf;
    use regex::Regex;
    use types::*;
    use colored::*;
    use std::process::exit;

    pub use cli_helpers::*;
    pub use error::*;

    /// Helper function to determine whether a path points  
    ///
    /// Rules:
    /// - if the file extension of that is that of an artifact, return true
    /// - if the file is executable and included in the .gitignore, return true
    /// - if the file looks like a configuration file and is in the .gitignore, return true
    /// (\.cache.*, \.conf
    /// - return false otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use libsniff::prelude::*;
    /// use std::path::PathBuf;
    ///
    /// let path_buf: PathBuf = PathBuf::from("lib.so");
    /// assert_eq!(is_artifact(&path_buf, None), true);
    /// ```
    #[cfg(not(os = "windows"))]
    pub fn is_artifact(p: &PathBuf, re: Option<&Regex>) -> bool {
        let path_str = p.clone().into_os_string().into_string().expect("OS string invalid.");
        if let Some(r) = re {
            r.is_match(&path_str)
        }
        else {
            lazy_static! {
                static ref REGEX: Regex = 
                    Regex::new(r".*?\.(a|o|ll|keter|bc|dyn_o|out|d|rlib|crate|min\.js|hi|dyn_hi|toc|aux|fdb_latexmk|fls|egg-info|whl|js_a|js_hi|js_o|so.*|dump-.*|vba|crx|cache)$")
                    .unwrap();
            }
            REGEX.is_match(&path_str)
        }
    }

    #[cfg(os = "windows")]
    pub fn is_artifact(p: PathBuf, re: Option<&Regex>) -> bool {
        lazy_static! {
            static ref REGEX: Regex = Regex::new(r".*?\.(exe|dll|ll|keter|bc|rlib|crate|min\.js|toc|aux|whl|vba|crx|out)$").unwrap();
        }
        let path_str = &p.into_os_string().into_string().expect("OS String invalid.");
        if let Some(r) = re {
            r.is_match(path_str)
        }
        else {
            REGEX.is_match(path_str)
        }
    }

    // how depth/recursion SHOULD work for artifacts: if e.g. .stack-work/ has *multiple* subdirs
    // with artifacts, then list it in place of all of them. Basically find "root nodes" of these
    // places - hard but potentially very nice? look in .gitignore?
    /// Function to process directory contents and return a `FileTree` struct.
    ///
    /// # Examples
    ///
    /// ```
    /// use libsniff::prelude::*;
    /// use std::path::PathBuf;
    /// 
    /// let path = PathBuf::from("src");
    /// let file_tree = read_all(&path, 2, None, None, None, false, true);
    /// ```
    pub fn read_all(in_paths: &PathBuf,
                          depth: u8,
                          min_bytes: Option<u64>,
                          artifact_regex: Option<&Regex>,
                          excludes: Option<&Regex>,
                          silent: bool,
                          artifacts_only: bool) -> FileTree {

        let mut tree = FileTree::new();
        let min_size = min_bytes.map(FileSize::new);

        // try to read directory contents
        if let Ok(paths) = fs::read_dir(in_paths) {

            // iterate over all the entries in the directory
            for p in paths {
                let path = p.unwrap().path(); // TODO no unwraps; idk what this error would be though.
                let path_string = path.clone().into_os_string().into_string().expect("OS String invalid."); // TODO nicer error message, mention windows/utf-8?
                let bool_loop = match excludes {
                    Some(ex) => !ex.is_match(&path_string),
                    _ => true,
                };

                // only consider path if we're not using regex excludes or if they don't match the
                // exclusion regex
                if bool_loop {

                    // if this fails, it's probably because `path` is a broken symlink
                    if let Ok(metadata) = fs::metadata(&path) {

                        // append file size/name for a file
                        if metadata.is_file() {
                            if !artifacts_only || is_artifact(&path, artifact_regex) {
                                let file_size = FileSize::new(metadata.len());
                                if let Some(b) = min_bytes {
                                    if file_size >= FileSize::new(b) {
                                            tree.push(path_string, file_size, None, depth + 1, min_size);
                                        }
                                    }
                                else {
                                    tree.push(path_string, file_size, None, depth + 1, min_size);
                                }
                            }
                        }

                        // otherwise, go deeper
                        
                        else if metadata.is_dir() {
                            let mut subtree = read_all(&path, depth + 1, min_bytes, artifact_regex, excludes, silent, artifacts_only);
                            let dir_size = subtree.file_size;
                            if let Some(b) = min_bytes {
                                if dir_size >= FileSize::new(b) {
                                    tree.push(path_string, dir_size, Some(&mut subtree), depth + 1, min_size);
                                }
                            }
                            else { tree.push(path_string, dir_size, Some(&mut subtree), depth + 1, min_size); }
                        }
                    }
                    else if !silent { println!("{}: ignoring symlink at {}", "Warning".yellow(), path.display()); }
                }
            }
        }

        // if we can't read the directory contents, figure out why
        // 1: check the path exists
        else if !in_paths.exists() {
            eprintln!("{}: path '{}' does not exist.", "Error".red(), &in_paths.display()); // FIXME check it is a directory too
            exit(0x0001);
        }
        // 2: check the path is actually a directory
        else if !in_paths.is_dir() {
            eprintln!("{}: {} is not a directory.", "Error".red(), &in_paths.display());
            exit(0x0001);
        }
        // 3: otherwise, give a warning about permissions
        else if !silent {
            eprintln!("{}: permission denied for directory: {}", "Warning".yellow(), &in_paths.display());
        }

        tree
    }
}
