package com.setupsnap;

import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/** Shared ids, folders and version helpers used by the rest of the mod. */
public final class SetupSnap {
	public static final String MOD_ID = "setupsnap";
	public static final String PROFILE_EXTENSION = ".snapsnap";
	public static final Logger LOGGER = LoggerFactory.getLogger("SetupSnap");

	private SetupSnap() {
	}

	/** The instance folder (".minecraft") that holds options.txt. */
	public static Path gameDir() {
		return FabricLoader.getInstance().getGameDir();
	}

	/** SetupSnap's own folder, kept inside the config folder so it travels with the instance. */
	public static Path rootDir() {
		return FabricLoader.getInstance().getConfigDir().resolve(MOD_ID);
	}

	public static Path profilesDir() {
		return rootDir().resolve("profiles");
	}

	public static Path backupsDir() {
		return rootDir().resolve("backups");
	}

	public static Path ignoreFile() {
		return rootDir().resolve("ignore.txt");
	}

	public static String minecraftVersion() {
		return FabricLoader.getInstance().getModContainer("minecraft")
				.map(container -> container.getMetadata().getVersion().getFriendlyString())
				.orElse("unknown");
	}

	public static String modVersion() {
		return FabricLoader.getInstance().getModContainer(MOD_ID)
				.map(container -> container.getMetadata().getVersion().getFriendlyString())
				.orElse("dev");
	}
}
