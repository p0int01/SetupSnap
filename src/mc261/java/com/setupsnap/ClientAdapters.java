package com.setupsnap;

import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.setupsnap.command.CommandBuilders;
import net.fabricmc.fabric.api.client.command.v2.ClientCommands;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;

/**
 * Fabric adapters for 26.1.x: renamed key mapping and command helpers, but screens and key types are
 * still the ones used by 1.21.11.
 */
public final class ClientAdapters {
	/** GLFW key code for right shift; stable across every supported version. */
	private static final int RIGHT_SHIFT_KEY = 344;

	private ClientAdapters() {
	}

	public static KeyMapping createOpenKeyMapping() {
		return new KeyMapping("key.setupsnap.open", RIGHT_SHIFT_KEY, KeyMapping.Category.MISC);
	}

	public static KeyMapping registerKeyMapping(KeyMapping mapping) {
		return KeyMappingHelper.registerKeyMapping(mapping);
	}

	public static void openScreen(Screen screen) {
		Minecraft.getInstance().setScreen(screen);
	}

	public static Screen currentScreen() {
		return Minecraft.getInstance().screen;
	}

	public static CommandBuilders commandBuilders() {
		return new CommandBuilders() {
			@Override
			public LiteralArgumentBuilder<FabricClientCommandSource> literal(String name) {
				return ClientCommands.literal(name);
			}

			@Override
			public <T> RequiredArgumentBuilder<FabricClientCommandSource, T> argument(String name, ArgumentType<T> type) {
				return ClientCommands.argument(name, type);
			}
		};
	}
}
