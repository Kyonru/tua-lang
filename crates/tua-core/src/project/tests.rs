use super::*;

#[test]
fn exposes_project_description() {
    assert!(project_description().contains("LuaJIT"));
}

#[test]
fn exposes_package_version() {
    assert!(!crate::VERSION.is_empty());
}
