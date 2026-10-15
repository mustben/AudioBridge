"""Export actual Cargo dependency licenses; does not download or execute dependency code."""
import json
import shutil
import subprocess
from pathlib import Path


def validate_license_expression(expression: str) -> None:
    supported = {"MIT OR Apache-2.0", "(MIT OR Apache-2.0) AND Unicode-3.0"}
    if expression not in supported:
        raise RuntimeError(f"Manual license review needed: {expression}")


def validate_license_inventory(existing: set[str], expected: set[str]) -> None:
    stale = existing - expected
    if stale:
        raise RuntimeError(f"Stale license directories need manual review: {sorted(stale)}")


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")],
        check=True, capture_output=True, text=True,
    )
    metadata = json.loads(result.stdout)
    packages = [p for p in metadata["packages"] if p["source"] is not None]
    expected = {f'{p["name"]}-{p["version"]}' for p in packages}
    license_root = root / "THIRD_PARTY_LICENSES"
    existing = {p.name for p in license_root.iterdir() if p.is_dir()} if license_root.exists() else set()
    validate_license_inventory(existing, expected)
    rows = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["source"] is None:
            continue
        source = Path(package["manifest_path"]).parent
        target = root / "THIRD_PARTY_LICENSES" / f'{package["name"]}-{package["version"]}'
        names = ("LICENSE-MIT", "license-mit", "LICENSE-UNICODE")
        licenses = [source / name for name in names if (source / name).is_file()]
        if not licenses:
            raise RuntimeError(f'No supported license files found for {package["name"]}')
        declared = package.get("license") or ""
        validate_license_expression(declared)
        if "Unicode" in declared and not any(p.name == "LICENSE-UNICODE" for p in licenses):
            raise RuntimeError("Missing additional Unicode license")
        target.mkdir(parents=True, exist_ok=True)
        for license_path in licenses:
            shutil.copyfile(license_path, target / license_path.name)
        rows.append(f'| {package["name"]} | {package["version"]} | {declared} |')
    text = (
        "# Third-party notices\n\n"
        "Generated from the checked-in Cargo.lock using `python scripts/export_licenses.py`.\n"
        "License texts are copied from the installed Cargo packages, not invented.\n"
        "For MIT OR Apache-2.0 dependencies this distribution selects the MIT option;\n"
        "unicode-ident also carries its mandatory Unicode license. Some entries are\n"
        "build-time or alternative-target dependencies rather than linked x64 runtime code.\n\n"
        "| Package | Version | Declared license |\n| --- | --- | --- |\n"
        + "\n".join(rows)
        + "\n\nFull texts: `THIRD_PARTY_LICENSES/`. The application and original icon are MIT licensed.\n"
    )
    (root / "THIRD_PARTY_NOTICES.md").write_text(text, encoding="utf-8")
    print(f"Exported actual license texts for {len(rows)} dependency packages")


if __name__ == "__main__":
    main()
