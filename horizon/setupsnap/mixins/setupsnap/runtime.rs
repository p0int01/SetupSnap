// SetupSnap for Horizon: the same .snapsnap profile format the Fabric mod uses.
//
// This file is both the standalone test library ("cargo test" in the folder
// above) and the module Horizon injects as crate::game::setupsnap, so it uses
// nothing but the standard library. The zip reader, the DEFLATE decoder and
// CRC-32 are all here: Horizon only lets a native mod depend on crates that are
// already in its own Cargo.lock, and a mod with no dependencies never has to.
//
// A profile is a plain zip holding manifest.json, options.txt and every file of
// config/, exactly as the Java mod writes it. Applying only ever writes the
// files the profile holds: nothing in the instance is deleted, and every file
// it replaces is copied to config/setupsnap/backups/<timestamp>/ first.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const GAME_VERSION: &str = "26.3";
pub const COMMAND_NAME: &str = "setupsnap";
pub const PROFILE_EXTENSION: &str = ".snapsnap";
pub const MANIFEST_NAME: &str = "manifest.json";
pub const OPTIONS_FILE: &str = "options.txt";

/// Nothing above this size is stored in a profile, and nothing above it is
/// written out of one.
pub const MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;
/// A profile is read into memory, so a corrupt file cannot ask for more than this.
pub const MAX_ARCHIVE_SIZE: u64 = 256 * 1024 * 1024;

/// Settings some mods keep beside options.txt instead of inside config/.
pub const EXTRA_ROOT_FILES: [&str; 4] = [
    "iris.properties",
    "optionsof.txt",
    "optionsshaders.txt",
    "sodium-options.json",
];

const OWN_ROOT: &str = "config/setupsnap";

const DEFAULT_IGNORE: [&str; 12] = [
    "# SetupSnap ignore list",
    "# One glob per line, relative to your Minecraft folder, with forward slashes.",
    "# '**' spans folders, '*' stays inside one file or folder name, '?' is one character.",
    "# Anything inside config/setupsnap is always ignored, and files over 16 MiB are skipped.",
    "# Add patterns here for files that contain tokens, passwords or machine specific paths.",
    "",
    "**/*.log",
    "**/*.tmp",
    "**/*.bak",
    "**/session.lock",
    "**/cache/**",
    "**/.cache/**",
];

/// A profile as the list shows it.
pub struct ProfileSummary {
    pub path: PathBuf,
    pub name: String,
    pub file_name: String,
    pub file_count: i64,
    pub total_bytes: u64,
    pub created_at: String,
    pub minecraft_version: String,
}

/// What one apply did.
pub struct ApplySummary {
    pub changed: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub backup_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn root_dir(instance: &Path) -> PathBuf {
    instance.join(OWN_ROOT)
}

pub fn profiles_dir(instance: &Path) -> PathBuf {
    root_dir(instance).join("profiles")
}

pub fn backups_dir(instance: &Path) -> PathBuf {
    root_dir(instance).join("backups")
}

pub fn ignore_file(instance: &Path) -> PathBuf {
    root_dir(instance).join("ignore.txt")
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

pub fn list_profiles(instance: &Path) -> Result<Vec<ProfileSummary>, String> {
    let dir = profiles_dir(instance);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("could not read {}: {e}", dir.display()))? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_owned(),
            None => continue,
        };
        let lower = file_name.to_ascii_lowercase();
        if !lower.ends_with(PROFILE_EXTENSION) && !lower.ends_with(".zip") {
            continue;
        }

        summaries.push(describe(&path, &file_name));
    }

    summaries.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(summaries)
}

