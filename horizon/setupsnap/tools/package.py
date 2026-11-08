#!/usr/bin/env python3
"""Package the SetupSnap Horizon bundle as a source-only `.hnmod`.

The archive holds `horizon-native.json` and `mixins/setupsnap/` (only `.rs` and
`.toml`), with fixed dates and modes, so the same sources always produce the
same bytes. Horizon's launcher reads it, then compiles it on the pinned commit
when the player installs it.

Usage: python3 tools/package.py
"""
import hashlib
import json
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = "26.3"


def package():
    compat = json.loads((ROOT / "compatibility.json").read_text())
    manifests = sorted((ROOT / "mixins").glob("*/horizon-mixin.toml"))
    if len(manifests) != 1:
        raise SystemExit("a .hnmod holds one bundle: keep one mixins/<id>/horizon-mixin.toml")

    bundle = manifests[0].parent
    mixin = tomllib.loads(manifests[0].read_text())
    mod = mixin["mod"]

    if bundle.name != mod["id"]:
        raise SystemExit(f"rename mixins/{bundle.name}/ to mixins/{mod['id']}/ (the launcher finds it by id)")
    if mixin.get("compatibility", {}).get("horizon_revision") != compat["horizon_revision"]:
        raise SystemExit("horizon-mixin.toml and compatibility.json pin different Horizon revisions")

    manifest = {
        "schema": 1,
        "id": mod["id"],
        "version": mod["version"],
        "game": compat.get("game", GAME),
        "horizon": compat["horizon_revision"],
        "mixins": compat["horizon_mixins_revision"],
        "hooks": compat["hooks"],
    }

    files = {"horizon-native.json": (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()}
    for path in sorted(bundle.iterdir()):
        if path.is_file() and path.suffix in (".rs", ".toml"):
            files[path.relative_to(ROOT).as_posix()] = path.read_bytes()

    out = ROOT / "target" / f"{mod['id']}-{mod['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)

    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)

    print(json.dumps({
        "file": str(out),
        "size": out.stat().st_size,
        "sha256": hashlib.sha256(out.read_bytes()).hexdigest(),
        "entries": sorted(files),
    }, indent=2))


if __name__ == "__main__":
    package()
