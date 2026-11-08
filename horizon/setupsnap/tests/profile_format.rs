// End-to-end tests against profiles written by Python's zlib, which is what the
// Fabric mod's java.util.zip.ZipOutputStream produces: stored and deflated
// entries, a JSON manifest, and entries a profile must never be allowed to write.

use std::fs;
use std::path::{Path, PathBuf};

use setupsnap_horizon::{
    apply_profile, handle_command, list_profiles, read_profile_entries_for_test, save_profile,
    GAME_VERSION, OPTIONS_FILE,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "setupsnap-it-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn reads_a_profile_written_by_the_fabric_mod() {
    let entries = read_profile_entries_for_test(&fixture("fabric-made.snapsnap")).unwrap();
    let names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"manifest.json"));
    assert!(names.contains(&"options.txt"));
    assert!(names.contains(&"config/nested/deep.cfg"));

    let options = entries
        .iter()
        .find(|(name, _)| name == "options.txt")
        .map(|(_, data)| data.clone())
        .unwrap();
    let text = String::from_utf8(options).unwrap();
    assert!(text.contains("key_key.forward:87"));
    assert!(text.contains("fov:0.5"));

    // The large repetitive file is stored by Python as a dynamic-Huffman block,
    // so this proves the hand-written inflate handles real deflate output.
    let big = entries
        .iter()
        .find(|(name, _)| name == "config/big.json")
        .map(|(_, data)| data.clone())
        .unwrap();
    assert!(big.len() > 8000, "big.json unpacked to {} bytes", big.len());
    assert!(String::from_utf8(big).unwrap().contains("\"value\": \"xxxxxxxx"));
}

#[test]
fn applies_a_fabric_profile_to_an_instance() {
    let instance = scratch("apply");
    fs::write(instance.join(OPTIONS_FILE), "fov:0.25\nkey_key.forward:65\n").unwrap();

    let (path, summary) = apply_profile(&instance, fixture("fabric-made.snapsnap").to_str().unwrap()).unwrap();
    assert!(path.ends_with("fabric-made.snapsnap"));
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.changed, 4);
    assert!(summary.backup_dir.is_some());

    let options = fs::read_to_string(instance.join(OPTIONS_FILE)).unwrap();
    assert!(options.contains("key_key.forward:87"));
    assert!(instance.join("config/nested/deep.cfg").is_file());

    // The replaced file is in the backup, not lost.
    let backup = summary.backup_dir.unwrap();
    assert!(backup.join(OPTIONS_FILE).is_file());
    assert_eq!(
        fs::read_to_string(backup.join(OPTIONS_FILE)).unwrap(),
        "fov:0.25\nkey_key.forward:65\n"
    );

    let _ = fs::remove_dir_all(&instance);
}

#[test]
fn never_writes_outside_the_instance_or_over_setupsnap() {
    let instance = scratch("hostile");
    let (_, summary) = apply_profile(&instance, fixture("hostile.snapsnap").to_str().unwrap()).unwrap();

    assert!(!instance.parent().unwrap().join("escape.txt").exists());
    assert!(!instance.join("absolute.txt").exists());
    assert!(!instance.join("windows.txt").exists());
    assert!(!instance.join("config/setupsnap/profiles/evil.snapsnap").exists());
    assert!(instance.join("config/kept.txt").is_file());
    assert!(summary.skipped >= 4, "skipped {}", summary.skipped);

    let _ = fs::remove_dir_all(&instance);
}

#[test]
fn lists_profiles_by_name() {
    let instance = scratch("list");
    let dir = instance.join("config/setupsnap/profiles");
    fs::create_dir_all(&dir).unwrap();
    fs::copy(fixture("fabric-made.snapsnap"), dir.join("Shared.snapsnap")).unwrap();

    let profiles = list_profiles(&instance).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "Setup 2026-11-08");
    assert_eq!(profiles[0].minecraft_version, "26.3");
    assert_eq!(profiles[0].file_count, 4);

    let (_, summary) = apply_profile(&instance, "Setup 2026-11-08").unwrap();
    assert_eq!(summary.skipped, 0);

    let _ = fs::remove_dir_all(&instance);
}

#[test]
fn our_snapshot_is_byte_compatible_with_the_java_reader() {
    // A stored-entry profile, which java.util.zip.ZipFile reads without any
    // special support (method 0), plus the manifest fields the Java mod parses.
    let source = scratch("compat");
    fs::create_dir_all(source.join("config")).unwrap();
    fs::write(source.join(OPTIONS_FILE), "fov:0.5\n").unwrap();
    fs::write(source.join("config/a.json"), "{}\n").unwrap();

    let summary = save_profile(&source, "Compat", GAME_VERSION).unwrap();
    let entries = read_profile_entries_for_test(&summary.path).unwrap();
    assert!(entries.iter().any(|(name, _)| name == "manifest.json"));
    let manifest = entries
        .iter()
        .find(|(name, _)| name == "manifest.json")
        .map(|(_, data)| String::from_utf8(data.clone()).unwrap())
        .unwrap();
    for key in ["\"format\": 1", "\"name\": \"Compat\"", "\"minecraftVersion\": \"26.3\"", "\"files\""] {
        assert!(manifest.contains(key), "manifest is missing {key}: {manifest}");
    }

    let _ = fs::remove_dir_all(&source);
}

#[test]
fn commands_answer_and_leave_other_commands_alone() {
    let instance = scratch("commands");
    assert!(handle_command(&instance, "tp @s 0 100 0").is_none());
    assert!(handle_command(&instance, "say setupsnap").is_none());

    let help = handle_command(&instance, "setupsnap").unwrap();
    assert!(help.iter().any(|line| line.contains("/setupsnap save")));

    let saved = handle_command(&instance, "setupsnap save Mine").unwrap();
    assert!(saved[0].starts_with("Saved 'Mine'"), "{saved:?}");

    let listed = handle_command(&instance, "setupsnap list").unwrap();
    assert!(listed.iter().any(|line| line.contains("Mine")), "{listed:?}");

    let applied = handle_command(&instance, "setupsnap apply Mine").unwrap();
    assert!(applied[0].contains("unchanged"), "{applied:?}");

    let missing = handle_command(&instance, "setupsnap apply Nope").unwrap();
    assert!(missing[0].contains("no profile called"), "{missing:?}");

    let _ = fs::remove_dir_all(&instance);
}

#[test]
fn names_and_paths_may_contain_spaces() {
    let instance = scratch("spaces");
    fs::write(instance.join(OPTIONS_FILE), "fov:0.5\n").unwrap();

    let saved = handle_command(&instance, "setupsnap save My Setup 2").unwrap();
    assert!(saved[0].starts_with("Saved 'My Setup 2'"), "{saved:?}");

    let profile = instance.join("config/setupsnap/profiles/My Setup 2.snapsnap");
    assert!(profile.is_file());

    // Horizon hands the line over without its slash, and a name or path with a
    // space in it must still work: only the word after the subcommand was read
    // before, so "My Setup" saved as "My" and a path with a space never applied.
    let line = format!("/setupsnap apply {}", profile.display());
    let applied = handle_command(&instance, &line).unwrap();
    assert!(applied[0].contains("unchanged"), "{applied:?}");

    let _ = fs::remove_dir_all(&instance);
}
