pub fn server_name() -> &'static str {
    "tlua-lsp"
}

pub fn server_description() -> String {
    format!("{} language server", tlua_core::project_description())
}

#[cfg(test)]
mod tests;
