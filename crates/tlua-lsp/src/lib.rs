pub fn server_name() -> &'static str {
    "tlua-lsp"
}

pub fn server_description() -> String {
    format!("{} language server", tlua_core::project_description())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_server_name() {
        assert_eq!(server_name(), "tlua-lsp");
    }

    #[test]
    fn describes_tlua_language_server() {
        assert!(server_description().contains("language server"));
    }
}
