# AGENTS.md

## Project Shape

- This is a Tauri 2 desktop app with a Vue 3/TypeScript frontend and Rust backend.
- Frontend entrypoints: `src/main.ts`, `src/App.vue`, `src/style.css`.
- Tauri/Rust entrypoints: `src-tauri/src/main.rs` calls `climanager_lib::run()` in `src-tauri/src/lib.rs`.
- Most backend logic currently lives in `src-tauri/src/lib.rs`; keep new Tauri commands registered in the `generate_handler!` list.

## Commands

- Install deps with `pnpm install`.
- Web dev server: `pnpm dev`.
- Desktop dev app: `pnpm dev:desktop`.
- Frontend typecheck: `pnpm typecheck`.
- Frontend production build: `pnpm build`.
- Rust backend check: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Full desktop build: `pnpm build:desktop`.

## Build Wiring

- Vite must run on port `1420`; `vite.config.ts` has `strictPort: true` and Tauri `devUrl` points to `http://localhost:1420`.
- Tauri `beforeDevCommand` is `pnpm dev`; do not start another dev server on a different port without updating `src-tauri/tauri.conf.json`.
- Tauri bundle icon is `src-tauri/icons/icon.ico`; Windows resource generation fails if it is missing.

## Local Data

- SQLite uses `rusqlite` with the bundled SQLite feature; no external SQLite installation is required.
- The database is created in Tauri's app data directory as `climanager.sqlite3`, not in the repo.
- Current tables are created in `init_database`: `workspaces`, `tools`, `launch_history`, `sessions`, `settings`.
- Built-in tools are seeded in Rust: `opencode`, `codex`, and `claude code` with command `claude`.

## Native Behavior To Preserve

- Workspace paths are canonicalized and stored with forward slashes.
- Tool detection uses `where.exe` on Windows and `which` elsewhere.
- Windows tool launch uses `powershell.exe -NoExit -Command <tool>` with `CREATE_NEW_CONSOLE`.
- Proxy config only injects `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` into the launched terminal process; it must not change system proxy settings or global env vars.

## Session Indexing

- Session scanning is intentionally conservative: it only indexes text-like files up to 2 MB.
- A file is associated with a workspace only when its content contains the normalized workspace path, Windows-style workspace path, or workspace name.
- Do not modify original session files during indexing.
- Default scan roots are defined in `session_roots`; update README and `docs/README.zh-CN.md` when changing them.

## Verification

- For frontend-only changes, run `pnpm typecheck`; run `pnpm build` if UI/build config changed.
- For Rust/Tauri command changes, run `cargo check --manifest-path src-tauri/Cargo.toml`.
- For changes crossing the IPC boundary, run both `pnpm typecheck` and the Rust cargo check.

## Generated And Ignored Files

- Do not commit `node_modules/`, `dist/`, `dist-ssr/`, or `src-tauri/target/`.
- `src-tauri/gen/schemas/` is currently committed and used by the Tauri capability schema references.
