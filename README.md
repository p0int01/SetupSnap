# SetupSnap

A small **client-side Fabric mod** that saves your *whole setup* into one portable file and puts it
back on any other instance — so switching launchers, making a new instance, or moving to another PC no
longer means redoing every keybind, slider and mod config by hand.

## The problem

Every launcher profile and every new instance starts with a fresh `.minecraft`: default keybinds,
default sensitivity, default video settings, and no mod configs. Rebuilding that by hand is slow and
easy to get subtly wrong (a slightly different FOV or a crosshair offset changes how the game feels).

## Supported versions

| Minecraft | Fabric Loader | Fabric API | Java | Adapter |
| --- | --- | --- | --- | --- |
| 1.21.11 | 0.20.1 | 0.141.6+1.21.11 | 21+ | `src/mc121` |
| 26.1.2 | 0.20.1 | 0.155.3+26.1.2 | 25+ | `src/mc261` |
| 26.2 | 0.20.1 | 0.161.0+26.2 | 25+ | `src/mc262` |
| 26.3 | 0.20.1 | 0.165.2+26.3 | 25+ | `src/mc262` |

Each version gets its own jar, built from the same shared sources. There is no 26.1.3 — the 26.1
series ends at 26.1.2, so that is the version covered.

## What a setup file contains

A profile is a single `.snapsnap` file (a normal zip) containing:

| Item | Why |
| --- | --- |
| `options.txt` | Keybinds, mouse sensitivity, FOV, video/graphics settings, sound sliders, GUI scale, chat and accessibility options, language |
| `config/**` | Every mod config file: crosshair addons, HUD mods, Sodium/Iris options, keybind managers, shader settings, … |
| `iris.properties`, `optionsof.txt`, `optionsshaders.txt`, `sodium-options.json` | Settings some mods keep next to `options.txt` instead of in `config/` |
| `manifest.json` | Name, creation time, game/mod version, and the list of files with their sizes |

Mod configs are picked up automatically — anything a mod writes under `config/` is included, so custom
crosshairs, HUD layouts, Zoom settings, chat tweaks and similar come along without per-mod support.

Profiles are not tied to a Minecraft version: a setup saved on 1.21.11 can be applied on 26.3, since
`options.txt` and config files are plain files. Settings for mods that are not installed are ignored.

## Using it

**In game**

* Press **Right Shift** (rebindable in Options → Controls → Miscellaneous → *Open SetupSnap*).
* Or run `/setupsnap` in any world or on the title screen.

The screen lists your saved setups, one button per setup, with its file count and size:

* **Click a setup** – asks for confirmation, then applies it (with a backup).
* **Delete** (next to each setup) – removes that profile file only.
* **< / >** – pages through the list when there are more setups than fit.
* **Save setup** – names and captures the current instance.
* **Open folder** – opens `config/setupsnap/profiles` in your file manager.

**Commands**

```
/setupsnap                      open the screen
/setupsnap list                 list saved setups with size and date
/setupsnap save <name>          save the current setup
/setupsnap apply <name>         apply a setup (also accepts a full path to a .snapsnap file)
/setupsnap delete <name>        delete a setup
/setupsnap folder               open the profiles folder
```

## Moving a setup to another instance

1. On the old instance: save a setup, then click **Open folder**.
2. Copy the `.snapsnap` file (or the whole `profiles` folder) to the new instance's
   `config/setupsnap/profiles/`.
3. Start the new instance with SetupSnap installed, open the screen, click the setup, confirm, then
   restart the game.

You can also skip step 2 by pointing `apply` at the file wherever it is:
`/setupsnap apply /home/me/Downloads/My Setup.snapsnap`.

## Other launchers (Horizon, Prism, MultiMC, …)

SetupSnap is a normal Fabric mod, so any launcher that runs a **Minecraft: Java Edition instance with
Fabric** can use it: install Fabric API and the jar for that game version into the instance's `mods/`
folder. The mod only asks Fabric for its paths (`FabricLoader.getGameDir()` / `getConfigDir()`), so
per-instance folders need no configuration and profiles move between instances as plain files.

