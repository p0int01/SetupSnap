package com.setupsnap.profile;

import java.util.ArrayList;
import java.util.List;

/** Contents of manifest.json inside every profile file. */
public final class SnapshotManifest {
	public static final int CURRENT_FORMAT = 1;

	public int format = CURRENT_FORMAT;
	public String modVersion;
	public String minecraftVersion;
	public String name;
	public String createdAt;
	public String sourceGameDir;
	public List<FileEntry> files = new ArrayList<>();

	public static final class FileEntry {
		public String path;
		public long size;

		public FileEntry() {
		}

		public FileEntry(String path, long size) {
			this.path = path;
			this.size = size;
		}
	}
}
