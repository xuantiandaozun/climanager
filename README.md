# CLI Manager

[中文说明](docs/README.zh-CN.md)

[Roadmap](ROADMAP.md)

CLI Manager is a local-first desktop manager for AI coding CLI workflows.

It helps developers organize project workspaces, launch AI coding CLIs such as Codex, Claude Code, and OpenCode from the selected workspace directory, manage proxy settings, and index local session history. CLI Manager itself keeps workspace metadata and session indexing local instead of sending that data to any remote server.

The goal is to make AI-assisted development workflows more reliable, private, and reproducible for open source maintainers and solo developers.

## Features

- Manage project workspaces with grouping, favorites, and archiving
- Search and filter workspaces by name, path, or group
- Set a default CLI tool per workspace
- Launch CLI tools from a selected workspace directory
- Quick launch from the overview with recent workspace cards
- Detect whether built-in tools are available in `PATH`
- Persist local data with SQLite
- Record launch history
- View session records in a modal dialog per workspace
- Configure a proxy for launched terminal sessions
- Index session files from common AI CLI history directories
- Minimize to system tray
- Keep all data local by default

## Supported Tools

Built-in tool entries:

- `opencode`
- `codex`
- `claude code`

Custom CLI configuration is planned.

## Proxy Injection

When proxy support is enabled, CLI Manager injects these environment variables into the launched terminal process:

```bash
HTTP_PROXY=http://host:port
HTTPS_PROXY=http://host:port
ALL_PROXY=http://host:port
```

This does not modify system proxy settings or global environment variables.

On Windows, the current launcher uses:

```text
powershell.exe -NoExit -Command <tool>
```

## Session Indexing

Session indexing currently uses a conservative matching strategy. CLI Manager scans common local history directories and associates text files with a workspace when the file content contains the workspace path or workspace name.

Default scan locations include:

- `~/.opencode`
- `~/.local/share/opencode`
- `~/.codex`
- `~/.claude`
- `~/.antigravity`
- `~/AppData/Roaming/opencode`
- `~/AppData/Roaming/Claude`
- `~/AppData/Local/agy`

The original session files are not modified.

## Local Data

Data is stored in the Tauri app data directory. The SQLite database file is:

```text
climanager.sqlite3
```

Current tables:

- `workspaces`
- `tools`
- `launch_history`
- `sessions`
- `settings`

## Tech Stack

- Tauri 2
- Vue 3
- TypeScript
- Rust
- SQLite

## Development

Install dependencies:

```bash
pnpm install
```

Run the web dev server:

```bash
pnpm dev
```

Run the desktop app:

```bash
pnpm dev:desktop
```

Type-check and build:

```bash
pnpm typecheck
pnpm build
```

Check the Rust backend:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
