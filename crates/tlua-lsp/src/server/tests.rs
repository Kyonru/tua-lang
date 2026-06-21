use super::*;

#[test]
fn exposes_server_name() {
    assert_eq!(server_name(), "tlua-lsp");
}

#[test]
fn describes_tlua_language_server() {
    assert!(server_description().contains("language server"));
}
