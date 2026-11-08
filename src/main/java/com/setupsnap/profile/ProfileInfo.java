package com.setupsnap.profile;

import java.nio.file.Path;
import java.util.Locale;

/** A profile file found on disk, with the details shown in the list. */
public record ProfileInfo(Path file, String name, int fileCount, long totalBytes, String createdAt, String minecraftVersion) {
	public String fileName() {
		return this.file.getFileName().toString();
	}

	public String sizeLabel() {
		if (this.totalBytes < 1024L) {
			return this.totalBytes + " B";
		}

		double kilobytes = this.totalBytes / 1024.0;
		if (kilobytes < 1024.0) {
			return String.format(Locale.ROOT, "%.1f kB", kilobytes);
		}

		return String.format(Locale.ROOT, "%.1f MB", kilobytes / 1024.0);
	}
}
