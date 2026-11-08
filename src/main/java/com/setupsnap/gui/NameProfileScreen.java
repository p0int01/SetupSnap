package com.setupsnap.gui;

import com.setupsnap.ClientAdapters;
import com.setupsnap.SetupSnap;
import com.setupsnap.profile.ProfileInfo;
import com.setupsnap.profile.ProfileStore;
import java.io.IOException;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** Asks for a name and saves the current instance's setup as a new profile. */
public class NameProfileScreen extends Screen {
	private final ProfilesScreen parent;
	private EditBox nameBox;
	private Button createButton;
	private StringWidget errorWidget;

	public NameProfileScreen(ProfilesScreen parent) {
		super(Component.translatable("setupsnap.gui.save_title"));
		this.parent = parent;
	}

	@Override
	protected void init() {
		int contentWidth = Math.max(180, Math.min(this.width - 40, 280));
		int left = (this.width - contentWidth) / 2;
		int labelY = this.height / 2 - 44;

		this.centeredLabel(this.title, labelY - 20);
		this.addRenderableOnly(new StringWidget(left, labelY, contentWidth, 12,
				Component.translatable("setupsnap.gui.name_label"), this.font));

		this.nameBox = new EditBox(this.font, left, labelY + 14, contentWidth, 20,
				Component.translatable("setupsnap.gui.name_hint"));
		this.nameBox.setMaxLength(48);
		this.nameBox.setValue(ProfileStore.defaultProfileName());
		this.nameBox.setResponder(value -> {
			if (this.createButton != null) {
				this.createButton.active = !value.isBlank();
			}
		});
		this.addRenderableWidget(this.nameBox);
		this.nameBox.setFocused(true);

		int buttonWidth = Math.max(70, Math.min(100, (contentWidth - 8) / 2));
		this.createButton = this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.create"),
						button -> this.create())
				.bounds(this.width / 2 - buttonWidth - 4, labelY + 44, buttonWidth, 20)
				.build());

		this.addRenderableWidget(Button.builder(Component.translatable("gui.cancel"), button -> this.back())
				.bounds(this.width / 2 + 4, labelY + 44, buttonWidth, 20)
				.build());

		this.errorWidget = this.addRenderableOnly(new StringWidget(left, labelY + 72, contentWidth, 12,
				Component.empty(), this.font));
		this.errorWidget.setMaxWidth(contentWidth);

		this.centeredLabel(Component.translatable("setupsnap.gui.save_hint"), labelY + 88);
	}

	private void create() {
		try {
			ProfileInfo info = ProfileStore.save(this.nameBox.getValue());
			SetupSnap.LOGGER.info("SetupSnap saved {} with {} files", info.fileName(), info.fileCount());
			this.back();
		} catch (IOException e) {
			SetupSnap.LOGGER.error("Could not save a setup profile", e);
			this.errorWidget.setMessage(Component.translatable("setupsnap.gui.save_error", String.valueOf(e.getMessage())));
		}
	}

	private void centeredLabel(Component text, int y) {
		int width = this.font.width(text);
		this.addRenderableOnly(new StringWidget((this.width - width) / 2, y, width, 12, text, this.font));
	}

	private void back() {
		ClientAdapters.openScreen(this.parent.copy());
	}

	@Override
	public void onClose() {
		this.back();
	}
}