fn describe(path: &Path, file_name: &str) -> ProfileSummary {
    let fallback_name = strip_extension(file_name);
    let size_on_disk = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);

    let manifest = read_zip(path).ok().and_then(|entries| {
        entries
            .into_iter()
            .find(|entry| entry.name == MANIFEST_NAME)
            .map(|entry| String::from_utf8_lossy(&entry.data).into_owned())
    });

    let Some(manifest) = manifest else {
        return ProfileSummary {
            path: path.to_path_buf(),
            name: fallback_name.clone(),
            file_name: file_name.to_owned(),
            file_count: -1,
            total_bytes: size_on_disk,
            created_at: "?".to_owned(),
            minecraft_version: "?".to_owned(),
        };
    };

    ProfileSummary {
        path: path.to_path_buf(),
        name: json_string_field(&manifest, "name").unwrap_or(fallback_name),
        file_name: file_name.to_owned(),
        file_count: json_array_len_field(&manifest, "files") as i64,
        total_bytes: size_on_disk,
        created_at: json_string_field(&manifest, "createdAt").unwrap_or_else(|| "?".to_owned()),
        minecraft_version: json_string_field(&manifest, "minecraftVersion")
            .unwrap_or_else(|| "?".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

/// Writes options.txt, the extra root settings files and config/ into a new
/// profile. Returns the summary of the file it created.
pub fn save_profile(instance: &Path, raw_name: &str, minecraft_version: &str) -> Result<ProfileSummary, String> {
    let name = sanitize_name(raw_name);
    let profiles = profiles_dir(instance);
    fs::create_dir_all(&profiles)
        .map_err(|e| format!("could not create {}: {e}", profiles.display()))?;

    let target = unique_path(&profiles, &name);
    let rules = load_ignore_rules(instance)?;
    let files = collect_files(instance, &rules)?;

    let mut manifest = String::new();
    manifest.push_str("{\n");
    manifest.push_str("  \"format\": 1,\n");
    manifest.push_str(&format!("  \"modVersion\": {},\n", json_quote("1.0.0-horizon")));
    manifest.push_str(&format!("  \"minecraftVersion\": {},\n", json_quote(minecraft_version)));
    manifest.push_str(&format!("  \"name\": {},\n", json_quote(&name)));
    manifest.push_str(&format!("  \"createdAt\": {},\n", json_quote(&format_created_at(now_secs()))));
    manifest.push_str(&format!(
        "  \"sourceGameDir\": {},\n",
        json_quote(&instance.display().to_string())
    ));
    manifest.push_str("  \"files\": [\n");
    for (index, file) in files.iter().enumerate() {
        let comma = if index + 1 == files.len() { "" } else { "," };
        manifest.push_str(&format!(
            "    {{\"path\": {}, \"size\": {}}}{}\n",
            json_quote(&file.relative),
            file.size,
            comma
        ));
    }
    manifest.push_str("  ]\n}\n");

    let temporary = target.with_file_name(format!(
        "{}.tmp",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("profile")
    ));

    let mut zip = ZipWriter::create(&temporary)?;
    zip.add(MANIFEST_NAME, manifest.as_bytes())?;
    for file in &files {
        let data = fs::read(&file.absolute)
            .map_err(|e| format!("could not read {}: {e}", file.absolute.display()))?;
        if data.len() as u64 > MAX_FILE_SIZE {
            continue;
        }
        zip.add(&file.relative, &data)?;
    }
    zip.finish()?;

    if fs::rename(&temporary, &target).is_err() {
        fs::copy(&temporary, &target).map_err(|e| format!("could not write {}: {e}", target.display()))?;
        let _ = fs::remove_file(&temporary);
    }

    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    let total: u64 = files.iter().map(|file| file.size).sum();

    Ok(ProfileSummary {
        path: target,
        name,
        file_name,
        file_count: files.len() as i64,
        total_bytes: total,
        created_at: format_created_at(now_secs()),
        minecraft_version: minecraft_version.to_owned(),
    })
}

struct SourceFile {
    relative: String,
    absolute: PathBuf,
    size: u64,
}

fn collect_files(instance: &Path, rules: &[String]) -> Result<Vec<SourceFile>, String> {
    let mut files = Vec::new();
    let own_root = root_dir(instance);

    let mut roots: Vec<PathBuf> = vec![instance.join(OPTIONS_FILE)];
    for extra in EXTRA_ROOT_FILES {
        roots.push(instance.join(extra));
    }

    for path in roots {
        if !path.is_file() {
            continue;
        }
        let Some(relative) = relative_name(instance, &path) else {
            continue;
        };
        if rules.iter().any(|rule| glob_match(rule, &relative)) {
            continue;
        }
        let Ok(size) = fs::metadata(&path).map(|meta| meta.len()) else {
            continue;
        };
        if size > MAX_FILE_SIZE {
            continue;
        }
        files.push(SourceFile { relative, absolute: path, size });
    }

    let config = instance.join("config");
    if config.is_dir() {
        walk(&config, &mut |path| {
            if !path.is_file() || path.starts_with(&own_root) {
                return;
            }
            let Some(relative) = relative_name(instance, path) else {
                return;
            };
            if rules.iter().any(|rule| glob_match(rule, &relative)) {
                return;
            }
            let Ok(size) = fs::metadata(path).map(|meta| meta.len()) else {
                return;
            };
            if size > MAX_FILE_SIZE {
                return;
            }
            files.push(SourceFile {
                relative,
                absolute: path.to_path_buf(),
                size,
            });
        });
    }

    files.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(files)
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}

fn relative_name(instance: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(instance).ok()?;
    let mut parts = Vec::new();
    for part in relative.components() {
        let text = part.as_os_str().to_str()?;
        parts.push(text);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

// ---------------------------------------------------------------------------
// Applying
// ---------------------------------------------------------------------------

pub fn find_profile(instance: &Path, query: &str) -> Result<PathBuf, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("name a profile first, or use /setupsnap list".to_owned());
    }

    let as_path = PathBuf::from(trimmed);
    if as_path.is_file() {
        return Ok(as_path);
    }

    for summary in list_profiles(instance)? {
        let stem = strip_extension(&summary.file_name);
        if summary.name.eq_ignore_ascii_case(trimmed)
            || summary.file_name.eq_ignore_ascii_case(trimmed)
            || stem.eq_ignore_ascii_case(trimmed)
        {
            return Ok(summary.path);
        }
    }

    Err(format!("no profile called '{trimmed}' (see /setupsnap list)"))
}

pub fn apply_profile(instance: &Path, query: &str) -> Result<(PathBuf, ApplySummary), String> {
    let profile = find_profile(instance, query)?;
    let entries = read_zip(&profile)?;

    let own_root = root_dir(instance);
    let backup_root = backups_dir(instance).join(format_backup_name(now_secs()));
    let mut backup_dir = None;

    let mut changed = 0usize;
    let mut unchanged = 0usize;
    let mut skipped = 0usize;

    for entry in entries {
        if entry.name == MANIFEST_NAME {
            continue;
        }

        let Some(relative) = safe_relative(&entry.name) else {
            skipped += 1;
            continue;
        };

        let destination = instance.join(&relative);
        if !destination.starts_with(instance) {
            skipped += 1;
            continue;
        }

        // A profile must never rewrite SetupSnap's own profiles, backups or ignore list.
        if destination.starts_with(&own_root) {
            skipped += 1;
            continue;
        }

        if entry.data.len() as u64 > MAX_FILE_SIZE {
            skipped += 1;
            continue;
        }

        if destination.is_file() {
            match fs::read(&destination) {
                Ok(existing) if existing == entry.data => {
                    unchanged += 1;
                    continue;
                }
                Ok(_) => {
                    let backup = backup_root.join(&relative);
                    if let Some(parent) = backup.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if fs::copy(&destination, &backup).is_ok() {
                        backup_dir = Some(backup_root.clone());
                    }
                }
                Err(_) => {}
            }
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }

        fs::write(&destination, &entry.data)
            .map_err(|e| format!("could not write {}: {e}", destination.display()))?;
        changed += 1;
    }

    Ok((
        profile,
        ApplySummary {
            changed,
            unchanged,
            skipped,
            backup_dir,
        },
    ))
}

// ---------------------------------------------------------------------------
// Ignore rules
// ---------------------------------------------------------------------------

fn load_ignore_rules(instance: &Path) -> Result<Vec<String>, String> {
    let file = ignore_file(instance);
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if !file.is_file() {
        let mut text = String::new();
        for line in DEFAULT_IGNORE {
            text.push_str(line);
            text.push('\n');
        }
        text.push_str("**/.DS_Store\n");
        text.push_str("**/Thumbs.db\n");
        let _ = fs::write(&file, text);
    }

    let Ok(text) = fs::read_to_string(&file) else {
        return Ok(Vec::new());
    };

    Ok(text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.replace('\\', "/"))
        .collect())
}

/// The glob rules of the Java mod: `**` spans folders, `*` and `?` stay inside
/// one path segment.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    glob_bytes(pattern.as_bytes(), path.as_bytes())
}

