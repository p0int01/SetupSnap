package com.setupsnap.profile;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.setupsnap.SetupSnap;
import java.io.IOException;
import java.awt.Desktop;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;

/**
 * Creates, lists and applies setup profiles.
 *
 * <p>A profile is a plain zip file holding {@code options.txt} (keybinds, sensitivity, video, sound and
 * accessibility settings) plus every file from the {@code config} folder, so mod configs such as a custom
 * crosshair travel with it. Nothing is ever deleted from the target instance: applying only writes the files
 * that the profile contains, and copies over replaced files into {@code config/setupsnap/backups} first.
 */
public final class ProfileStore {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static final String MANIFEST_NAME = "manifest.json";
	private static final String OPTIONS_FILE = "options.txt";
	private static final long MAX_FILE_SIZE = 16L * 1024L * 1024L;

	private static final DateTimeFormatter DATE_FORMAT = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm", Locale.ROOT)
			.withZone(ZoneId.systemDefault());
	private static final DateTimeFormatter BACKUP_FORMAT = DateTimeFormatter.ofPattern("yyyy-MM-dd_HH-mm-ss", Locale.ROOT)
			.withZone(ZoneId.systemDefault());

	/** Settings files that some mods keep next to options.txt instead of inside config/. */
	private static final List<String> EXTRA_ROOT_FILES = List.of(
			"iris.properties",
			"optionsof.txt",
			"optionsshaders.txt",
			"sodium-options.json");

	private ProfileStore() {
	}

	public static List<ProfileInfo> list() throws IOException {
		Path profiles = SetupSnap.profilesDir();
		Files.createDirectories(profiles);

		try (Stream<Path> stream = Files.list(profiles)) {
			return stream.filter(Files::isRegularFile)
					.filter(ProfileStore::looksLikeProfile)
					.map(ProfileStore::describe)
					.sorted(Comparator.comparing(ProfileInfo::name, String.CASE_INSENSITIVE_ORDER))
					.toList();
		}
	}

	public static Optional<ProfileInfo> find(String query) throws IOException {
		String trimmed = query == null ? "" : query.trim();
		if (trimmed.isEmpty()) {
			return Optional.empty();
		}

		try {
			Path asPath = Path.of(trimmed);
			if (Files.isRegularFile(asPath)) {
				return Optional.of(describe(asPath));
			}
		} catch (InvalidPathException ignored) {
			// Not a path, fall through to a name lookup.
		}

		for (ProfileInfo info : list()) {
			if (info.name().equalsIgnoreCase(trimmed)
					|| info.fileName().equalsIgnoreCase(trimmed)
					|| stripExtension(info.fileName()).equalsIgnoreCase(trimmed)) {
				return Optional.of(info);
			}
		}

		return Optional.empty();
	}

	public static ProfileInfo save(String rawName) throws IOException {
		String name = sanitizeName(rawName);
		Path profiles = SetupSnap.profilesDir();
		Files.createDirectories(profiles);
		Path target = uniquePath(profiles, name);

		IgnoreRules rules = IgnoreRules.load();
		List<Path> files = collectFiles(rules);

		SnapshotManifest manifest = new SnapshotManifest();
		manifest.name = name;
		manifest.modVersion = SetupSnap.modVersion();
		manifest.minecraftVersion = SetupSnap.minecraftVersion();
		manifest.createdAt = DATE_FORMAT.format(Instant.now());
		manifest.sourceGameDir = SetupSnap.gameDir().toAbsolutePath().toString();

		long totalBytes = 0L;
		for (Path file : files) {
			long size = Files.size(file);
			totalBytes += size;
			manifest.files.add(new SnapshotManifest.FileEntry(relativePath(file), size));
		}

		Path temporary = target.resolveSibling(target.getFileName() + ".tmp");
		try (OutputStream output = Files.newOutputStream(temporary);
				ZipOutputStream zip = new ZipOutputStream(output, StandardCharsets.UTF_8)) {
			zip.putNextEntry(new ZipEntry(MANIFEST_NAME));
			zip.write(GSON.toJson(manifest).getBytes(StandardCharsets.UTF_8));
			zip.closeEntry();

			for (Path file : files) {
				zip.putNextEntry(new ZipEntry(relativePath(file)));
				Files.copy(file, zip);
				zip.closeEntry();
			}
		}

		Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
		SetupSnap.LOGGER.info("Saved setup profile {} with {} files", target, files.size());
		return new ProfileInfo(target, name, files.size(), totalBytes, manifest.createdAt, manifest.minecraftVersion);
	}

