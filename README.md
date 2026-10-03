# Game Library

An open-source, Linux-first, cross-platform game library. It brings every game you own on
Steam, Epic, GOG, itch.io and manual installs into one place.

- **Linux first.** Built and tested on Linux before anything else, with Windows and macOS
  close behind. Native Steam, Flatpak Steam and Proton/Wine prefixes are first-class.
- **Customization without code.** Playnite-level control over how your library looks, but
  every game page is a drag-and-drop layout of widgets. You never write a theme or a script.
- **Ownership-aware deal finder.** Deals for games you don't own yet, and none for the ones
  you already do on any store.

Status: early development. Nothing is stable yet.

## Roadmap

| Milestone | What you get | Status |
| --- | --- | --- |
| 0. Scaffold | Tauri 2 app, SQLite library, Linux CI | Done |
| 1. Steam import | Installed Steam games from every library folder (native, Flatpak, Snap), launch via Steam | Done |
| 2. Steam ownership | Uninstalled games you own, via the Steam Web API with your own key | Planned |
| 3. More stores | GOG (via Heroic/GOG Galaxy data), Epic (via Heroic/Legendary), itch.io app | Planned |
| 4. Manual games | Add any executable, Wine/Proton prefix or emulator ROM by hand | Planned |
| 5. Metadata | Covers, descriptions, genres and playtime from store APIs and IGDB/SteamGridDB | In progress (Steam store and IGDB done) |
| 6. Widget layouts | Drag-and-drop game page editor: cover, description, playtime, screenshots, links, notes and more | Planned |
| 7. Deal finder | Price tracking across stores that skips anything you already own, with wishlist alerts | Planned |
| 8. Polish | Collections, filters, controller navigation, Windows and macOS packages | Planned |

## Development

You need Node 22+, a stable Rust toolchain and Tauri's
[Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux). On Debian or Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev
```

Then:

```sh
npm install
npm run tauri dev       # run the app with hot reload
cargo test --workspace  # run the Rust tests
npm run tauri build     # build .deb, .rpm and AppImage packages
```

### Layout

| Path | What it is |
| --- | --- |
| `src/` | React + TypeScript frontend |
| `src-tauri/` | Tauri shell: window, IPC commands, app state |
| `crates/library-core/` | Data model, SQLite store and store importers, with no Tauri dependency |

The library database lives in the app data directory (`~/.local/share/io.github.burtonjaden3-collab.gamelibrary/library.db` on Linux).

## License

[MIT](LICENSE)