fn glob_bytes(pattern: &[u8], path: &[u8]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern.starts_with(b"**/") {
        let rest = &pattern[3..];
        if glob_bytes(rest, path) {
            return true;
        }
        for index in 0..path.len() {
            if path[index] == b'/' && glob_bytes(rest, &path[index + 1..]) {
                return true;
            }
        }
        return false;
    }

    if pattern.starts_with(b"**") {
        let rest = &pattern[2..];
        for index in 0..=path.len() {
            if glob_bytes(rest, &path[index..]) {
                return true;
            }
        }
        return false;
    }

    match pattern[0] {
        b'*' => {
            let mut index = 0;
            loop {
                if glob_bytes(&pattern[1..], &path[index..]) {
                    return true;
                }
                if index == path.len() || path[index] == b'/' {
                    return false;
                }
                index += 1;
            }
        }
        b'?' => !path.is_empty() && path[0] != b'/' && glob_bytes(&pattern[1..], &path[1..]),
        byte => !path.is_empty() && path[0] == byte && glob_bytes(&pattern[1..], &path[1..]),
    }
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// Adds "setupsnap" to the names the chat box colours and completes.
pub fn push_command_names(commands: &mut Vec<&'static str>) {
    if !commands.contains(&COMMAND_NAME) {
        commands.push(COMMAND_NAME);
    }
}

/// Handles a command line without its slash. None means "not one of ours", so
/// Horizon lets the server see it.
pub fn handle_command(instance: &Path, line: &str) -> Option<Vec<String>> {
    // Horizon hands the line over without its slash; accepting one anyway means a
    // caller that keeps it does not silently lose every command.
    let line = line.trim().trim_start_matches('/');
    let mut words = line.split_whitespace();
    if words.next()? != COMMAND_NAME {
        return None;
    }

    let rest: Vec<&str> = words.collect();
    // Everything after a subcommand is one name or path, so "My Setup" and
    // "/home/me/My Profiles/main.snapsnap" work here exactly as they do through
    // the Fabric mod's greedy argument.
    let argument = |from: usize| -> String {
        rest.get(from..)
            .map(|words| words.join(" "))
            .unwrap_or_default()
    };

    let reply = match rest.first().copied().unwrap_or("help") {
        "help" => help(),
        "save" => {
            let name = argument(1);
            match save_profile(instance, &name, GAME_VERSION) {
                Ok(summary) => vec![format!(
                    "Saved '{}' with {} files ({}).",
                    summary.name,
                    summary.file_count,
                    format_bytes(summary.total_bytes)
                )],
                Err(error) => vec![format!("Could not save: {error}")],
            }
        }
        "apply" => match argument(1).as_str() {
            "" => vec!["Usage: /setupsnap apply <name or file>".to_owned()],
            query => match apply_profile(instance, query) {
                Ok((path, summary)) => {
                    let mut lines = vec![format!(
                        "Applied '{}': {} changed, {} unchanged, {} skipped. Restart the client for every setting to take effect.",
                        strip_extension(
                            path.file_name().and_then(|name| name.to_str()).unwrap_or("profile")
                        ),
                        summary.changed,
                        summary.unchanged,
                        summary.skipped
                    )];
                    if let Some(backup) = summary.backup_dir {
                        lines.push(format!("Replaced files were backed up to {}.", backup.display()));
                    }
                    lines
                }
                Err(error) => vec![format!("Could not apply: {error}")],
            },
        },
        "list" => match list_profiles(instance) {
            Ok(profiles) if profiles.is_empty() => vec![format!(
                "No profiles yet. /setupsnap save <name> writes one to {}.",
                profiles_dir(instance).display()
            )],
            Ok(profiles) => {
                let mut lines = vec![format!("{} profile(s) in {}:", profiles.len(), profiles_dir(instance).display())];
                for profile in profiles.iter().take(12) {
                    lines.push(format!(
                        "  {} - {} files, {} ({})",
                        profile.name,
                        profile.file_count.max(0),
                        format_bytes(profile.total_bytes),
                        profile.created_at
                    ));
                }
                if profiles.len() > 12 {
                    lines.push(format!("  ... and {} more.", profiles.len() - 12));
                }
                lines
            }
            Err(error) => vec![format!("Could not list profiles: {error}")],
        },
        "folder" => vec![
            format!("Profiles: {}", profiles_dir(instance).display()),
            format!("Ignore list: {}", ignore_file(instance).display()),
        ],
        other => {
            let mut lines = vec![format!("Unknown subcommand '{other}'.")];
            lines.extend(help());
            lines
        }
    };

    Some(reply)
}

fn help() -> Vec<String> {
    vec![
        "/setupsnap save [name] - save options.txt and config/ into a profile".to_owned(),
        "/setupsnap apply <name or file> - write a profile into this instance".to_owned(),
        "/setupsnap list - show the saved profiles".to_owned(),
        "/setupsnap folder - print the profile and ignore-list paths".to_owned(),
    ]
}

// ---------------------------------------------------------------------------
// Names, dates, sizes
// ---------------------------------------------------------------------------

pub fn sanitize_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut cleaned: String = trimmed
        .chars()
        .map(|character| match character {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            character if character.is_control() => '\0',
            character => character,
        })
        .filter(|character| *character != '\0')
        .collect();

    cleaned = cleaned.trim_end_matches('.').trim().to_owned();
    if cleaned.chars().count() > 48 {
        cleaned = cleaned.chars().take(48).collect::<String>().trim().to_owned();
    }

    if cleaned.is_empty() {
        format!("Setup {}", format_date(now_secs()))
    } else {
        cleaned
    }
}

