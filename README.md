# Packdesk

A friendly, Git-native desktop workspace for **packwiz** Minecraft modpacks.

**Early preview, not a stable release.** This is a renamed fork of
[Packweave](https://github.com/packweavers/packweave), being adapted to use
packwiz as its pack-management backend. The Linux distribution format is an
AppImage, with a pinned packwiz binary bundled inside.

## Connected in the native packwiz preview

- Packweave's familiar content browser, file editor, Git changes, diffs,
  commits, history, and branches.
- Open and edit native `pack.toml`, `index.toml`, and `.pw.toml` metadata without
  converting packs to Packweave's format.
- The bundled packwiz handles installing exact provider projects/versions,
  removing mods, refreshing metadata, and Modrinth/CurseForge exports.
- Selected Git commits rebuild manifest/index hashes from the staged files in
  an isolated temporary tree. Unselected working changes are not smuggled into
  the commit or its hashes.
- Metadata snapshots support undo/redo; opening a pack does not convert or
  silently repair it.
- Ordinary packwiz and Git CLI workflows remain usable alongside the app.

## Not connected yet

This is **not full Packweave feature parity**. Native instance sync, self-updating
exports, one-click publishing, alternate-provider editing, disabling entries,
and dependency-role/bundling overrides are not implemented. These actions
report an explicit error rather than quietly using an incompatible format.
Provider dependency validation is still upstream work, not a packwiz health
guarantee. Archive imports still use the inherited storage path; open native
packwiz packs directly instead of importing/converting them.

Native packwiz Modrinth export supports the client pack; use CurseForge's
server-side export when a server-only target is required. Always back up or
commit an existing pack before testing this preview.

## Development

The application uses the upstream Svelte/TypeScript frontend and Rust/Tauri
backend. It requires Node.js 24+, pnpm, Rust, Go, and Tauri's Linux build
dependencies (GTK 3, WebKitGTK 4.1, librsvg, and AppIndicator).

```sh
pnpm install --frozen-lockfile
./bin/prepare-packwiz
pnpm tauri dev
```

`PACKDESK_PACKWIZ=/absolute/path/to/packwiz` selects a development CLI. The
packaged AppImage automatically finds its bundled CLI.

```sh
./bin/test
./bin/appimage
```

AppImages are written to `src-tauri/target/release/bundle/appimage/`.
Pass `--debug` to `bin/appimage` for a development build under `target/debug/`.
`PACKWIZ_SOURCE=/path/to/packwiz` can select an existing checkout when building;
otherwise `bin/prepare-packwiz` fetches the exact revision in
[packwiz-revision](packwiz-revision). GitHub Actions' **Linux AppImage** workflow
also builds a downloadable artifact; tags beginning with `v` publish the AppImage
as a GitHub release asset.

## Headless interface

```sh
packdesk check /path/to/pack.toml --json
packdesk run --pack /path/to/pack.toml --json -- list
packdesk skill install --json
```

These commands do not start a GUI. Results go to stdout, failures to stderr;
exit status is 0 or 1. For an AppImage, use its path instead of `packdesk`.

## License and attribution

GPL-3.0-only; see [LICENSE](LICENSE). Copyright notices and upstream history are
retained. This software comes without warranty.

Packdesk is modified from Packweave, initially based on upstream commit
`c21d4527074ac6126db10c9a2211d3e8291c0dcd`, on November 23, 2026.
It is independently maintained and is not endorsed by Packweave or packwiz.
The Packweave name, logo, and brand assets are not used to identify this fork;
see upstream's [branding policy](COPYING.md).
The separate packwiz binary is MIT-licensed; see [PACKWIZ-LICENSE](PACKWIZ-LICENSE)
and its [pinned source](https://github.com/packwiz/packwiz/tree/170b22d785959dda5876b54b7f16e1b807a57fb6).
