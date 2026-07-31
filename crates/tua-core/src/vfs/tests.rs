use super::*;

#[test]
fn open_files_overlay_disk_contents_and_close_back_to_disk() {
    let mut vfs = VirtualFileSystem::new();
    let path = PathBuf::from("game/src/player.tua");

    vfs.set_disk_file(&path, "local hp: number = 10\n", VfsSourceKind::Tua);
    vfs.open_file(
        "game\\src\\player.tua",
        "local hp: number = \"open\"\n",
        VfsSourceKind::Tua,
    );

    let snapshot = vfs.snapshot();

    assert_eq!(
        snapshot.source("./game/src/player.tua"),
        Some("local hp: number = \"open\"\n")
    );
    assert!(snapshot.is_open(&path));
    assert_eq!(
        snapshot.file_id(&path),
        snapshot.file_id("game/src/./player.tua")
    );

    vfs.close_file("game/src/player.tua");

    let snapshot = vfs.snapshot();

    assert_eq!(snapshot.source(&path), Some("local hp: number = 10\n"));
    assert!(!snapshot.is_open(&path));
}

#[test]
fn excludes_output_roots_and_ignored_paths() {
    let mut vfs = VirtualFileSystem::new();

    vfs.set_source_roots([PathBuf::from("game/src")]);
    vfs.set_output_roots([PathBuf::from("game/build")]);
    vfs.set_ignored_paths(["vendor/**".to_owned(), "*.tmp".to_owned()]);

    assert!(vfs
        .set_disk_file("game/build/main.lua", "return {}\n", VfsSourceKind::Lua)
        .is_none());
    assert!(vfs
        .set_disk_file("game/src/vendor/lib.lua", "return {}\n", VfsSourceKind::Lua)
        .is_none());
    assert!(vfs
        .set_disk_file("game/src/player.tmp", "return {}\n", VfsSourceKind::Lua)
        .is_none());
    assert!(vfs
        .set_disk_file("game/src/player.tua", "return {}\n", VfsSourceKind::Tua)
        .is_some());

    let snapshot = vfs.snapshot();

    assert_eq!(snapshot.files().count(), 1);
    assert!(snapshot.file("game/src/player.tua").is_some());
}

#[test]
fn uri_conversion_preserves_spaces_nested_paths_and_tua_uris() {
    let path = PathBuf::from("/tmp/tua game/src/player file.tua");
    let uri = file_uri_from_path(&path);

    assert_eq!(uri, "file:///tmp/tua%20game/src/player%20file.tua");
    assert_eq!(path_from_file_uri(&uri), path);
    assert_eq!(
        path_from_file_uri("file://localhost/tmp/tua%20game/src/main.tua"),
        PathBuf::from("/tmp/tua game/src/main.tua")
    );
    assert_eq!(
        file_uri_from_path("tua:/builtins/Image.setFilter.tua"),
        "tua:/builtins/Image.setFilter.tua"
    );
    assert_eq!(
        file_uri_from_path(r"C:\Users\Tua Game\src\main.tua"),
        "file:///C:/Users/Tua%20Game/src/main.tua"
    );
}

#[test]
fn normalizes_cross_platform_separators_and_dot_segments() {
    assert_eq!(
        normalize_path("game\\src\\systems\\..\\player.tua"),
        PathBuf::from("game/src/player.tua")
    );
    assert_eq!(
        path_from_file_uri("file:///tmp/tua%20game/src/../src/main.tua"),
        PathBuf::from("/tmp/tua game/src/main.tua")
    );
}

#[test]
fn normalizes_windows_file_uri_path_text() {
    assert_eq!(
        windows_path_text_from_file_uri("/C:/Tua%20Game/main.tua", "/C:/Tua Game/main.tua".into()),
        "C:/Tua Game/main.tua"
    );
    assert_eq!(
        windows_path_text_from_file_uri("server/share/main.tua", "server/share/main.tua".into()),
        "//server/share/main.tua"
    );
    assert_eq!(
        windows_path_text_from_file_uri("/tmp/main.tua", "/tmp/main.tua".into()),
        "/tmp/main.tua"
    );
}

#[cfg(windows)]
#[test]
fn uri_conversion_round_trips_windows_drive_and_unc_paths() {
    let drive_path = PathBuf::from(r"C:\Users\Tua Game\src\main.tua");
    let drive_uri = file_uri_from_path(&drive_path);

    assert_eq!(drive_uri, "file:///C:/Users/Tua%20Game/src/main.tua");
    assert_eq!(path_from_file_uri(&drive_uri), normalize_path(&drive_path));
    assert_eq!(
        path_from_file_uri("file://server/share/Tua%20Game/main.tua"),
        normalize_path(r"\\server\share\Tua Game\main.tua")
    );
}

#[test]
fn module_names_support_dot_and_slash_separators() {
    assert_eq!(
        module_name_for_path(
            "game/src",
            "game/src/entities/player.tua",
            ModulePathSeparator::Dot
        ),
        Some("entities.player".to_owned())
    );
    assert_eq!(
        module_name_for_path(
            "game/src",
            "game/src/entities/player.tua",
            ModulePathSeparator::Slash
        ),
        Some("entities/player".to_owned())
    );
    assert_eq!(
        module_name_for_path(
            "game/src",
            "game/src/systems/init.lua",
            ModulePathSeparator::Dot
        ),
        Some("systems".to_owned())
    );
}