fn unique_path(directory: &Path, name: &str) -> PathBuf {
    let mut candidate = directory.join(format!("{name}{PROFILE_EXTENSION}"));
    let mut counter = 2;
    while candidate.exists() && counter < 1000 {
        candidate = directory.join(format!("{name} ({counter}){PROFILE_EXTENSION}"));
        counter += 1;
    }
    candidate
}

fn strip_extension(file_name: &str) -> String {
    match file_name.rfind('.') {
        Some(index) if index > 0 => file_name[..index].to_owned(),
        _ => file_name.to_owned(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// UTC, so a profile written on one machine reads the same everywhere.
fn format_created_at(secs: i64) -> String {
    let (year, month, day, hour, minute, _second) = civil(secs);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn format_backup_name(secs: i64) -> String {
    let (year, month, day, hour, minute, second) = civil(secs);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}")
}

fn format_date(secs: i64) -> String {
    let (year, month, day, _, _, _) = civil(secs);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let of_day = secs.rem_euclid(86_400);
    let hour = (of_day / 3600) as u32;
    let minute = ((of_day % 3600) / 60) as u32;
    let second = (of_day % 60) as u32;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day, hour, minute, second)
}

// ---------------------------------------------------------------------------
// Zip entries
// ---------------------------------------------------------------------------

/// The names and bytes of a profile's entries: for tests and for diagnosing a
/// file a player reports.
pub fn read_profile_entries_for_test(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    Ok(read_zip(path)?
        .into_iter()
        .map(|entry| (entry.name, entry.data))
        .collect())
}

fn safe_relative(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return None;
    }

    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        path.push(part);
    }

    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn json_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

fn json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let mut characters = rest.char_indices();
    if characters.next()?.1 != '"' {
        return None;
    }

    let mut out = String::new();
    let mut escaped = false;
    for (_, character) in characters {
        if escaped {
            out.push(match character {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'b' => '\u{8}',
                'f' => '\u{c}',
                other => other,
            });
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }

    None
}

/// Counts the objects in a top-level array field, which is enough for the
/// "N files" line without a JSON dependency.
fn json_array_len_field(json: &str, key: &str) -> usize {
    let needle = format!("\"{key}\"");
    let Some(start) = json.find(&needle) else {
        return 0;
    };
    let rest = &json[start + needle.len()..];
    let Some(colon) = rest.find(':') else {
        return 0;
    };
    let rest = &rest[colon + 1..];
    let Some(open) = rest.find('[') else {
        return 0;
    };

    let bytes = rest.as_bytes();
    let mut index = open + 1;
    let mut depth = 0usize;
    let mut count = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' => {
                    if depth == 0 {
                        count += 1;
                    }
                    depth += 1;
                }
                b'}' => depth = depth.saturating_sub(1),
                b']' if depth == 0 => break,
                _ => {}
            }
        }
        index += 1;
    }

    count
}

