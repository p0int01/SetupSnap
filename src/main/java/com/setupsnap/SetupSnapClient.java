package com.setupsnap;

import com.setupsnap.command.SetupSnapCommands;
import com.setupsnap.gui.ProfilesScreen;
import com.setupsnap.profile.IgnoreRules;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.KeyMapping;

public class SetupSnapClient implements ClientModInitializer {
	private KeyMapping openProfilesKey;

	@Override
	public void onInitializeClient() {
		// Creates the default ignore.txt so players can see and edit what gets skipped.
		IgnoreRules.load();

		this.openProfilesKey = ClientAdapters.registerKeyMapping(ClientAdapters.createOpenKeyMapping());

		ClientTickEvents.END_CLIENT_TICK.register(client -> {
			while (this.openProfilesKey.consumeClick()) {
				if (ClientAdapters.currentScreen() == null) {
					ClientAdapters.openScreen(new ProfilesScreen(null));
				}
			}
		});

		ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
				SetupSnapCommands.register(dispatcher, ClientAdapters.commandBuilders()));

		SetupSnap.LOGGER.info("SetupSnap ready: profiles live in {}", SetupSnap.profilesDir());
	}
}
