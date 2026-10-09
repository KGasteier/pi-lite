[Deutsch](README.md) · **English**

# pi-lite

A lightweight desktop frontend for [pi](https://github.com/earendil-works/pi), using
Tauri 2, the system webview and plain JavaScript. No Electron or web server.

**Prototype:** developed on Omarchy/Arch Linux aarch64. Fresh-machine installation,
x86_64 and the latest UI additions are not fully live-tested. macOS and Windows
are not supported installation targets yet.

## Install on Omarchy

Requires an existing configured pi installation (`pi` on PATH; 1.0.4 recommended),
Git, Internet access and access to this private repository.

```bash
git clone git@github.com:KGasteier/pi-lite.git
cd pi-lite
./scripts/setup-omarchy.sh
```

Setup installs missing Arch build/system libraries through sudo/pacman, installs
Rust through existing mise or pacman if necessary, tests and builds for the host
architecture, then installs a user-local launcher and icon. Search for **pi-lite**
in Omarchy's application launcher. No Hyprland configuration is changed.
A sudo password may be required; allow several minutes and several GB of build space.
Update a partially upgraded Arch system normally before setup.

## Features

Streaming Markdown chat, tools/thinking, sessions, model selection, workspace
picker, configuration browser/editor, keyboard navigation (F1), prompt history,
Markdown copying, prompt recall, token statistics and optional AI explanations.
[Slash-command completion](docs/SLASH_COMMANDS.md) in the input: type to filter,
hover/arrows to select, Tab to complete, Enter/click to execute directly.
VSCodium integration opens workspaces and referenced files with explicit clicks,
without a second agent (see [editor integration](docs/EDITOR_INTEGRATION.md), German).
[Session bookmarks](docs/SESSION_BOOKMARKS.md) (German): persistent small bookmark
buttons, time/bookmark sorting and a bookmarks-only filter in the sidebar.
A small animated pixel snake precedes the busy status in the header.
[Material attachments](docs/MATERIAL.md) (German): files/URLs for the next prompt,
image input, bounded local PDF text extraction and manual processing/model choice.
[Audio/video analysis](docs/MEDIA_ANALYSIS.md): isolated OpenRouter call through
explicitly verified routes with confirmation; keeps the main model unchanged.
Requires configuration; parallel specialist jobs and fixed cost budgets remain unimplemented.

Optional explanations require `PI_LITE_HELP_PROVIDER` and `PI_LITE_HELP_MODEL` in
the app environment, matching your existing pi model configuration (the newest available Haiku
version is recommended). No provider
or credentials are bundled. Transmission requires confirmation. File previews
may include confidential excerpts and paths; local caches can be deleted via the link at the bottom of a preview.

Optional speech reuses an existing compatible Linux Piper `vorlesen` pi extension.
The installer backs it up before adapting it for pi-lite RPC. No TTS packages or
voices are downloaded. Disable adaptation with `PI_LITE_SETUP_SPEECH=0`.

## Contribute

Use feature branches and pull requests; see [CONTRIBUTING.md](CONTRIBUTING.md).
Run JavaScript checks/tests and `cargo test --locked`. Rebuild after UI changes.
Never commit credentials, private sessions or machine-specific configuration.
Detailed documentation is currently German: [README.md](README.md).

The project's own license is not yet selected. This private repository is intended
for collaboration with invited colleagues, not public redistribution. Third-party
libraries retain their respective licenses.
