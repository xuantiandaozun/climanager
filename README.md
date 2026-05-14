# CLI Manager

CLI Manager 是一个面向程序员自用场景的 AI CLI 编码工具工作区管理器。

它优先解决自己的真实痛点：在多个项目里使用 `opencode`、`codex`、`claude code` 这类终端编码工具时，项目路径、启动入口和会话历史分散在不同位置，下一次恢复上下文很麻烦。

## 定位

- 开源优先，先发布到 GitHub。
- 自用优先，先服务高频个人工作流。
- 本地优先，不默认依赖云端账号或服务。
- 桌面优先，后续如果有人关注再逐步维护。
- 上架商店只是顺带目标，不作为早期产品约束。

## 当前阶段

这是早期可用版本，已经具备：

- Tauri 2 + Vue 3 + TypeScript 桌面应用基础结构
- 本地 SQLite 持久化
- 添加和删除工作区目录
- 检测 `opencode`、`codex`、`claude` 是否在 `PATH` 中
- 在指定工作区目录启动 CLI 工具
- 记录启动历史
- 扫描并索引工具会话历史文件
- 配置代理端口，并在启动终端时自动注入代理环境变量

启动终端时，如果代理启用，会自动设置：

```bash
HTTP_PROXY=http://host:port
HTTPS_PROXY=http://host:port
ALL_PROXY=http://host:port
```

Windows 当前默认使用 `powershell.exe -NoExit -Command <tool>` 打开新终端窗口。

## 计划支持的工具

- opencode
- codex
- claude code
- 自定义 CLI 命令

## 本地数据

数据保存到 Tauri 的应用数据目录，数据库文件名为：

```text
climanager.sqlite3
```

当前表包括：

- `workspaces`
- `tools`
- `launch_history`
- `sessions`
- `settings`

## 会话索引

当前会话索引采用保守策略：扫描常见工具目录中的文本类文件，如果文件内容包含工作区路径或工作区名称，就把它关联到对应工作区。

默认扫描范围包括：

- `~/.opencode`
- `~/.local/share/opencode`
- `~/.codex`
- `~/.claude`
- `~/AppData/Roaming/opencode`
- `~/AppData/Roaming/Claude`

这一版只做索引，不修改原始会话文件。

## 开发

```bash
pnpm install
pnpm dev
```

桌面模式：

```bash
pnpm dev:desktop
```

类型检查和构建：

```bash
pnpm typecheck
pnpm build
```
