use super::*;

#[test]
fn format_init_result_reports_created_project_files() {
    let result = tua_core::InitResult {
        config_path: "tua.toml".into(),
        source_dir: "src".into(),
        main_path: "src/main.tua".into(),
        config_created: true,
        source_created: true,
        main_created: true,
    };

    let message = format_init_result(result);

    assert!(message.contains("initialized tua project"));
    assert!(message.contains("created tua.toml"));
    assert!(message.contains("created src/"));
    assert!(message.contains("created src/main.tua"));
}
