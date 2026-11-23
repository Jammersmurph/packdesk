---
name: packdesk
description: Inspect and manage native packwiz Minecraft modpacks without a display.
---

Packdesk is a GPLv3 desktop client for packwiz with a headless interface.
Replace `packdesk` with the AppImage path when using an AppImage.

- `packdesk check /absolute/path/to/pack.toml --json` reads native metadata.
  It does not change files or contact mod providers.
- `packdesk run --pack /absolute/path/to/pack.toml --json -- list`
  invokes the bundled packwiz. Any packwiz command and arguments can follow `--`.
  Prompts use packwiz's noninteractive defaults.
- `packdesk skill install --json` installs this skill.

Commands return 0 on success, 1 on failure. JSON results go to stdout; JSON
errors go to stderr. Ask before updates, removals, or exports that overwrite
files. Commit or back up existing packs before changing them.