	public static ApplyResult apply(ProfileInfo profile) throws IOException {
		Path gameDir = SetupSnap.gameDir().toAbsolutePath().normalize();
		Path ownDir = SetupSnap.rootDir().toAbsolutePath().normalize();
		Path backupRoot = SetupSnap.backupsDir().resolve(BACKUP_FORMAT.format(Instant.now()));
		Path backupDir = null;

		int changed = 0;
		int unchanged = 0;
		int skipped = 0;
		boolean optionsApplied = false;
		boolean modConfigsChanged = false;

		try (ZipFile zip = new ZipFile(profile.file().toFile(), StandardCharsets.UTF_8)) {
			var entries = zip.entries();
			while (entries.hasMoreElements()) {
				ZipEntry entry = entries.nextElement();
				if (entry.isDirectory()) {
					continue;
				}

				String entryName = entry.getName().replace('\\', '/');
				if (entryName.equals(MANIFEST_NAME)) {
					continue;
				}

				Path destination = gameDir.resolve(entryName).normalize();
				if (!destination.startsWith(gameDir) || destination.equals(gameDir)) {
					SetupSnap.LOGGER.warn("Skipping unsafe profile entry {}", entryName);
					skipped++;
					continue;
				}

				// A profile must never be able to rewrite SetupSnap's own profiles, backups or ignore list.
				if (destination.startsWith(ownDir)) {
					SetupSnap.LOGGER.warn("Skipping profile entry {} because it belongs to SetupSnap itself", entryName);
					skipped++;
					continue;
				}

				if (entry.getSize() > MAX_FILE_SIZE) {
					SetupSnap.LOGGER.warn("Skipping {} because it is larger than 16 MiB", entryName);
					skipped++;
					continue;
				}

				byte[] content;
				try (InputStream input = zip.getInputStream(entry)) {
					content = input.readAllBytes();
				}

				if (content.length > MAX_FILE_SIZE) {
					SetupSnap.LOGGER.warn("Skipping {} because it is larger than 16 MiB", entryName);
					skipped++;
					continue;
				}

				if (Files.isRegularFile(destination) && Arrays.equals(Files.readAllBytes(destination), content)) {
					unchanged++;
					continue;
				}

				if (Files.isRegularFile(destination)) {
					Path backup = backupRoot.resolve(entryName).normalize();
					if (!backup.startsWith(backupRoot.normalize())) {
						SetupSnap.LOGGER.warn("Skipping {} because it cannot be backed up first", entryName);
						skipped++;
						continue;
					}

					try {
						Files.createDirectories(backup.getParent());
						Files.copy(destination, backup, StandardCopyOption.REPLACE_EXISTING);
					} catch (IOException e) {
						// Never overwrite a file we could not back up.
						SetupSnap.LOGGER.warn("Skipping {} because it could not be backed up", entryName, e);
						skipped++;
						continue;
					}

					backupDir = backupRoot;
				}

				// One unwritable file (read-only, or a folder sitting where the profile has a
				// file) must not abandon the rest of the profile halfway through.
				try {
					Path parent = destination.getParent();
					if (parent != null) {
						Files.createDirectories(parent);
					}

					Files.write(destination, content);
				} catch (IOException e) {
					SetupSnap.LOGGER.warn("Skipping {} because it could not be written", entryName, e);
					skipped++;
					continue;
				}

				changed++;

				if (entryName.equals(OPTIONS_FILE)) {
					optionsApplied = true;
				} else {
					modConfigsChanged = true;
				}
			}
		}

		boolean optionsReloaded = optionsApplied && reloadClientOptions();
		SetupSnap.LOGGER.info("Applied profile {}: {} changed, {} unchanged, {} skipped",
				profile.file(), changed, unchanged, skipped);

		return new ApplyResult(changed, unchanged, skipped, backupDir, optionsApplied, optionsReloaded, modConfigsChanged);
	}

	public static void delete(ProfileInfo profile) throws IOException {
		Files.deleteIfExists(profile.file());
	}

	public static void openProfilesFolder() {
		Path folder = SetupSnap.profilesDir();
		try {
			Files.createDirectories(folder);

			// Minecraft moved its own folder-opening helper between releases, and desktop
			// integration is the one API that behaves the same on every supported version.
			if (!Desktop.isDesktopSupported() || !Desktop.getDesktop().isSupported(Desktop.Action.OPEN)) {
				SetupSnap.LOGGER.warn("Opening folders is not supported here; the profile folder is {}", folder);
				return;
			}

			Desktop.getDesktop().open(folder.toFile());
		} catch (Exception e) {
			SetupSnap.LOGGER.warn("Could not open the profile folder {}; open it manually", folder, e);
		}
	}

	public static String defaultProfileName() {
		return "Setup " + LocalDate.now();
	}

	public static String sanitizeName(String rawName) {
		String trimmed = rawName == null ? "" : rawName.trim();
		if (trimmed.isEmpty()) {
			trimmed = "Setup";
		}

		String cleaned = trimmed.replaceAll("[\\\\/:*?\"<>|]", "_").replaceAll("\\p{Cntrl}", "");
		cleaned = cleaned.replaceAll("\\.+$", "").trim();
		if (cleaned.length() > 48) {
			cleaned = cleaned.substring(0, 48).trim();
		}

		return cleaned.isEmpty() ? "Setup" : cleaned;
	}

