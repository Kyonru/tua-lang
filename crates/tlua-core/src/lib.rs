pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn project_description() -> &'static str {
    "typed Lua source-to-source tooling for LuaJIT/LÖVE projects"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_project_description() {
        assert!(project_description().contains("LuaJIT"));
    }

    #[test]
    fn exposes_package_version() {
        assert!(!VERSION.is_empty());
    }
}
