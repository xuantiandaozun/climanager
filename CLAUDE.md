# CLAUDE.md

## 项目概述

Tauri 2 桌面应用，Vue 3/TypeScript 前端 + Rust 后端。

- 前端入口：`src/main.ts`、`src/App.vue`、`src/style.css`
- Rust 入口：`src-tauri/src/main.rs` 调用 `climanager_lib::run()`，逻辑主体在 `src-tauri/src/lib.rs`
- 新增 Tauri 命令须注册到 `generate_handler!` 列表

## 常用命令

```bash
pnpm install          # 安装依赖
pnpm dev              # 前端开发服务器
pnpm dev:desktop      # 桌面开发模式
pnpm typecheck        # 前端类型检查
pnpm build            # 前端生产构建
pnpm build:desktop    # 完整桌面构建
cargo check --manifest-path src-tauri/Cargo.toml  # Rust 检查
```

## 构建约束

- Vite 必须运行在端口 `1420`（`vite.config.ts` 已设 `strictPort: true`，Tauri `devUrl` 指向该端口）
- Tauri `beforeDevCommand` 为 `pnpm dev`，不得在其他端口启动额外开发服务器
- Windows 资源生成依赖 `src-tauri/icons/icon.ico`，缺失会导致构建失败

## 本地数据

- SQLite 使用 `rusqlite`（bundled feature），无需外部安装
- 数据库位于 Tauri 应用数据目录，文件名 `climanager.sqlite3`，不在仓库中
- 表结构在 `init_database` 中创建：`workspaces`、`tools`、`launch_history`、`sessions`、`settings`
- 内置工具在 Rust 中预填：`opencode`、`codex`、`claude code`（命令为 `claude`）

## 平台行为（不可破坏）

- 工作区路径规范化后以正斜杠存储
- 工具检测：Windows 用 `where.exe`，其他平台用 `which`
- Windows 工具启动：`powershell.exe -NoExit -Command <tool>`，带 `CREATE_NEW_CONSOLE`
- 代理配置仅向已启动的终端进程注入 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`，不得修改系统代理或全局环境变量

## Session 索引

- 仅索引不超过 2 MB 的文本类文件
- 文件与工作区关联条件：内容包含规范化路径、Windows 风格路径或工作区名称之一
- 索引过程中不得修改原始 session 文件
- 默认扫描根目录定义在 `session_roots`；变更时同步更新 README 和 `docs/README.zh-CN.md`

## 验证流程

| 变更范围 | 验证命令 |
|---|---|
| 仅前端 | `pnpm typecheck` |
| UI / 构建配置 | `pnpm typecheck && pnpm build` |
| Rust / Tauri 命令 | `cargo check --manifest-path src-tauri/Cargo.toml` |
| 跨 IPC 边界 | 两者都跑 |

## 忽略文件

不提交：`node_modules/`、`dist/`、`dist-ssr/`、`src-tauri/target/`

`src-tauri/gen/schemas/` 已提交，供 Tauri capability schema 引用使用。
