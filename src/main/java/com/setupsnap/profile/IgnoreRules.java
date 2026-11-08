package com.setupsnap.profile;

import com.setupsnap.SetupSnap;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/**
 * Glob patterns that keep machine specific or secret files out of profiles.
 *
 * <p>The list lives in {@code config/setupsnap/ignore.txt} so players can extend it without a config screen.
 */
public final class IgnoreRules {
	private static final List<String> DEFAULT_LINES = List.of(
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
			"**/.DS_Store",
			"**/Thumbs.db");

	private final List<Pattern> patterns;

	private IgnoreRules(List<Pattern> patterns) {
		this.patterns = patterns;
	}

	public static IgnoreRules load() {
		Path file = SetupSnap.ignoreFile();

		try {
			Files.createDirectories(file.getParent());
			if (!Files.isRegularFile(file)) {
				Files.writeString(file, String.join("\n", DEFAULT_LINES) + "\n", StandardCharsets.UTF_8);
			}

			List<Pattern> patterns = new ArrayList<>();
			for (String line : Files.readAllLines(file, StandardCharsets.UTF_8)) {
				String trimmed = line.trim();
				if (trimmed.isEmpty() || trimmed.startsWith("#")) {
					continue;
				}

				try {
					patterns.add(globToPattern(trimmed.replace('\\', '/')));
				} catch (RuntimeException e) {
					SetupSnap.LOGGER.warn("Ignoring invalid pattern in ignore.txt: {}", trimmed, e);
				}
			}

			return new IgnoreRules(patterns);
		} catch (IOException e) {
			SetupSnap.LOGGER.warn("Could not read config/setupsnap/ignore.txt", e);
			return new IgnoreRules(List.of());
		}
	}

	public boolean ignores(String relativePath) {
		for (Pattern pattern : this.patterns) {
			if (pattern.matcher(relativePath).matches()) {
				return true;
			}
		}

		return false;
	}

	static Pattern globToPattern(String glob) {
		StringBuilder regex = new StringBuilder("^");
		int index = 0;

		while (index < glob.length()) {
			char character = glob.charAt(index);

			if (character == '*') {
				if (index + 1 < glob.length() && glob.charAt(index + 1) == '*') {
					index += 2;
					if (index < glob.length() && glob.charAt(index) == '/') {
						regex.append("(?:.*/)?");
						index++;
					} else {
						regex.append(".*");
					}
				} else {
					regex.append("[^/]*");
					index++;
				}
			} else if (character == '?') {
				regex.append("[^/]");
				index++;
			} else {
				regex.append(Pattern.quote(String.valueOf(character)));
				index++;
			}
		}

		return Pattern.compile(regex.append('$').toString());
	}
}
