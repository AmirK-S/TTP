#!/usr/bin/env python3
"""Print the GitHub release notes for one TTP version, in Markdown.

The text is the app's own What's New note (`whatsNew.notes.<x-y-z>` in
src/i18n/locales/en.json), so a release page says exactly what users read in
the app, and there is one place to write it. Exits 1 when the version has no
note, so the caller can keep the default body.

    python3 scripts/release_notes.py 3.2.3 > notes.md
"""

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO = "https://github.com/AmirK-S/TTP"


def notes_for(version: str) -> str | None:
    locale = json.loads((ROOT / "src/i18n/locales/en.json").read_text(encoding="utf-8"))
    note = locale.get("whatsNew", {}).get("notes", {}).get(version.replace(".", "-"))
    if not note:
        return None

    lines = [line.strip() for line in note.split("\n") if line.strip()]
    headline, bullets = lines[0], lines[1:]
    out = [f"**{headline}**", ""]
    out += [f"- {b.lstrip('•').strip()}" for b in bullets]
    out += [
        "",
        "## Download",
        "",
        "| Platform | File |",
        "|---|---|",
        f"| macOS, Apple Silicon | [TTP-macOS-arm64.dmg]({REPO}/releases/download/v{version}/TTP-macOS-arm64.dmg) |",
        f"| macOS, Intel | [TTP.by.AmirKS_{version}_x64.dmg]({REPO}/releases/download/v{version}/TTP.by.AmirKS_{version}_x64.dmg) |",
        f"| Windows | [TTP-Windows-x64-setup.exe]({REPO}/releases/download/v{version}/TTP-Windows-x64-setup.exe) |",
        "",
        "Already installed? TTP updates itself; nothing to do.",
        "",
        "Setup takes a free [Groq API key](https://console.groq.com). More at [ttp.amirks.eu](https://ttp.amirks.eu).",
    ]
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: release_notes.py <version>")
    body = notes_for(sys.argv[1].lstrip("v"))
    if body is None:
        sys.exit(1)
    sys.stdout.write(body)
