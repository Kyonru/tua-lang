
#[test]
fn final_review_transitive_module_cache_tracks_dependency_edits() {
    let project = TempProject::new("review_transitive_cache");
    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/b.tua", "return { value = 123 }\n");
    project.write("src/a.tua", "local value = require('b').value\nreturn { value = value }\n");
    let path = project.path.join("src/a.tua");
    assert_eq!(required_module_exports_for_path(&path).unwrap().field("value"), Some(&Type::number()));
    project.write("src/b.tua", "return { value = 'changed' }\n");
    assert_eq!(required_module_exports_for_path(&path).unwrap().field("value"), Some(&Type::string()));
}
