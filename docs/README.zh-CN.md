# CLI Manager

[English](../README.md)

CLI Manager 是一个本地优先的 AI 编码 CLI 工作区管理桌面应用。

它用于把项目目录、CLI 启动入口、代理配置、启动历史和会话文件索引集中到一个地方，适合经常在多个代码仓库和 AI CLI 工具之间切换的开发者。

## 功能

- 管理项目工作区，支持分组、收藏和归档
- 按名称、路径或分组搜索和筛选工作区
- 为每个工作区设置默认 CLI 工具
- 从指定工作区目录启动 CLI 工具
- 概览页快捷启动，显示最近工作区卡片
- 检测内置工具是否存在于 `PATH`
- 使用 SQLite 保存本地数据
- 记录启动历史
- 在弹窗中查看每个工作区的会话记录
- 为启动的终端会话注入代理环境变量
- 扫描并索引常见 AI CLI 历史目录中的会话文件
- 最小化到系统托盘
- 默认所有数据保存在本地

## 支持的工具

内置工具：

- `opencode`
- `codex`
- `claude code`

后续计划支持自定义 CLI 配置。

## 代理注入

启用代理后，CLI Manager 会在启动终端进程时注入：

```bash
HTTP_PROXY=http://host:port
HTTPS_PROXY=http://host:port
ALL_PROXY=http://host:port
```

该功能不会修改系统代理，也不会修改全局环境变量。

Windows 当前使用：

```text
powershell.exe -NoExit -Command <tool>
```

## 会话索引

当前会话索引采用保守匹配策略：扫描常见本地历史目录中的文本类文件，如果文件内容包含工作区路径或工作区名称，就将其关联到对应工作区。

默认扫描位置包括：

- `~/.opencode`
- `~/.local/share/opencode`
- `~/.codex`
- `~/.claude`
- `~/AppData/Roaming/opencode`
- `~/AppData/Roaming/Claude`

扫描过程不会修改原始会话文件。

## 本地数据

数据保存在 Tauri 应用数据目录中，SQLite 数据库文件名为：

```text
climanager.sqlite3
```

当前表包括：

- `workspaces`
- `tools`
- `launch_history`
- `sessions`
- `settings`

## 技术栈

- Tauri 2
- Vue 3
- TypeScript
- Rust
- SQLite

## 开发

安装依赖：

```bash
pnpm install
```

启动 Web 开发服务：

```bash
pnpm dev
```

启动桌面应用：

```bash
pnpm dev:desktop
```

类型检查和构建：

```bash
pnpm typecheck
pnpm build
```

检查 Rust 后端：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
