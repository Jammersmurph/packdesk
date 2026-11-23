# Packdesk

A friendly, Git-native desktop workspace for **packwiz** Minecraft modpacks.

**Work in progress.** This is a renamed fork of
[Packweave](https://github.com/packweavers/packweave), being adapted to use
packwiz as its pack-management backend. It is not yet a finished or tested release.
The target Linux distribution format is an AppImage.

## Direction

- Keep Packweave's familiar content browser, file editor, Git changes, diffs,
  commits, history, and branches.
- Open and edit native `pack.toml`, `index.toml`, and `.pw.toml` metadata without
  converting packs to Packweave's format.
- Let the bundled packwiz handle installing, updating, removing, and exporting.
- Keep ordinary packwiz and Git CLI workflows fully usable alongside the app.

## Development

The application uses the upstream Svelte/TypeScript frontend and Rust/Tauri
backend. It requires Node.js 24+, pnpm, Rust, Go, and Tauri's Linux build
dependencies (GTK 3, WebKitGTK 4.1, librsvg, and AppIndicator).

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Build scripts and AppImage packaging are being adapted to bundle packwiz.
Do not use this development version on the only copy of a pack.

## License and attribution

GPL-3.0-only; see [LICENSE](LICENSE). Copyright notices and upstream history are
retained. This software comes without warranty.

Packdesk is modified from Packweave, initially based on upstream commit
`c21d4527074ac6126db10c9a2211d3e8291c0dcd`, on November 23, 2026.
It is independently maintained and is not endorsed by Packweave or packwiz.
The Packweave name, logo, and brand assets are not used to identify this fork;
see upstream's [branding policy](COPYING.md).
