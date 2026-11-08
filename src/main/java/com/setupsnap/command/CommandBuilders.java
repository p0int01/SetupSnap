package com.setupsnap.command;

import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;

/**
 * The two Brigadier builders differ between Fabric API generations:
 * 1.21.11 exposes {@code ClientCommandManager}, 26.x exposes {@code ClientCommands}.
 * A version adapter supplies this interface so the command tree itself stays shared.
 */
public interface CommandBuilders {
	LiteralArgumentBuilder<FabricClientCommandSource> literal(String name);

	<T> RequiredArgumentBuilder<FabricClientCommandSource, T> argument(String name, ArgumentType<T> type);
}
