use super::*;

#[test]
fn format_init_result_reports_created_project_files() {
    let result = tlua_core::InitResult {
        config_path: "tlua.toml".into(),
        source_dir: "src".into(),
        config_created: true,
        source_created: true,
    };

    let message = format_init_result(result);

    assert!(message.contains("initialized tlua project"));
    assert!(message.contains("created tlua.toml"));
    assert!(message.contains("created src/"));
}
