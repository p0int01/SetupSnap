package com.setupsnap.gui;

import com.setupsnap.ClientAdapters;
import com.setupsnap.SetupSnap;
import com.setupsnap.profile.ApplyResult;
import com.setupsnap.profile.ProfileInfo;
import com.setupsnap.profile.ProfileStore;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.screens.ConfirmScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/**
 * Lists saved setups and applies, creates or deletes them.
 *
 * <p>Only vanilla widgets are used for layout: newer Minecraft versions replaced the immediate-mode
 * drawing used by older screens, while Buttons and StringWidgets keep working across all of them.
 */
public class ProfilesScreen extends Screen {
	private static final int PAGE_SIZE = 6;
	private static final int ROW_HEIGHT = 24;

	private final Screen parent;
	private final List<ProfileInfo> profiles = new ArrayList<>();
	private int page;
	private Component status;
	private StringWidget statusWidget;
	private String loadError;

	public ProfilesScreen(Screen parent) {
		this(parent, 0, Component.empty());
	}

	private ProfilesScreen(Screen parent, int page, Component status) {
		super(Component.translatable("setupsnap.gui.title"));
		this.parent = parent;
		this.page = Math.max(0, page);
		this.status = status;
	}

	/**
	 * A fresh copy of this screen.
	 *
	 * <p>Opening the same screen instance again would re-run {@code init()} and append a second set of
	 * widgets on top of the old ones, so every return to this list goes through here instead.
	 */
	ProfilesScreen copy() {
		return new ProfilesScreen(this.parent, this.page, this.status);
	}

