# rail.

A tiny browser that scrolls sideways.

Tabs live on a horizontal strip, the way windows do in the [niri](https://github.com/YaLTeR/niri)
compositor: every new tab opens as a half-width column, you slide left and right between them,
and one key zooms the whole strip out so you can see everything at once.

Rail is a thin native shell (Rust + GTK4) around the **WebKitGTK that is already installed on
your system**, so the program itself is only ~450 KB and it ships no browser engine of its own.

- one native binary, no Electron, no bundled Chromium
- tabs on a niri-style scrolling strip, with a live zoomed-out overview
- keyboard-first (vim-style `h` / `l`), no title bar, no toolbar, nothing but the page
- **workspaces**: stacked strips, each with its own endless row of tabs; name them, jump with `Ctrl+1`…`9`
- **group by site**: `app.example.com`, `example.com` and `example.com/x` land together
- address bar with history **autocomplete** (inline completion + suggestions)
- **downloads** with a panel, progress, open / show-in-folder
- starts fresh every time: one empty tab, no session restore (workspace names are remembered)
- logins are kept between runs (cookies are stored on disk)

> **Platform:** Linux only (GTK4 + WebKitGTK). Developed on Wayland (niri); X11 should work too.

## Quick start

```sh
git clone <this repo's URL> rail
cd rail
cargo run --release
```

You need a Rust toolchain and the GTK4 / WebKitGTK development packages first (see below).
Open a page directly with `cargo run --release -- https://example.com`.

## Requirements

- **Rust 1.85 or newer** (the crate uses the 2024 edition). Get it from [rustup.rs](https://rustup.rs).
- **GTK 4.12+** and **WebKitGTK 6.0** (the GTK4 build of WebKit) with their development files.
- `bubblewrap` and `xdg-dbus-proxy` at runtime. WebKit uses them for its security sandbox,
  and they are normally installed together with WebKitGTK.
- The Adwaita icon theme (installed on practically every desktop).

Install the build dependencies for your distribution:

| Distribution | Command |
|---|---|
| **Arch / Manjaro** | `sudo pacman -S --needed base-devel rust gtk4 webkitgtk-6.0` |
| **Debian 13+ / Ubuntu 24.04+** | `sudo apt install build-essential pkg-config libgtk-4-dev libwebkitgtk-6.0-dev bubblewrap xdg-dbus-proxy` |
| **Fedora** | `sudo dnf install gcc pkgconf-pkg-config gtk4-devel webkitgtk6.0-devel bubblewrap xdg-dbus-proxy` |

On Debian/Ubuntu and Fedora, install Rust with [rustup](https://rustup.rs) because the packaged
version may be too old (`rustc --version` should say 1.85 or higher).

Check that the libraries are found:

```sh
pkg-config --modversion gtk4 webkitgtk-6.0
```

## Build and run

```sh
cargo build --release          # binary: target/release/rail
./target/release/rail          # run it
./target/release/rail https://example.com   # open a page right away
```

To install it for your user:

```sh
install -Dm755 target/release/rail ~/.local/bin/rail
```

## Keys

Everything is on the keyboard. The page has no toolbar; the address bar appears when you ask for it.

### Tabs and navigation

| Key | Action |
|---|---|
| `Ctrl+T` | New tab (opens half-width next to the current one, with the address bar open) |
| `Ctrl+W` | Close tab |
| `Ctrl+H` / `Ctrl+L` | Focus the tab on the left / right |
| `Ctrl+Shift+H` / `Ctrl+Shift+L` | Move the tab left / right |
| `Ctrl+F` | Toggle the focused tab between half and full width |
| `Ctrl+G` | Go to address (type a URL or a search) |
| `Ctrl+Enter` | Edit the current tab's URL |
| `Ctrl+R` | Reload |
| `Alt+Left` / `Alt+Right` | Back / forward |

### Workspaces

Workspaces are strips stacked on top of each other (like niri's). The sidebar lists them.

| Key | Action |
|---|---|
| `Ctrl+1` … `Ctrl+9` | Go to workspace N (the next free number creates it) |
| `Ctrl+Shift+1` … `9` | Move the focused tab to workspace N |
| `Ctrl+Alt+J` / `Ctrl+Alt+K` | Workspace below / above |
| `F2` | Rename the current workspace |
| `Ctrl+Alt+W` | Close the current workspace and its tabs |
| `Ctrl+Shift+G` | Group tabs by site (stays on while you browse; press again to stop) |

### Address bar and downloads

| Key | Action |
|---|---|
| typing | Completes inline from history (`git` → `github.com`); `Backspace` removes the suggestion |
| `↑` / `↓` | Walk the history suggestions |
| `Ctrl+Shift+D` | Downloads panel (also the button in the sidebar) |

Files go to your XDG Downloads folder (`~/Downloads` by default).

### Overview

| Key | Action |
|---|---|
| `Ctrl+O` | Zoom out to see every tab; press again to open the highlighted one |
| `h` / `l` or arrows | Move between tabs |
| `Enter` / `Space` / click | Open the highlighted tab |
| `x` / `Delete` | Close the highlighted tab |
| `Esc` | Cancel and return to the tab you started from |

### Window

| Key | Action |
|---|---|
| `Ctrl+B` | Show / hide the sidebar |
| `F11` | Fullscreen |
| `Ctrl+Q` | Quit |
| `Esc` | Close the address bar (on a brand-new unused tab it also discards the tab) |

Anything typed in the address bar that is not a URL is searched on DuckDuckGo.
The window has no title bar, so `Ctrl+Q` is how you close it.

## Where your data goes

`~/.local/share/rail/` holds cookies (so you stay logged in), `history.tsv` (address
autocomplete), `downloads.txt` (the downloads list) and `workspaces.txt` (workspace names).
Delete the folder to reset everything. Open tabs are never restored.

## Troubleshooting

**`Failed to open display` / `cannot open display`**
Rail has to be started from inside your graphical session. If the terminal was opened before the
session or from outside it (SSH, an old tmux), `WAYLAND_DISPLAY` is not set. Check with
`echo $WAYLAND_DISPLAY` and start from a terminal opened inside your desktop.

**Blank or black page, or graphics glitches**
Try software compositing, which works around some GPU driver problems:

```sh
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./target/release/rail
```

**`The system library 'webkitgtk-6.0' ... was not found` while building**
The development package is missing. Install the one for your distribution from the table above.
Make sure it is the **6.0** (GTK4) build, not `webkit2gtk-4.1`, which is the GTK3 one.

**Video or audio does not play**
WebKit needs the GStreamer plugins. Install `gst-plugins-good` and `gst-libav`
(Arch) or the equivalent packages for your distribution.

**Google refuses to sign in**
Google occasionally rejects embedded WebKit browsers. If it happens, open an issue; the usual
fix is a custom user agent for `accounts.google.com`.

## Project layout

```
src/main.rs                    the app: workspaces, tabs, sidebar, address bar, downloads, keys
src/history.rs                 history log, autocomplete, site (domain) grouping key
src/strip.rs                   custom GTK layouts: scrolling strip, workspace stack, sliding sidebar
packaging/build-appimage.sh    experimental AppImage builder (see below)
```

## AppImage (experimental)

`packaging/build-appimage.sh` builds `dist/rail-x86_64.AppImage`. **It is not fully
self-contained yet**: WebKit starts its helper processes from a path baked into the system
library, so the AppImage only works on a machine whose WebKitGTK matches the bundled one.
For everyday use, build from source as described above.

## Not done yet

- No ad blocking, private mode, multiple profiles or password manager.
- No find-in-page, bookmarks or full history page.
- Tabs are never suspended, so every open tab stays in memory.
- `Ctrl+J` / `Ctrl+K` are unused (reserved for stacked tabs in a column).
- A failed page load shows WebKit's plain default error page.

## Contributing

Issues and pull requests are welcome. Before sending a change, run `cargo build --release`
and try it for real. Most of the interesting behaviour (scrolling, zoom, key handling)
is visual and only shows up when the app is actually running.
