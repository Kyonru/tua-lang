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
fn effective_source_version_does_not_change_for_identical_updates() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";

    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    let version = vfs.source_version(path).unwrap();
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    assert_eq!(vfs.source_version(path), Some(version));

    vfs.open_file(path, "return 2\n", VfsSourceKind::Tua);
    let open_version = vfs.source_version(path).unwrap();
    vfs.open_file(path, "return 2\n", VfsSourceKind::Tua);
    assert_eq!(vfs.source_version(path), Some(open_version));
}

#[test]
fn disk_updates_hidden_by_an_overlay_change_version_only_when_visible() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    vfs.set_disk_file(path, "return 'disk one'\n", VfsSourceKind::Tua);
    vfs.open_file(path, "return 'overlay'\n", VfsSourceKind::Tua);
    let overlay_version = vfs.source_version(path).unwrap();

    vfs.set_disk_file(path, "return 'disk two'\n", VfsSourceKind::Tua);
    assert_eq!(vfs.source(path), Some("return 'overlay'\n"));
    assert_eq!(vfs.source_version(path), Some(overlay_version));
    assert_eq!(vfs.snapshot().source_version(path), Some(overlay_version));

    vfs.close_file(path);
    assert_eq!(vfs.source(path), Some("return 'disk two'\n"));
    assert!(vfs.source_version(path).unwrap() > overlay_version);
}

#[test]
fn opening_and_closing_an_overlay_identical_to_disk_preserves_version() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    let disk_version = vfs.source_version(path).unwrap();

    vfs.open_file(path, "return 1\n", VfsSourceKind::Tua);
    assert_eq!(vfs.source_version(path), Some(disk_version));
    assert!(vfs.snapshot().is_open(path));

    vfs.close_file(path);
    assert_eq!(vfs.source_version(path), Some(disk_version));
    assert!(!vfs.snapshot().is_open(path));
    vfs.close_file(path);
    assert_eq!(vfs.source_version(path), Some(disk_version));
}

#[test]
fn distinct_open_and_close_overlay_transitions_receive_new_versions() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    vfs.set_disk_file(path, "return 'disk'\n", VfsSourceKind::Tua);
    let disk_before_open = vfs.source_version(path).unwrap();

    vfs.open_file(path, "return 'overlay'\n", VfsSourceKind::Tua);
    let overlay = vfs.source_version(path).unwrap();
    assert!(overlay > disk_before_open);

    vfs.close_file(path);
    let disk_after_close = vfs.source_version(path).unwrap();
    assert!(disk_after_close > overlay);
}

#[test]
fn closing_an_overlay_without_a_disk_file_removes_its_version() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/untitled.tua";
    vfs.open_file(path, "return 1\n", VfsSourceKind::Tua);
    let first_open = vfs.source_version(path).unwrap();

    vfs.close_file(path);
    assert_eq!(vfs.source_version(path), None);
    assert_eq!(vfs.source(path), None);

    vfs.open_file(path, "return 1\n", VfsSourceKind::Tua);
    assert!(vfs.source_version(path).unwrap() > first_open);
}

#[test]
fn changing_only_the_effective_source_kind_receives_a_new_version() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.lua";
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Lua);
    let lua_version = vfs.source_version(path).unwrap();

    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);

    assert!(vfs.source_version(path).unwrap() > lua_version);
    assert_eq!(vfs.kind(path), Some(VfsSourceKind::Tua));
}

#[test]
fn exclusion_and_reinclusion_receives_a_new_version() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    let before_exclusion = vfs.source_version(path).unwrap();

    vfs.set_ignored_paths(["player.tua".to_owned()]);
    assert_eq!(vfs.source_version(path), None);
    assert_eq!(vfs.source(path), None);

    vfs.set_ignored_paths(Vec::<String>::new());
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    assert!(vfs.source_version(path).unwrap() > before_exclusion);
}

#[test]
fn removing_and_readding_a_path_receives_a_new_version() {
    let mut vfs = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    let before_remove = vfs.source_version(path).unwrap();

    vfs.remove_file(path);
    assert_eq!(vfs.source_version(path), None);
    vfs.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);

    assert!(vfs.source_version(path).unwrap() > before_remove);
}

#[test]
fn cloned_vfs_shares_the_allocator_and_snapshots_freeze_versions() {
    let mut coordinator = VirtualFileSystem::new();
    let path = "game/src/player.tua";
    coordinator.set_disk_file(path, "return 1\n", VfsSourceKind::Tua);
    let original_version = coordinator.source_version(path).unwrap();
    let frozen = coordinator.snapshot();
    let mut worker = coordinator.clone();

    coordinator.set_disk_file(path, "return 2\n", VfsSourceKind::Tua);
    let coordinator_version = coordinator.source_version(path).unwrap();
    worker.set_disk_file(path, "return 3\n", VfsSourceKind::Tua);
    let worker_version = worker.source_version(path).unwrap();

    assert_eq!(frozen.source_version(path), Some(original_version));
    assert_eq!(frozen.file(path).unwrap().version(), original_version);
    assert!(coordinator_version > original_version);
    assert!(worker_version > coordinator_version);
    assert_ne!(coordinator_version, worker_version);
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
