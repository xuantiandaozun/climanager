# CLI Manager

[中文说明](docs/README.zh-CN.md)

CLI Manager is a local-first desktop app for managing AI coding CLI workspaces.

It helps connect project folders, CLI launchers, proxy settings, launch history, and indexed session files in one place. The project is designed for developers who regularly switch between multiple repositories and AI CLI tools such as `opencode`, `codex`, and `claude code`.

## Features

- Manage project workspaces
- Launch CLI tools from a selected workspace directory
- Detect whether built-in tools are available in `PATH`
- Persist local data with SQLite
- Record launch history
- Configure a proxy for launched terminal sessions
- Index session files from common AI CLI history directories
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
- `~/AppData/Roaming/opencode`
- `~/AppData/Roaming/Claude`

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