// ---------------------------------------------------------------------------
// CRC-32
// ---------------------------------------------------------------------------

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Zip writer (stored entries: every zip reader, including the Java mod's,
// accepts them)
// ---------------------------------------------------------------------------

struct ZipWriter {
    file: fs::File,
    offset: u64,
    central: Vec<u8>,
    entries: u16,
}

impl ZipWriter {
    fn create(path: &Path) -> Result<ZipWriter, String> {
        let file = fs::File::create(path)
            .map_err(|e| format!("could not create {}: {e}", path.display()))?;
        Ok(ZipWriter {
            file,
            offset: 0,
            central: Vec::new(),
            entries: 0,
        })
    }

    fn add(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        let name = name.replace('\\', "/");
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(format!("entry name is too long: {name}"));
        }
        let size = u32::try_from(data.len()).map_err(|_| format!("{name} is too large"))?;
        let start = u32::try_from(self.offset).map_err(|_| "profile is over 4 GiB".to_owned())?;

        let crc = crc32(data);

        let mut header = Vec::with_capacity(30 + name_bytes.len());
        header.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&0x0800u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&33u16.to_le_bytes());
        header.extend_from_slice(&crc.to_le_bytes());
        header.extend_from_slice(&size.to_le_bytes());
        header.extend_from_slice(&size.to_le_bytes());
        header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(name_bytes);

        self.file
            .write_all(&header)
            .and_then(|_| self.file.write_all(data))
            .map_err(|e| format!("could not write {name}: {e}"))?;
        self.offset += header.len() as u64 + data.len() as u64;

        self.central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        self.central.extend_from_slice(&20u16.to_le_bytes());
        self.central.extend_from_slice(&20u16.to_le_bytes());
        self.central.extend_from_slice(&0x0800u16.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&33u16.to_le_bytes());
        self.central.extend_from_slice(&crc.to_le_bytes());
        self.central.extend_from_slice(&size.to_le_bytes());
        self.central.extend_from_slice(&size.to_le_bytes());
        self.central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&(0o100_644u32 << 16).to_le_bytes());
        self.central.extend_from_slice(&start.to_le_bytes());
        self.central.extend_from_slice(name_bytes);

        self.entries = self.entries.saturating_add(1);
        Ok(())
    }

    fn finish(mut self) -> Result<(), String> {
        let central_start = u32::try_from(self.offset).map_err(|_| "profile is over 4 GiB".to_owned())?;
        let central_size = u32::try_from(self.central.len()).map_err(|_| "central directory too large".to_owned())?;

        let mut end = Vec::with_capacity(22);
        end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        end.extend_from_slice(&self.entries.to_le_bytes());
        end.extend_from_slice(&self.entries.to_le_bytes());
        end.extend_from_slice(&central_size.to_le_bytes());
        end.extend_from_slice(&central_start.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());

        let central = std::mem::take(&mut self.central);
        self.file
            .write_all(&central)
            .and_then(|_| self.file.write_all(&end))
            .and_then(|_| self.file.flush())
            .map_err(|e| format!("could not finish the profile: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Zip reader (stored and deflate, which is what the Java mod writes)
// ---------------------------------------------------------------------------

struct ZipEntry {
    name: String,
    data: Vec<u8>,
}

fn read_zip(path: &Path) -> Result<Vec<ZipEntry>, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
    if metadata.len() > MAX_ARCHIVE_SIZE {
        return Err(format!("{} is larger than 256 MiB", path.display()));
    }
    let bytes = fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;

    let eocd = find_eocd(&bytes).ok_or_else(|| format!("{} is not a zip file", path.display()))?;
    let count = u16_at(&bytes, eocd + 10)? as usize;
    let directory_size = u32_at(&bytes, eocd + 12)? as usize;
    let directory_start = u32_at(&bytes, eocd + 16)? as usize;

    if count == 0xFFFF || directory_start == u32::MAX as usize || directory_size == u32::MAX as usize {
        return Err("zip64 profiles are not supported".to_owned());
    }
    if directory_start + directory_size > bytes.len() {
        return Err("the zip directory points past the end of the file".to_owned());
    }

    let mut entries = Vec::with_capacity(count);
    let mut cursor = directory_start;

    for _ in 0..count {
        if cursor + 46 > bytes.len() || u32_at(&bytes, cursor)? != 0x0201_4b50 {
            return Err("damaged zip directory".to_owned());
        }

        let method = u16_at(&bytes, cursor + 10)?;
        let crc = u32_at(&bytes, cursor + 16)?;
        let compressed_size = u32_at(&bytes, cursor + 20)? as usize;
        let size = u32_at(&bytes, cursor + 24)? as usize;
        let name_length = u16_at(&bytes, cursor + 28)? as usize;
        let extra_length = u16_at(&bytes, cursor + 30)? as usize;
        let comment_length = u16_at(&bytes, cursor + 32)? as usize;
        let local_offset = u32_at(&bytes, cursor + 42)? as usize;

        if size as u64 > MAX_FILE_SIZE {
            return Err(format!("an entry is larger than 16 MiB ({} bytes)", size));
        }

        let name_start = cursor + 46;
        let name_end = name_start + name_length;
        if name_end > bytes.len() {
            return Err("damaged zip directory".to_owned());
        }
        let name = String::from_utf8_lossy(&bytes[name_start..name_end]).into_owned();

        if local_offset + 30 > bytes.len() || u32_at(&bytes, local_offset)? != 0x0403_4b50 {
            return Err(format!("damaged zip entry for {name}"));
        }
        let local_name = u16_at(&bytes, local_offset + 26)? as usize;
        let local_extra = u16_at(&bytes, local_offset + 28)? as usize;
        let data_start = local_offset + 30 + local_name + local_extra;
        let data_end = data_start + compressed_size;
        if data_end > bytes.len() {
            return Err(format!("damaged zip entry for {name}"));
        }

        let raw = &bytes[data_start..data_end];
        let data = match method {
            0 => raw.to_vec(),
            8 => inflate(raw, size)?,
            other => return Err(format!("{name} uses unsupported compression method {other}")),
        };

        if data.len() != size {
            return Err(format!("{name} unpacked to {} bytes, expected {size}", data.len()));
        }
        if crc32(&data) != crc {
            return Err(format!("{name} failed its checksum"));
        }

        entries.push(ZipEntry { name, data });
        cursor = name_end + extra_length + comment_length;
    }

    Ok(entries)
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let start = bytes.len().saturating_sub(22 + 65_535);
    let mut index = bytes.len() - 22;
    loop {
        if u32::from_le_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]])
            == 0x0605_4b50
        {
            return Some(index);
        }
        if index == start {
            return None;
        }
        index -= 1;
    }
}