	@Override
	protected void init() {
		this.reload();

		int contentWidth = Math.max(180, Math.min(this.width - 40, 340));
		int left = (this.width - contentWidth) / 2;
		int listTop = 42;
		int buttonRow = this.height - 46;
		int statusRow = buttonRow - 16;
		int rows = this.rows(listTop, statusRow);
		int pages = this.pageCount(rows);

		this.centeredLabel(this.title, 16);
		this.centeredLabel(Component.translatable("setupsnap.gui.subtitle", this.profiles.size()), 29);

		if (this.loadError != null) {
			this.centeredLabel(Component.translatable("setupsnap.gui.read_error", this.loadError), listTop);
		} else if (this.profiles.isEmpty()) {
			this.centeredLabel(Component.translatable("setupsnap.gui.empty"), listTop);
		}

		for (int row = 0; row < rows; row++) {
			int index = this.page * rows + row;
			if (index >= this.profiles.size()) {
				break;
			}

			ProfileInfo info = this.profiles.get(index);
			int y = listTop + row * ROW_HEIGHT;

			this.addRenderableWidget(Button.builder(this.rowLabel(info), button -> this.confirmApply(info))
					.bounds(left, y, contentWidth - 56, 20)
					.build());

			this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.delete"),
							button -> this.confirmDelete(info))
					.bounds(left + contentWidth - 52, y, 52, 20)
					.build());
		}

		this.statusWidget = this.addRenderableOnly(new StringWidget(left, statusRow, contentWidth, 12,
				this.status, this.font));
		this.statusWidget.setMaxWidth(contentWidth);

		int buttonWidth = Math.max(56, (contentWidth - 8) / 3);
		this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.save"),
						button -> ClientAdapters.openScreen(new NameProfileScreen(this)))
				.bounds(left, buttonRow, buttonWidth, 20)
				.build());

		this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.folder"),
						button -> ProfileStore.openProfilesFolder())
				.bounds(left + buttonWidth + 4, buttonRow, buttonWidth, 20)
				.build());

		this.addRenderableWidget(Button.builder(Component.translatable("gui.done"), button -> this.onClose())
				.bounds(left + (buttonWidth + 4) * 2, buttonRow, buttonWidth, 20)
				.build());

		if (pages > 1) {
			int pagerRow = this.height - 22;
			this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.previous"),
							button -> this.changePage(-1, rows))
					.bounds(left, pagerRow, 40, 20)
					.build());

			this.addRenderableWidget(Button.builder(Component.translatable("setupsnap.gui.next"),
							button -> this.changePage(1, rows))
					.bounds(left + contentWidth - 40, pagerRow, 40, 20)
					.build());

			this.centeredLabel(Component.translatable("setupsnap.gui.page", this.page + 1, pages), pagerRow + 6);
		}
	}

	private int rows(int listTop, int statusRow) {
		return Math.max(1, Math.min(PAGE_SIZE, (statusRow - 8 - listTop) / ROW_HEIGHT));
	}

	private int pageCount(int rows) {
		return Math.max(1, (this.profiles.size() + rows - 1) / rows);
	}

	private void changePage(int delta, int rows) {
		int pages = this.pageCount(rows);
		this.page = Math.max(0, Math.min(pages - 1, this.page + delta));
		ClientAdapters.openScreen(this.copy());
	}

	private Component rowLabel(ProfileInfo info) {
		return Component.literal(info.name()).append(" ")
				.append(Component.translatable("setupsnap.gui.row_short", info.fileCount(), info.sizeLabel()));
	}

	private void reload() {
		this.profiles.clear();
		this.loadError = null;

		try {
			this.profiles.addAll(ProfileStore.list());
		} catch (IOException e) {
			this.loadError = String.valueOf(e.getMessage());
			SetupSnap.LOGGER.warn("Could not list setup profiles", e);
		}

		int rows = this.rows();
		int pages = this.pageCount(rows);
		this.page = Math.max(0, Math.min(this.page, pages - 1));
	}

	private int rows() {
		return this.rows(42, this.height - 62);
	}

	private void centeredLabel(Component text, int y) {
		int width = this.font.width(text);
		this.addRenderableOnly(new StringWidget((this.width - width) / 2, y, width, 12, text, this.font));
	}

	private void showStatus(Component message) {
		this.status = message;
		if (this.statusWidget != null) {
			this.statusWidget.setMessage(message);
		}
	}

	private void confirmApply(ProfileInfo info) {
		ClientAdapters.openScreen(new ConfirmScreen(confirmed -> {
			if (!confirmed) {
				ClientAdapters.openScreen(this.copy());
				return;
			}

			try {
				ApplyResult result = ProfileStore.apply(info);
				this.showStatus(Component.translatable("setupsnap.gui.status_applied", info.name(),
						result.changed(), result.unchanged()));

				if (result.modConfigsChanged() || (result.optionsApplied() && !result.optionsReloaded())) {
					SetupSnap.LOGGER.info("SetupSnap applied {}; a restart is recommended for mod configs", info.name());
				}
			} catch (IOException e) {
				SetupSnap.LOGGER.error("Could not apply profile {}", info.file(), e);
				this.showStatus(Component.translatable("setupsnap.gui.status_failed", String.valueOf(e.getMessage())));
			}

			ClientAdapters.openScreen(this.copy());
		}, Component.translatable("setupsnap.gui.apply_title"),
				Component.translatable("setupsnap.gui.apply_confirm", info.name(), info.fileCount(),
						info.sizeLabel(), info.createdAt())));
	}

	private void confirmDelete(ProfileInfo info) {
		ClientAdapters.openScreen(new ConfirmScreen(confirmed -> {
			if (confirmed) {
				try {
					ProfileStore.delete(info);
					this.showStatus(Component.translatable("setupsnap.message.deleted", info.name()));
				} catch (IOException e) {
					SetupSnap.LOGGER.error("Could not delete profile {}", info.file(), e);
					this.showStatus(Component.translatable("setupsnap.gui.status_failed", String.valueOf(e.getMessage())));
				}
			}

			ClientAdapters.openScreen(this.copy());
		}, Component.translatable("setupsnap.gui.delete_title"),
				Component.translatable("setupsnap.gui.delete_confirm", info.name())));
	}

	@Override
	public void onClose() {
		ClientAdapters.openScreen(this.parent);
	}
}
