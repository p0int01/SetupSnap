package com.setupsnap.command;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.setupsnap.ClientAdapters;
import com.setupsnap.SetupSnap;
import com.setupsnap.gui.ProfilesScreen;
import com.setupsnap.profile.ApplyResult;
import com.setupsnap.profile.ProfileInfo;
import com.setupsnap.profile.ProfileStore;
import java.io.IOException;
import java.util.List;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.network.chat.Component;

/** Client side {@code /setupsnap} command, handy for quick saves and for using profiles in any world. */
public final class SetupSnapCommands {
	private SetupSnapCommands() {
	}

	public static void register(CommandDispatcher<FabricClientCommandSource> dispatcher, CommandBuilders builders) {
		dispatcher.register(builders.literal("setupsnap")
				.executes(context -> openGui())
				.then(builders.literal("gui").executes(context -> openGui()))
				.then(builders.literal("list").executes(SetupSnapCommands::list))
				.then(builders.literal("folder").executes(context -> {
					ProfileStore.openProfilesFolder();
					return 1;
				}))
				.then(builders.literal("save").then(builders.argument("name", StringArgumentType.greedyString())
						.executes(SetupSnapCommands::save)))
				.then(builders.literal("apply").then(builders.argument("name", StringArgumentType.greedyString())
						.executes(SetupSnapCommands::apply)))
				.then(builders.literal("delete").then(builders.argument("name", StringArgumentType.greedyString())
						.executes(SetupSnapCommands::delete))));
	}

	private static int openGui() {
		ClientAdapters.openScreen(new ProfilesScreen(null));
		return 1;
	}

	private static int list(CommandContext<FabricClientCommandSource> context) {
		try {
			List<ProfileInfo> profiles = ProfileStore.list();
			if (profiles.isEmpty()) {
				context.getSource().sendFeedback(Component.translatable("setupsnap.message.no_profiles"));
				return 0;
			}

			context.getSource().sendFeedback(Component.translatable("setupsnap.message.list_header", profiles.size()));
			for (ProfileInfo info : profiles) {
				context.getSource().sendFeedback(Component.literal(" - " + info.name() + " ")
						.append(Component.translatable("setupsnap.gui.row_short", info.fileCount(), info.sizeLabel())));
			}

			return profiles.size();
		} catch (IOException e) {
			SetupSnap.LOGGER.error("Could not list the setup profiles", e);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.list_failed",
					String.valueOf(e.getMessage())));
			return 0;
		}
	}

	private static int save(CommandContext<FabricClientCommandSource> context) {
		String name = StringArgumentType.getString(context, "name");

		try {
			ProfileInfo info = ProfileStore.save(name);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.saved", info.name(),
					info.fileCount(), info.fileName()));
			return 1;
		} catch (IOException e) {
			SetupSnap.LOGGER.error("Could not save a setup profile", e);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.save_failed",
					String.valueOf(e.getMessage())));
			return 0;
		}
	}

	private static int apply(CommandContext<FabricClientCommandSource> context) {
		String name = StringArgumentType.getString(context, "name");

		try {
			ProfileInfo info = ProfileStore.find(name).orElse(null);
			if (info == null) {
				context.getSource().sendFeedback(Component.translatable("setupsnap.message.profile_not_found", name));
				return 0;
			}

			ApplyResult result = ProfileStore.apply(info);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.applied", info.name(),
					result.changed(), result.unchanged(), result.backupDir() == null ? "-" : result.backupDir().toString()));

			if (result.modConfigsChanged() || (result.optionsApplied() && !result.optionsReloaded())) {
				context.getSource().sendFeedback(Component.translatable("setupsnap.message.restart"));
			}

			return 1;
		} catch (IOException e) {
			SetupSnap.LOGGER.error("Could not apply a setup profile", e);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.apply_failed",
					String.valueOf(e.getMessage())));
			return 0;
		}
	}

	private static int delete(CommandContext<FabricClientCommandSource> context) {
		String name = StringArgumentType.getString(context, "name");

		try {
			ProfileInfo info = ProfileStore.find(name).orElse(null);
			if (info == null) {
				context.getSource().sendFeedback(Component.translatable("setupsnap.message.profile_not_found", name));
				return 0;
			}

			ProfileStore.delete(info);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.deleted", info.name()));
			return 1;
		} catch (IOException e) {
			SetupSnap.LOGGER.error("Could not delete a setup profile", e);
			context.getSource().sendFeedback(Component.translatable("setupsnap.message.delete_failed",
					String.valueOf(e.getMessage())));
			return 0;
		}
	}
}