**Horizon** ([playhorizon.dev](https://playhorizon.dev)) is two different things, and only one of them
can run this mod:

* **Horizon launcher with a Fabric instance** – create the instance and pick Fabric as its loader, then
  add Fabric API and the matching SetupSnap jar. This is the same path as every other launcher and
  needs nothing extra. Horizon runs each version with the Java release that version's JSON asks for
  (21 for 1.21.11, 25 for 26.x), which is exactly what the jars require.
* **Horizon's native client** – Minecraft: Java Edition 26.3 rewritten in Rust. It has no JVM, so Fabric
  jars such as this one cannot load there. It runs native Rust mods instead, and this repository ships
  one that reads and writes the same `.snapsnap` files: see [Horizon native mod](#horizon-native-mod).

## Horizon native mod

`horizon/setupsnap/` is a native Horizon mod (`.hnmod`) that keeps the same commands, the same profile
folder and the same file format as the Fabric mod, so a profile saved on one applies on the other. It is
written against Horizon's own hook points rather than against Java classes:

| | Hook | API |
|---|---|---|
| Registers the `setupsnap` name so chat colours and completes it | `commands` | 35 |
| Runs `/setupsnap ...` and answers in chat | `command` | 35 |
| Finds `config/` (its parent is the instance folder) | `native_config_dir()` | 26 |

`horizon/setupsnap/mixins/setupsnap/runtime.rs` holds the whole implementation and is injected into the
game as `crate::game::setupsnap`. It depends on nothing but the standard library — Horizon only lets a
native mod use crates already in its `Cargo.lock`, so the zip reader, the DEFLATE decoder and CRC-32 are
written in the file itself. It is the same code a plain `cargo test` compiles, which is how its safety
rules and the real Java-made profiles in `tests/fixtures/` are checked here:

```sh
cd horizon/setupsnap
cargo test                 # runtime tests against real deflate fixtures
python3 tools/package.py   # -> target/setupsnap-1.0.0.hnmod
```

Players build it with their own launcher, which pins Rust and compiles the client:

```sh
horizon-launcher native-build horizon/setupsnap --trust-native-code
```

Native Horizon mods run unsandboxed, so that trust flag is the player's decision, never an agent's. The
pins in `horizon/setupsnap/compatibility.json` must match the Nightly the launcher plays. What is not
verified here is the compile into a real Horizon client: that needs the pinned checkout and a ~2 GB build,
and it is the step that proves the two hook adapters match that commit's signatures.

## Safety

* **Nothing is deleted.** Applying only writes the files the profile contains; files that exist only in
  the target instance are left alone.
* **Backups first.** Every file that gets replaced is copied to
  `config/setupsnap/backups/<date-time>/<same relative path>` before being overwritten.
* **Unsafe entries are skipped.** Profile entries that would resolve outside the game folder are ignored.
* **Secrets stay home.** `config/setupsnap/ignore.txt` lists glob patterns that are never saved
  (`*.log`, caches, `session.lock`, …). Add patterns there for configs that hold tokens or passwords
  before sharing a profile with anyone.

## What applies instantly and what needs a restart

* `options.txt` is reloaded into the running client right away, so keybinds, sensitivity, FOV, video and
  sound settings change immediately. Options that rebuild render targets or reload resource packs still
  look best after a restart.
* Mod config files **are written instantly but almost every mod reads them only at startup**, so restart
  the game after applying a profile that contains mod configs. SetupSnap says so in the screen.
* Mods that keep settings somewhere else entirely (for example inside `saves/`, a world folder, or a
  database) are not captured by this version.

## Building

The build needs **JDK 25**, because Loom 1.19 runs on it (26.x also targets Java 25). JDK 21 can run the
1.21.11 jar but cannot run the build.

```bash
# once, if you do not already have a JDK 25 (any distribution works):
mkdir -p .jdk
curl -L -o /tmp/jdk25.tar.gz "https://api.adoptium.net/v3/binary/latest/25/ga/linux/x64/jdk/hotspot/normal/eclipse"
tar -xzf /tmp/jdk25.tar.gz -C .jdk

JAVA_HOME="$PWD/.jdk/jdk-25.0.5+8" ./gradlew build
```

Jars are written per version:

```
versions/mc-1.21.11/build/libs/setupsnap-1.0.0+1.21.11.jar
versions/mc-26.1.2/build/libs/setupsnap-1.0.0+26.1.2.jar
versions/mc-26.2/build/libs/setupsnap-1.0.0+26.2.jar
versions/mc-26.3/build/libs/setupsnap-1.0.0+26.3.jar
```

Build a single version with `./gradlew :versions:mc-26.3:build`.

## Project layout

```
src/main/java/com/setupsnap/     shared sources compiled for every version
  SetupSnap.java                 constants, folders, version helpers
  SetupSnapClient.java           client entrypoint, keybind, command registration
  command/SetupSnapCommands      /setupsnap ... built through CommandBuilders
  gui/ProfilesScreen             setup list, apply/delete confirmation, paging
  gui/NameProfileScreen          naming a new profile
  profile/ProfileStore           save / list / apply / backups / ignore rules
  profile/SnapshotManifest       manifest.json model
src/mc121/java/com/setupsnap/    adapter for 1.21.11
  ClientAdapters                 KeyBindingHelper, ClientCommandManager, Minecraft#setScreen
src/mc261/java/com/setupsnap/    adapter for 26.1.x
  ClientAdapters                 KeyMappingHelper, ClientCommands, Minecraft#setScreen
src/mc262/java/com/setupsnap/    adapter for 26.2 and 26.3
  ClientAdapters                 KeyMappingHelper, ClientCommands, Gui#setScreen
src/main/resources/              fabric.mod.json, language, icon
versions/mc-<game>/              one module per Minecraft version
```

### Why the adapters exist

1.21.11 is the last obfuscated release, so it needs remapping and the older Fabric API names
(`fabric-key-binding-api-v1`, `ClientCommandManager`). 26.x ships unobfuscated, renamed the key
mapping module (`fabric-key-mapping-api-v1`) and moved the command helpers to `ClientCommands`, and it
replaced immediate-mode screen drawing (`GuiGraphics`) with a render-state extractor. 26.2 then moved
screen management from `Minecraft` to `Gui`, and 26.3 renamed the keyboard input type (`KEYSYM` to
`KEYBOARD`, which the adapters avoid entirely by using `KeyMapping`'s integer constructor).

The screens therefore use only vanilla widgets (`Button`, `StringWidget`, `EditBox`), which work on both
render pipelines, and each version difference is confined to the ~40-line adapter in `src/mc121`,
`src/mc261` or `src/mc262`.

## License

MIT.
