package com.setupsnap.profile;

import java.nio.file.Path;

/**
 * Outcome of applying a profile.
 *
 * @param changed           files that were created or overwritten
 * @param unchanged         files that already matched the profile
 * @param skipped           unsafe or unreadable entries that were ignored
 * @param backupDir         where previous versions were copied, or {@code null} when nothing was replaced
 * @param optionsApplied    whether options.txt was part of the profile
 * @param optionsReloaded   whether the new options.txt was applied to the running client
 * @param modConfigsChanged whether any mod config file changed (those usually need a restart)
 */
public record ApplyResult(int changed, int unchanged, int skipped, Path backupDir, boolean optionsApplied,
		boolean optionsReloaded, boolean modConfigsChanged) {
}
