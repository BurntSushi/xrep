extern crate ignore;


use std::path::Path;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder, Glob};


const IGNORE_FILE: &'static str = "tests/gitignore_tests.gitignore";


fn get_gitignore() -> Gitignore {
    let mut builder = GitignoreBuilder::new("ROOT");
    builder.add(IGNORE_FILE);
    builder.build().unwrap()
}


#[test]
fn test_gitignore_files_in_root() {
    let gitignore = get_gitignore();
    let m = |path: &str| -> Match<&Glob> { gitignore.matched(Path::new(path), false) };

    assert!(m("ROOT/file_root_1").is_ignore());
    assert!(m("ROOT/file_root_2").is_ignore());
    assert!(m("ROOT/file_root_3").is_none());
    assert!(m("ROOT/file_root_4").is_none());
    assert!(m("ROOT/file_root_5").is_none());
}


#[test]
fn test_gitignore_files_in_deep() {
    let gitignore = get_gitignore();
    let m = |path: &str| -> Match<&Glob> { gitignore.matched(Path::new(path), false) };

    assert!(m("ROOT/file_parent_dir/file_deep_1").is_ignore());
    assert!(m("ROOT/file_parent_dir/file_deep_2").is_none());
    assert!(m("ROOT/file_parent_dir/file_deep_3").is_none());
    assert!(m("ROOT/file_parent_dir/file_deep_4").is_none());
    assert!(m("ROOT/file_parent_dir/file_deep_5").is_none());
}


#[test]
fn test_gitignore_dirs_in_root() {
    let gitignore = get_gitignore();
    let m = |path: &str| -> Match<&Glob> { gitignore.matched(Path::new(path), true) };

    assert!(m("ROOT/dir_root_1").is_ignore());
    assert!(m("ROOT/dir_root_2").is_ignore());
    assert!(m("ROOT/dir_root_3").is_ignore());

    // in git, dirs don't matter, so the following line does not matter
    assert!(m("ROOT/dir_root_4").is_none());
    assert!(m("ROOT/dir_root_4/file").is_ignore());
    // FIXME: `dir_root_4/*` should also ignore all dirs under `dir_root_4`
    //assert!(m("ROOT/dir_root_4/child_dir/file").is_ignore());
    assert!(m("ROOT/dir_root_4/child_dir/file").is_none());

    // in git, dirs don't matter, so the following line does not matter
    assert!(m("ROOT/dir_root_5").is_none());
    assert!(m("ROOT/dir_root_5/file").is_ignore());
    assert!(m("ROOT/dir_root_5/child_dir/file").is_ignore());
}


#[test]
fn test_gitignore_dirs_in_deep() {
    let gitignore = get_gitignore();
    let m = |path: &str| -> Match<&Glob> { gitignore.matched(Path::new(path), true) };

    assert!(m("ROOT/dir_parent_dir/dir_deep_1").is_ignore());
    assert!(m("ROOT/dir_parent_dir/dir_deep_2").is_none());
    assert!(m("ROOT/dir_parent_dir/dir_deep_3").is_ignore());

    // in git, dirs don't matter, so the following line does not matter
    assert!(m("ROOT/dir_parent_dir/dir_deep_4").is_none());
    assert!(m("ROOT/dir_parent_dir/dir_deep_4/file").is_ignore());
    // FIXME: `dir_parent_dir/dir_deep_4/*` should also ignore all dirs under
    // `dir_parent_dir/dir_deep_4`
    //assert!(m("ROOT/dir_parent_dir/dir_deep_4/child_dir/file").is_ignore());
    assert!(m("ROOT/dir_parent_dir/dir_deep_4/child_dir/file").is_none());

    // in git, dirs don't matter, so the following line does not matter
    assert!(m("ROOT/dir_parent_dir/dir_deep_5").is_none());
    assert!(m("ROOT/dir_parent_dir/dir_deep_5/file").is_ignore());
    assert!(m("ROOT/dir_parent_dir/dir_deep_5/child_dir/file").is_ignore());
}
