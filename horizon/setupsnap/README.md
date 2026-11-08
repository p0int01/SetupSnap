# SetupSnap for Horizon

A native Horizon mod with the same commands, the same folder and the same
`.snapsnap` file as the Fabric mod in the repository root, so a profile saved in
one applies in the other.

| | Fabric mod | Horizon native mod |
|---|---|---|
| Install | jar in `mods/` | `.hnmod` built by the launcher |
| Commands | `/setupsnap save\|apply\|list\|folder` | same |
| Profiles | `config/setupsnap/profiles/*.snapsnap` | same |
| Ignore list | `config/setupsnap/ignore.txt` | same |
| Backups | `config/setupsnap/backups/<date>/` | same |
| Format | zip: `manifest.json`, `options.txt`, `config/**` | same |

Horizon keeps its settings in `options.txt` in the instance folder and mod
configs in `config/`, the same layout the Fabric mod reads, which is why the file
format carries over unchanged.

## Building it

The mod is source: Horizon compiles it into the game.

```sh
# What players do: the launcher pins Rust, compiles the client and links the mod.
horizon-launcher native-build horizon/setupsnap --trust-native-code

# Or make the installable file and hand it to the launcher's Mods page.
python3 horizon/setupsnap/tools/package.py
# -> horizon/setupsnap/target/setupsnap-1.0.0.hnmod
```

`--trust-native-code` is the player's decision: native mods run unsandboxed, with
the user's own rights. Nothing in this repository asks an agent to accept it.

The pins live in `compatibility.json` (currently Horizon Nightly commit
`2d22e6a`). Every mod in one build must pin the same Horizon commit, so if the
launcher plays a different Nightly, update `horizon_revision` there and in
`mixins/setupsnap/horizon-mixin.toml`; `horizon-launcher native-build` reports a
mismatch instead of guessing.

## What it adds to the game

| Hook | API | What it does |
|---|---|---|
| `commands` | 35 | registers the `setupsnap` name, so chat colours and completes it |
| `command` | 35 | runs `/setupsnap ...`, answers in chat, and keeps the line from the server |
| `native_config_dir()` | 26 | finds `config/`, whose parent is the instance folder |
| `native_chat()` | — | one line of feedback per result |

`runtime.rs` is the whole implementation and is injected as
`crate::game::setupsnap`. It uses nothing but the standard library, because a
Horizon native mod may only depend on crates already in Horizon's `Cargo.lock`:
the zip reader, the DEFLATE decoder (stored, fixed-Huffman and dynamic-Huffman
blocks) and CRC-32 are all in the file.

Profiles it writes are stored (uncompressed) zip entries, which every reader
accepts, including the Java mod. Profiles it reads may use either stored or
deflate entries, which is what `java.util.zip.ZipOutputStream` writes.

## Safety

The same rules as the Fabric mod:

* Nothing is deleted. Applying writes only the entries a profile holds.
* Every replaced file is copied to `config/setupsnap/backups/<timestamp>/` first.
* `../`, absolute paths, `C:` paths and anything under `config/setupsnap/` are
  skipped, so a profile cannot escape the instance or overwrite your profiles.
* Entries over 16 MiB, or an archive over 256 MiB, are refused.
* Files already identical are left untouched and counted as unchanged.
* Saving skips `config/setupsnap/` itself, the `ignore.txt` globs, and files over
  16 MiB.

Horizon reads `options.txt` and `config/` when it starts, so **restart the client
after applying**; the Fabric mod reloads live instead.

## Tests

```sh
cd horizon/setupsnap
cargo test
```

`tests/fixtures/fabric-made.snapsnap` and `hostile.snapsnap` are real zip files
written by Python's zlib, standing in for what the Fabric mod produces: reading
the large entry exercises the dynamic-Huffman inflate, and the hostile one checks
the traversal and own-folder guards.

What the tests do not cover: compiling this bundle into an actual Horizon client.
That needs a Horizon checkout with the pinned commit, the launcher's Rust
toolchain and the game's ~2 GB build, and it is the step that proves the two hook
adapters (`commands.rs`, `command.rs`) match the pinned commit's signatures.