	private static boolean looksLikeProfile(Path file) {
		String name = file.getFileName().toString().toLowerCase(Locale.ROOT);
		return name.endsWith(SetupSnap.PROFILE_EXTENSION) || name.endsWith(".zip");
	}

	private static ProfileInfo describe(Path file) {
		SnapshotManifest manifest = null;
		try (ZipFile zip = new ZipFile(file.toFile(), StandardCharsets.UTF_8)) {
			ZipEntry entry = zip.getEntry(MANIFEST_NAME);
			if (entry != null) {
				try (InputStream input = zip.getInputStream(entry)) {
					manifest = GSON.fromJson(new String(input.readAllBytes(), StandardCharsets.UTF_8), SnapshotManifest.class);
				}
			}
		} catch (Exception e) {
			SetupSnap.LOGGER.warn("Could not read the manifest of {}", file, e);
		}

		if (manifest == null || manifest.files == null) {
			try {
				String createdAt = DATE_FORMAT.format(Files.getLastModifiedTime(file).toInstant());
				return new ProfileInfo(file, stripExtension(file.getFileName().toString()), -1, Files.size(file), createdAt, "?");
			} catch (IOException e) {
				return new ProfileInfo(file, stripExtension(file.getFileName().toString()), -1, 0L, "?", "?");
			}
		}

		long totalBytes = 0L;
		int fileCount = 0;
		for (SnapshotManifest.FileEntry entry : manifest.files) {
			if (entry != null) {
				totalBytes += entry.size;
				fileCount++;
			}
		}

		String name = manifest.name == null || manifest.name.isBlank()
				? stripExtension(file.getFileName().toString())
				: manifest.name;

		return new ProfileInfo(file, name, fileCount, totalBytes,
				manifest.createdAt == null ? "?" : manifest.createdAt,
				manifest.minecraftVersion == null ? "?" : manifest.minecraftVersion);
	}

	private static List<Path> collectFiles(IgnoreRules rules) throws IOException {
		Path gameDir = SetupSnap.gameDir().toAbsolutePath().normalize();
		Path configDir = FabricLoader.getInstance().getConfigDir().toAbsolutePath().normalize();
		Path ownRoot = SetupSnap.rootDir().toAbsolutePath().normalize();

		List<Path> files = new ArrayList<>();

		Path options = gameDir.resolve(OPTIONS_FILE);
		if (Files.isRegularFile(options)) {
			files.add(options);
		}

		for (String extra : EXTRA_ROOT_FILES) {
			Path candidate = gameDir.resolve(extra);
			if (Files.isRegularFile(candidate)) {
				files.add(candidate);
			}
		}

		if (Files.isDirectory(configDir)) {
			try (Stream<Path> stream = Files.walk(configDir)) {
				stream.filter(Files::isRegularFile).forEach(file -> {
					Path absolute = file.toAbsolutePath().normalize();
					if (absolute.startsWith(ownRoot)) {
						return;
					}

					if (rules.ignores(relativePath(file))) {
						return;
					}

					try {
						if (Files.size(file) > MAX_FILE_SIZE) {
							SetupSnap.LOGGER.info("Skipping {} because it is larger than 16 MiB", relativePath(file));
							return;
						}
					} catch (IOException e) {
						return;
					}

					files.add(file);
				});
			}
		}

		return files;
	}

	private static String relativePath(Path file) {
		Path gameDir = SetupSnap.gameDir().toAbsolutePath().normalize();
		Path absolute = file.toAbsolutePath().normalize();
		if (absolute.startsWith(gameDir)) {
			return gameDir.relativize(absolute).toString().replace('\\', '/');
		}

		Path configDir = FabricLoader.getInstance().getConfigDir().toAbsolutePath().normalize();
		if (absolute.startsWith(configDir)) {
			return "config/" + configDir.relativize(absolute).toString().replace('\\', '/');
		}

		return file.getFileName().toString();
	}

	private static Path uniquePath(Path directory, String name) {
		Path candidate = directory.resolve(name + SetupSnap.PROFILE_EXTENSION);
		int counter = 2;
		while (Files.exists(candidate) && counter < 1000) {
			candidate = directory.resolve(name + " (" + counter + ")" + SetupSnap.PROFILE_EXTENSION);
			counter++;
		}

		return candidate;
	}

	private static String stripExtension(String fileName) {
		int dot = fileName.lastIndexOf('.');
		return dot > 0 ? fileName.substring(0, dot) : fileName;
	}

	/** Re-reads options.txt into the running client so keybinds and video settings apply without a restart. */
	private static boolean reloadClientOptions() {
		try {
			Minecraft minecraft = Minecraft.getInstance();
			minecraft.options.load();
			KeyMapping.resetMapping();
			return true;
		} catch (Exception e) {
			SetupSnap.LOGGER.warn("Could not reload options.txt at runtime", e);
			return false;
		}
	}
}