fn u16_at(bytes: &[u8], index: usize) -> Result<u16, String> {
    if index + 2 > bytes.len() {
        return Err("unexpected end of the zip file".to_owned());
    }
    Ok(u16::from_le_bytes([bytes[index], bytes[index + 1]]))
}

fn u32_at(bytes: &[u8], index: usize) -> Result<u32, String> {
    if index + 4 > bytes.len() {
        return Err("unexpected end of the zip file".to_owned());
    }
    Ok(u32::from_le_bytes([
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ]))
}

// ---------------------------------------------------------------------------
// DEFLATE (RFC 1951)
// ---------------------------------------------------------------------------

const MAX_BITS: usize = 15;

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

const DISTANCE_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    count: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Bits<'a> {
        Bits {
            data,
            pos: 0,
            bit: 0,
            count: 0,
        }
    }

    fn bits(&mut self, need: u32) -> Result<u32, String> {
        while self.count < need {
            if self.pos >= self.data.len() {
                return Err("the compressed data ends too early".to_owned());
            }
            self.bit |= (self.data[self.pos] as u32) << self.count;
            self.pos += 1;
            self.count += 8;
        }
        let value = self.bit & ((1 << need) - 1);
        self.bit >>= need;
        self.count -= need;
        Ok(value)
    }

    fn align(&mut self) {
        self.bit = 0;
        self.count = 0;
    }

    fn stored_bytes(&mut self, length: usize) -> Result<&'a [u8], String> {
        let start = self.pos;
        let end = start + length;
        if end > self.data.len() {
            return Err("the compressed data ends too early".to_owned());
        }
        self.pos = end;
        Ok(&self.data[start..end])
    }
}

struct Huffman {
    counts: [u16; MAX_BITS + 1],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut counts = [0u16; MAX_BITS + 1];
        for length in lengths {
            let index = *length as usize;
            if index > MAX_BITS {
                return Err("a Huffman code is longer than 15 bits".to_owned());
            }
            counts[index] += 1;
        }
        counts[0] = 0;

        let mut offsets = [0u16; MAX_BITS + 2];
        for length in 1..MAX_BITS {
            offsets[length + 1] = offsets[length] + counts[length];
        }

        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, length) in lengths.iter().enumerate() {
            let length = *length as usize;
            if length != 0 {
                symbols[offsets[length] as usize] = symbol as u16;
                offsets[length] += 1;
            }
        }

        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits<'_>) -> Result<u16, String> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;

        for length in 1..=MAX_BITS {
            code |= bits.bits(1)? as i32;
            let count = self.counts[length] as i32;
            if code - count < first {
                let position = index + (code - first);
                return self
                    .symbols
                    .get(position as usize)
                    .copied()
                    .ok_or_else(|| "a Huffman code has no symbol".to_owned());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }

        Err("a Huffman code is longer than 15 bits".to_owned())
    }
}

