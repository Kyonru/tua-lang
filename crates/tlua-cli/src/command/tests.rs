use super::*;

#[test]
fn command_placeholders_are_explicitly_unimplemented() {
    let commands = [CommandName::Watch, CommandName::Lsp];

    for command in commands {
        assert!(placeholder_text(command).contains("not implemented yet"));
    }
}