/// Raw DEFLATE (no zlib header), as a zip entry holds it.
pub fn inflate(input: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut bits = Bits::new(input);
    let mut out: Vec<u8> = Vec::with_capacity(limit.min(1 << 20));

    loop {
        let last = bits.bits(1)?;
        let kind = bits.bits(2)?;

        match kind {
            0 => {
                bits.align();
                let length = bits.bits(16)? as usize;
                let complement = bits.bits(16)? as usize;
                if length != (!complement & 0xFFFF) {
                    return Err("a stored block has a bad length".to_owned());
                }
                let data = bits.stored_bytes(length)?;
                out.extend_from_slice(data);
            }
            1 => {
                let literals = fixed_literals()?;
                let distances = fixed_distances()?;
                inflate_block(&mut bits, &mut out, &literals, &distances, limit)?;
            }
            2 => {
                let (literals, distances) = dynamic_tables(&mut bits)?;
                inflate_block(&mut bits, &mut out, &literals, &distances, limit)?;
            }
            _ => return Err("unsupported deflate block type".to_owned()),
        }

        if out.len() > limit {
            return Err("the compressed entry expands past its declared size".to_owned());
        }
        if last == 1 {
            break;
        }
    }

    Ok(out)
}

fn inflate_block(
    bits: &mut Bits<'_>,
    out: &mut Vec<u8>,
    literals: &Huffman,
    distances: &Huffman,
    limit: usize,
) -> Result<(), String> {
    loop {
        let symbol = literals.decode(bits)? as usize;

        if symbol < 256 {
            out.push(symbol as u8);
        } else if symbol == 256 {
            return Ok(());
        } else {
            let length_index = symbol - 257;
            if length_index >= LENGTH_BASE.len() {
                return Err("a length code is out of range".to_owned());
            }
            let length = LENGTH_BASE[length_index] as usize
                + bits.bits(LENGTH_EXTRA[length_index])? as usize;

            let distance_symbol = distances.decode(bits)? as usize;
            if distance_symbol >= DISTANCE_BASE.len() {
                return Err("a distance code is out of range".to_owned());
            }
            let distance = DISTANCE_BASE[distance_symbol] as usize
                + bits.bits(DISTANCE_EXTRA[distance_symbol])? as usize;

            if distance > out.len() {
                return Err("a distance points before the start of the entry".to_owned());
            }

            let start = out.len() - distance;
            for index in 0..length {
                let byte = out[start + index];
                out.push(byte);
            }
        }

        if out.len() > limit {
            return Err("the compressed entry expands past its declared size".to_owned());
        }
    }
}

fn fixed_literals() -> Result<Huffman, String> {
    let mut lengths = [0u8; 288];
    for (symbol, length) in lengths.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Huffman::new(&lengths)
}

fn fixed_distances() -> Result<Huffman, String> {
    Huffman::new(&[5u8; 32])
}

fn dynamic_tables(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), String> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    let literal_count = bits.bits(5)? as usize + 257;
    let distance_count = bits.bits(5)? as usize + 1;
    let code_count = bits.bits(4)? as usize + 4;

    let mut code_lengths = [0u8; 19];
    for item in ORDER.iter().take(code_count) {
        code_lengths[*item] = bits.bits(3)? as u8;
    }
    let code_table = Huffman::new(&code_lengths)?;

    let mut lengths = vec![0u8; literal_count + distance_count];
    let mut index = 0usize;
    while index < lengths.len() {
        let symbol = code_table.decode(bits)?;
        match symbol {
            0..=15 => {
                lengths[index] = symbol as u8;
                index += 1;
            }
            16 => {
                if index == 0 {
                    return Err("a repeated code length has nothing to repeat".to_owned());
                }
                let previous = lengths[index - 1];
                let repeats = 3 + bits.bits(2)? as usize;
                for _ in 0..repeats {
                    if index >= lengths.len() {
                        return Err("the code lengths run past their table".to_owned());
                    }
                    lengths[index] = previous;
                    index += 1;
                }
            }
            17 => {
                let repeats = 3 + bits.bits(3)? as usize;
                index += repeats;
            }
            18 => {
                let repeats = 11 + bits.bits(7)? as usize;
                index += repeats;
            }
            _ => return Err("a code length is out of range".to_owned()),
        }
    }

    if index > lengths.len() {
        return Err("the code lengths run past their table".to_owned());
    }

    let (literal_lengths, distance_lengths) = lengths.split_at(literal_count);
    Ok((
        Huffman::new(literal_lengths)?,
        Huffman::new(distance_lengths)?,
    ))
}

// ---------------------------------------------------------------------------
// Tests that need no fixture
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "setupsnap-{name}-{}-{}",
            std::process::id(),
            now_secs()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn glob_rules_follow_the_java_mod() {
        assert!(glob_match("**/*.log", "logs/latest.log"));
        assert!(glob_match("**/*.log", "foo.log"));
        assert!(!glob_match("**/*.log", "logs/latest.txt"));
        assert!(glob_match("**/cache/**", "config/x/cache/a.json"));
        assert!(glob_match("**/cache/**", "cache/a.json"));
        assert!(!glob_match("*/cache/**", "a/b/cache/x"));
        assert!(glob_match("**/.DS_Store", "config/.DS_Store"));
        assert!(glob_match("config/*.json", "config/a.json"));
        assert!(!glob_match("config/*.json", "config/nested/a.json"));
        assert!(glob_match("config/?.json", "config/a.json"));
    }

    #[test]
    fn json_fields_round_trip() {
        let manifest = "{\n  \"name\": \"Setup \\\"main\\\"\",\n  \"createdAt\": \"2026-11-08 10:00\",\n  \"files\": [{\"path\": \"a\"}, {\"path\": \"b\"}]\n}";
        assert_eq!(json_string_field(manifest, "name").unwrap(), "Setup \"main\"");
        assert_eq!(json_string_field(manifest, "createdAt").unwrap(), "2026-11-08 10:00");
        assert_eq!(json_array_len_field(manifest, "files"), 2);
        assert_eq!(json_quote("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(format_created_at(0), "1970-01-01 00:00");
        assert_eq!(format_backup_name(0), "1970-01-01_00-00-00");
        assert_eq!(format_created_at(1_735_689_600), "2025-01-01 00:00");
        assert_eq!(format_date(1_735_689_600), "2025-01-01");
    }

    #[test]
    fn names_are_sanitised() {
        assert_eq!(sanitize_name("  my/setup:1  "), "my_setup_1");
        assert!(sanitize_name("").starts_with("Setup "));
        assert_eq!(sanitize_name("x").chars().count(), 1);
    }

    #[test]
    fn stored_zip_round_trips_through_the_reader() {
        let dir = scratch("zip");
        let file = dir.join("a.snapsnap");
        let mut writer = ZipWriter::create(&file).unwrap();
        writer.add(MANIFEST_NAME, b"{\"name\":\"x\"}").unwrap();
        writer.add(OPTIONS_FILE, b"fov:0.5\n").unwrap();
        writer.add("config/empty.txt", b"").unwrap();
        writer.finish().unwrap();

        let entries = read_zip(&file).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, MANIFEST_NAME);
        assert_eq!(entries[1].data, b"fov:0.5\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tampered_entry_is_refused() {
        let dir = scratch("tamper");
        let file = dir.join("b.snapsnap");
        let mut writer = ZipWriter::create(&file).unwrap();
        writer.add(OPTIONS_FILE, b"fov:0.5\n").unwrap();
        writer.finish().unwrap();

        let mut bytes = fs::read(&file).unwrap();
        let index = bytes.windows(7).position(|w| w == b"fov:0.5").unwrap();
        bytes[index] = b'F';
        fs::write(&file, &bytes).unwrap();

        assert!(read_zip(&file).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsafe_entry_names_are_rejected() {
        assert!(safe_relative("../escape.txt").is_none());
        assert!(safe_relative("/abs.txt").is_none());
        assert!(safe_relative("a/../../b").is_none());
        assert!(safe_relative("config/a.json").is_some());
    }

    #[test]
    fn saving_then_applying_moves_a_setup_between_instances() {
        let source = scratch("source");
        fs::create_dir_all(source.join("config/nested")).unwrap();
        fs::write(source.join(OPTIONS_FILE), "fov:0.5\nkey_key.forward:87\n").unwrap();
        fs::write(source.join("config/nested/settings.json"), "{\"value\":1}\n").unwrap();
        fs::create_dir_all(source.join("config/setupsnap")).unwrap();
        fs::write(source.join("config/setupsnap/ignore.txt"), "# mine\n").unwrap();
        fs::write(source.join("config/empty.txt"), "").unwrap();

        let summary = save_profile(&source, "Shared", GAME_VERSION).unwrap();
        assert_eq!(summary.file_count, 3, "config/setupsnap must stay out of the profile");
        assert!(summary.path.exists());

        let target = scratch("target");
        fs::create_dir_all(target.join("config")).unwrap();
        fs::write(target.join(OPTIONS_FILE), "fov:0.25\n").unwrap();

        let profile_path = summary.path.to_str().unwrap().to_owned();
        let (_, applied) = apply_profile(&target, &profile_path).unwrap();
        assert_eq!(applied.changed, 3);
        assert_eq!(applied.skipped, 0);
        assert_eq!(
            fs::read_to_string(target.join(OPTIONS_FILE)).unwrap(),
            "fov:0.5\nkey_key.forward:87\n"
        );
        assert!(target.join("config/nested/settings.json").is_file());
        assert!(applied.backup_dir.is_some(), "replaced files are backed up");

        // The second apply changes nothing.
        let (_, again) = apply_profile(&target, &profile_path).unwrap();
        assert_eq!(again.changed, 0);
        assert_eq!(again.unchanged, 3);

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&target);
    }
}
