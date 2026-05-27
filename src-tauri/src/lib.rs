use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WindowEvent};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

struct AppState {
    db: Mutex<Connection>,
}

#[derive(Debug, Serialize)]
struct Workspace {
    id: i64,
    name: String,
    path: String,
    created_at: String,
    last_opened_at: Option<String>,
    favorite: bool,
    group_name: String,
    archived: bool,
    default_tool_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CliTool {
    id: i64,
    name: String,
    command: String,
    detected_path: Option<String>,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct LaunchRecord {
    id: i64,
    workspace_name: String,
    workspace_path: String,
    tool_name: String,
    command: String,
    launched_at: String,
    proxy_enabled: bool,
    proxy_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionRecord {
    id: i64,
    workspace_name: String,
    tool_name: String,
    title: String,
    source_path: String,
    updated_at: String,
    matched_by: String,
}

#[derive(Debug, Serialize)]
struct ToolSessionList {
    command: String,
    output: String,
    stderr: String,
    lines: Vec<String>,
    sessions: Vec<ToolSessionItem>,
}

#[derive(Debug, Serialize)]
struct ToolSessionItem {
    id: String,
    title: String,
    updated: String,
    line: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProxyConfig {
    enabled: bool,
    host: String,
    port: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceInput {
    name: Option<String>,
    path: String,
}

#[derive(Debug, Deserialize)]
struct UpdateWorkspaceInput {
    id: i64,
    name: String,
    group_name: String,
}

#[derive(Debug, Deserialize)]
struct LaunchInput {
    workspace_id: i64,
    tool_id: i64,
}

#[derive(Debug, Deserialize)]
struct OpenSessionInput {
    workspace_id: i64,
    tool_id: i64,
    session_id: String,
}

#[tauri::command]
fn app_ready() -> &'static str {
    "CLI Manager is ready"
}

#[tauri::command]
fn list_workspaces(state: tauri::State<'_, AppState>) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let mut stmt = db
        .prepare(
            "select id, name, path, created_at, last_opened_at, favorite, group_name, archived, default_tool_id
             from workspaces
             order by archived asc, favorite desc, coalesce(last_opened_at, created_at) desc, lower(name) asc, id desc",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], workspace_from_row)
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn add_workspace(
    state: tauri::State<'_, AppState>,
    input: WorkspaceInput,
) -> Result<Vec<Workspace>, String> {
    let path = normalize_path(&input.path)?;
    let name = input
        .name
        .unwrap_or_else(|| workspace_name_from_path(&path));
    let now = Utc::now().to_rfc3339();
    let db = state.db.lock().map_err(|err| err.to_string())?;

    db.execute(
        "insert into workspaces (name, path, created_at, last_opened_at)
         values (?1, ?2, ?3, ?3)
         on conflict(path) do update set name = excluded.name, last_opened_at = excluded.last_opened_at",
        params![name, path, now],
    )
    .map_err(|err| err.to_string())?;

    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn delete_workspace(state: tauri::State<'_, AppState>, id: i64) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    db.execute("delete from workspaces where id = ?1", params![id])
        .map_err(|err| err.to_string())?;
    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn update_workspace(
    state: tauri::State<'_, AppState>,
    input: UpdateWorkspaceInput,
) -> Result<Vec<Workspace>, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("工作区名称不能为空".to_string());
    }

    let group_name = input.group_name.trim();
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let changed = db
        .execute(
            "update workspaces set name = ?1, group_name = ?2 where id = ?3",
            params![name, group_name, input.id],
        )
        .map_err(|err| err.to_string())?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn toggle_workspace_favorite(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let changed = db
        .execute(
            "update workspaces set favorite = case favorite when 1 then 0 else 1 end where id = ?1",
            params![id],
        )
        .map_err(|err| err.to_string())?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn toggle_workspace_archived(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let changed = db
        .execute(
            "update workspaces set archived = case archived when 1 then 0 else 1 end where id = ?1",
            params![id],
        )
        .map_err(|err| err.to_string())?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn open_workspace_in_vscode(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace = read_workspace(&db, id)?;
    open_in_vscode(&workspace.path)?;

    let now = Utc::now().to_rfc3339();
    db.execute(
        "update workspaces set last_opened_at = ?1 where id = ?2",
        params![now, workspace.id],
    )
    .map_err(|err| err.to_string())?;

    drop(db);
    list_workspaces(state)
}

#[tauri::command]
fn list_tools(state: tauri::State<'_, AppState>) -> Result<Vec<CliTool>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let mut stmt = db
        .prepare("select id, name, command, enabled from tools order by id asc")
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let command: String = row.get(2)?;
            Ok(CliTool {
                id: row.get(0)?,
                name: row.get(1)?,
                detected_path: detect_command(&command),
                command,
                enabled: row.get::<_, i64>(3)? == 1,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_proxy_config(state: tauri::State<'_, AppState>) -> Result<ProxyConfig, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    read_proxy_config(&db)
}

#[tauri::command]
fn save_proxy_config(
    state: tauri::State<'_, AppState>,
    config: ProxyConfig,
) -> Result<ProxyConfig, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    db.execute(
        "insert into settings (key, value) values ('proxy', ?1)
         on conflict(key) do update set value = excluded.value",
        params![serde_json::to_string(&config).map_err(|err| err.to_string())?],
    )
    .map_err(|err| err.to_string())?;
    Ok(config)
}

#[tauri::command]
fn list_launch_history(state: tauri::State<'_, AppState>) -> Result<Vec<LaunchRecord>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let mut stmt = db
        .prepare(
            "select h.id, w.name, w.path, t.name, h.command, h.launched_at, h.proxy_enabled, h.proxy_url
             from launch_history h
             join workspaces w on w.id = h.workspace_id
             join tools t on t.id = h.tool_id
             order by h.launched_at desc
             limit 30",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let workspace_path: String = row.get(2)?;
            Ok(LaunchRecord {
                id: row.get(0)?,
                workspace_name: row.get(1)?,
                workspace_path: clean_path_prefix(&workspace_path),
                tool_name: row.get(3)?,
                command: row.get(4)?,
                launched_at: row.get(5)?,
                proxy_enabled: row.get::<_, i64>(6)? == 1,
                proxy_url: row.get(7)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<SessionRecord>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    read_sessions(&db)
}

#[tauri::command]
fn scan_sessions(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SessionRecord>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspaces = read_workspaces(&db)?;
    let tools = read_tools(&db)?;
    let roots = session_roots(&app);
    let mut indexed = 0;

    for workspace in &workspaces {
        for tool in &tools {
            for root in roots_for_tool(&roots, &tool.name) {
                if !root.exists() {
                    continue;
                }
                indexed += scan_root_for_workspace(&db, workspace, tool, &root)?;
            }
        }
    }

    if indexed == 0 {
        return read_sessions(&db);
    }

    read_sessions(&db)
}

#[tauri::command]
fn list_tool_sessions(
    state: tauri::State<'_, AppState>,
    input: LaunchInput,
) -> Result<ToolSessionList, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace = read_workspace(&db, input.workspace_id)?;
    let tool = read_tool(&db, input.tool_id)?;
    let proxy = read_proxy_config(&db)?;
    let proxy_url = proxy_url(&proxy);
    drop(db);

    run_tool_session_command(
        &workspace.path,
        &tool.name,
        &tool.command,
        proxy_url.as_deref(),
    )
}

#[tauri::command]
fn open_tool_session(
    state: tauri::State<'_, AppState>,
    input: OpenSessionInput,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace = read_workspace(&db, input.workspace_id)?;
    let tool = read_tool(&db, input.tool_id)?;
    let proxy = read_proxy_config(&db)?;
    let proxy_url = proxy_url(&proxy);
    drop(db);

    let command = tool_open_session_command(&tool.name, &tool.command, &input.session_id)?;
    spawn_terminal(&workspace.path, &tool.name, &command, proxy_url.as_deref())
}

#[tauri::command]
fn launch_tool(
    state: tauri::State<'_, AppState>,
    input: LaunchInput,
) -> Result<Vec<LaunchRecord>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace: Workspace = db
        .query_row(
            "select id, name, path, created_at, last_opened_at, favorite, group_name, archived, default_tool_id from workspaces where id = ?1",
            params![input.workspace_id],
            workspace_from_row,
        )
        .map_err(|_| "工作区不存在".to_string())?;

    let tool: CliTool = db
        .query_row(
            "select id, name, command, enabled from tools where id = ?1",
            params![input.tool_id],
            |row| {
                let command: String = row.get(2)?;
                Ok(CliTool {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    detected_path: detect_command(&command),
                    command,
                    enabled: row.get::<_, i64>(3)? == 1,
                })
            },
        )
        .map_err(|_| "工具不存在".to_string())?;

    if !tool.enabled {
        return Err("工具已禁用".to_string());
    }

    let proxy = read_proxy_config(&db)?;
    let proxy_url = proxy_url(&proxy);
    let launch_command = if tool.name.to_lowercase().contains("codex") {
        format!(
            "{} --sandbox workspace-write --ask-for-approval on-request -c approvals_reviewer=auto_review",
            tool.command.trim()
        )
    } else {
        tool.command.clone()
    };
    spawn_terminal(
        &workspace.path,
        &tool.name,
        &launch_command,
        proxy_url.as_deref(),
    )?;

    let now = Utc::now().to_rfc3339();
    db.execute(
        "insert into launch_history (workspace_id, tool_id, command, launched_at, proxy_enabled, proxy_url)
         values (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            workspace.id,
            tool.id,
            tool.command,
            now,
            if proxy_url.is_some() { 1 } else { 0 },
            proxy_url
        ],
    )
    .map_err(|err| err.to_string())?;
    db.execute(
        "update workspaces set last_opened_at = ?1 where id = ?2",
        params![now, workspace.id],
    )
    .map_err(|err| err.to_string())?;

    drop(db);
    list_launch_history(state)
}

fn init_database(app: &AppHandle) -> Result<Connection, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("无法获取应用数据目录: {err}"))?;
    fs::create_dir_all(&app_dir).map_err(|err| err.to_string())?;
    let db_path = app_dir.join("climanager.sqlite3");
    let db = Connection::open(db_path).map_err(|err| err.to_string())?;

    db.execute_batch(
        "create table if not exists workspaces (
            id integer primary key autoincrement,
            name text not null,
            path text not null unique,
            created_at text not null,
            last_opened_at text,
            favorite integer not null default 0,
            group_name text not null default '',
            archived integer not null default 0
        );

        create table if not exists tools (
            id integer primary key autoincrement,
            name text not null unique,
            command text not null,
            enabled integer not null default 1
        );

        create table if not exists launch_history (
            id integer primary key autoincrement,
            workspace_id integer not null,
            tool_id integer not null,
            command text not null,
            launched_at text not null,
            proxy_enabled integer not null default 0,
            proxy_url text,
            foreign key(workspace_id) references workspaces(id) on delete cascade,
            foreign key(tool_id) references tools(id) on delete cascade
        );

        create table if not exists sessions (
            id integer primary key autoincrement,
            workspace_id integer not null,
            tool_id integer not null,
            title text not null,
            source_path text not null,
            updated_at text not null,
            matched_by text not null,
            unique(workspace_id, tool_id, source_path),
            foreign key(workspace_id) references workspaces(id) on delete cascade,
            foreign key(tool_id) references tools(id) on delete cascade
        );

        create table if not exists settings (
            key text primary key,
            value text not null
        );",
    )
    .map_err(|err| err.to_string())?;

    migrate_database(&db)?;
    seed_tools(&db)?;
    Ok(db)
}

fn migrate_database(db: &Connection) -> Result<(), String> {
    let mut stmt = db
        .prepare("pragma table_info(workspaces)")
        .map_err(|err| err.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| err.to_string())?;

    let mut has_favorite = false;
    let mut has_group_name = false;
    let mut has_archived = false;
    let mut has_default_tool_id = false;
    for column in columns {
        match column.map_err(|err| err.to_string())?.as_str() {
            "favorite" => has_favorite = true,
            "group_name" => has_group_name = true,
            "archived" => has_archived = true,
            "default_tool_id" => has_default_tool_id = true,
            _ => {}
        }
    }

    if !has_favorite {
        db.execute(
            "alter table workspaces add column favorite integer not null default 0",
            [],
        )
        .map_err(|err| err.to_string())?;
    }

    if !has_group_name {
        db.execute(
            "alter table workspaces add column group_name text not null default ''",
            [],
        )
        .map_err(|err| err.to_string())?;
    }

    if !has_archived {
        db.execute(
            "alter table workspaces add column archived integer not null default 0",
            [],
        )
        .map_err(|err| err.to_string())?;
    }

    if !has_default_tool_id {
        db.execute(
            "alter table workspaces add column default_tool_id integer references tools(id) on delete set null",
            [],
        )
        .map_err(|err| err.to_string())?;
    }

    db.execute(
        "create index if not exists idx_workspaces_activity on workspaces(archived, favorite, last_opened_at, created_at)",
        [],
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

fn seed_tools(db: &Connection) -> Result<(), String> {
    let tools = [
        ("opencode", "opencode"),
        ("codex", "codex"),
        ("claude code", "claude"),
        ("antigravity cli", "agy"),
    ];

    for (name, command) in tools {
        db.execute(
            "insert into tools (name, command, enabled) values (?1, ?2, 1)
             on conflict(name) do nothing",
            params![name, command],
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn read_workspaces(db: &Connection) -> Result<Vec<Workspace>, String> {
    let mut stmt = db
        .prepare(
            "select id, name, path, created_at, last_opened_at, favorite, group_name, archived, default_tool_id
             from workspaces
             order by archived asc, favorite desc, coalesce(last_opened_at, created_at) desc, lower(name) asc, id desc",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], workspace_from_row)
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn read_workspace(db: &Connection, id: i64) -> Result<Workspace, String> {
    db.query_row(
        "select id, name, path, created_at, last_opened_at, favorite, group_name, archived, default_tool_id from workspaces where id = ?1",
        params![id],
        workspace_from_row,
    )
    .map_err(|_| "工作区不存在".to_string())
}

fn workspace_from_row(row: &Row<'_>) -> rusqlite::Result<Workspace> {
    let path: String = row.get(2)?;
    Ok(Workspace {
        id: row.get(0)?,
        name: row.get(1)?,
        path: clean_path_prefix(&path),
        created_at: row.get(3)?,
        last_opened_at: row.get(4)?,
        favorite: row.get::<_, i64>(5)? == 1,
        group_name: row.get(6)?,
        archived: row.get::<_, i64>(7)? == 1,
        default_tool_id: row.get(8)?,
    })
}

#[tauri::command]
fn set_workspace_default_tool(
    state: tauri::State<'_, AppState>,
    workspace_id: i64,
    tool_id: Option<i64>,
) -> Result<Vec<Workspace>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let changed = db
        .execute(
            "update workspaces set default_tool_id = ?1 where id = ?2",
            params![tool_id, workspace_id],
        )
        .map_err(|err| err.to_string())?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    drop(db);
    list_workspaces(state)
}

fn read_tools(db: &Connection) -> Result<Vec<CliTool>, String> {
    let mut stmt = db
        .prepare("select id, name, command, enabled from tools order by id asc")
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let command: String = row.get(2)?;
            Ok(CliTool {
                id: row.get(0)?,
                name: row.get(1)?,
                detected_path: detect_command(&command),
                command,
                enabled: row.get::<_, i64>(3)? == 1,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn read_tool(db: &Connection, id: i64) -> Result<CliTool, String> {
    db.query_row(
        "select id, name, command, enabled from tools where id = ?1",
        params![id],
        |row| {
            let command: String = row.get(2)?;
            Ok(CliTool {
                id: row.get(0)?,
                name: row.get(1)?,
                detected_path: detect_command(&command),
                command,
                enabled: row.get::<_, i64>(3)? == 1,
            })
        },
    )
    .map_err(|_| "工具不存在".to_string())
}

fn read_sessions(db: &Connection) -> Result<Vec<SessionRecord>, String> {
    let mut stmt = db
        .prepare(
            "select s.id, w.name, t.name, s.title, s.source_path, s.updated_at, s.matched_by
             from sessions s
             join workspaces w on w.id = s.workspace_id
             join tools t on t.id = s.tool_id
             order by s.updated_at desc
             limit 80",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                workspace_name: row.get(1)?,
                tool_name: row.get(2)?,
                title: row.get(3)?,
                source_path: row.get(4)?,
                updated_at: row.get(5)?,
                matched_by: row.get(6)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn read_proxy_config(db: &Connection) -> Result<ProxyConfig, String> {
    let value: Option<String> = db
        .query_row(
            "select value from settings where key = 'proxy'",
            [],
            |row| row.get(0),
        )
        .ok();

    match value {
        Some(value) => serde_json::from_str(&value).map_err(|err| err.to_string()),
        None => Ok(ProxyConfig {
            enabled: false,
            host: "127.0.0.1".to_string(),
            port: "7890".to_string(),
        }),
    }
}

fn proxy_url(config: &ProxyConfig) -> Option<String> {
    if !config.enabled || config.host.trim().is_empty() || config.port.trim().is_empty() {
        return None;
    }

    Some(format!(
        "http://{}:{}",
        config.host.trim(),
        config.port.trim()
    ))
}

fn normalize_path(path: &str) -> Result<String, String> {
    let path = PathBuf::from(path);
    let canonical = path
        .canonicalize()
        .map_err(|_| "请选择一个存在的工作区目录".to_string())?;

    if !canonical.is_dir() {
        return Err("工作区必须是目录".to_string());
    }

    Ok(clean_path_prefix(&canonical.to_string_lossy()))
}

fn clean_path_prefix(path: &str) -> String {
    let normalized = path.replace('\\', "/");

    if let Some(path) = normalized.strip_prefix("//?/UNC/") {
        return format!("//{path}");
    }

    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn workspace_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn detect_command(command: &str) -> Option<String> {
    let binary = command.split_whitespace().next()?;
    let output = if cfg!(windows) {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("where.exe")
            .arg(binary)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?
    } else {
        Command::new("which").arg(binary).output().ok()?
    };

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .and_then(|stdout| stdout.lines().next().map(|line| line.trim().to_string()))
        .filter(|line| !line.is_empty())
}

fn session_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(home) = app.path().home_dir() {
        roots.extend([
            home.join(".opencode"),
            home.join(".codex"),
            home.join(".claude"),
            home.join(".antigravity"),
            home.join(".local").join("share").join("opencode"),
            home.join("AppData").join("Roaming").join("opencode"),
            home.join("AppData").join("Roaming").join("Claude"),
            home.join("AppData").join("Local").join("agy"),
        ]);
    }

    roots
}

fn roots_for_tool(roots: &[PathBuf], tool_name: &str) -> Vec<PathBuf> {
    let needle = tool_name.to_lowercase();
    roots
        .iter()
        .filter(|root| {
            let root_name = root.to_string_lossy().to_lowercase();
            if needle.contains("claude") {
                root_name.contains("claude")
            } else if needle.contains("codex") {
                root_name.contains("codex")
            } else if needle.contains("opencode") {
                root_name.contains("opencode")
            } else if needle.contains("antigravity") {
                root_name.contains("antigravity") || root_name.contains("agy")
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

fn scan_root_for_workspace(
    db: &Connection,
    workspace: &Workspace,
    tool: &CliTool,
    root: &Path,
) -> Result<usize, String> {
    let mut found = 0;
    let mut stack = vec![root.to_path_buf()];
    let workspace_path = workspace.path.replace('\\', "/").to_lowercase();
    let workspace_path_windows = workspace.path.replace('/', "\\").to_lowercase();
    let workspace_name = workspace.name.to_lowercase();

    while let Some(path) = stack.pop() {
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if stack.len() < 512 {
                    stack.push(path);
                }
                continue;
            }

            if !is_session_candidate(&path) {
                continue;
            }

            let Some(content) = read_small_text_file(&path) else {
                continue;
            };
            let content = content.to_lowercase();
            let matched_by = if content.contains(&workspace_path) {
                "workspace_path"
            } else if content.contains(&workspace_path_windows) {
                "workspace_path_windows"
            } else if content.contains(&workspace_name) {
                "workspace_name"
            } else {
                continue;
            };

            let updated_at = updated_at(&path);
            let title = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("session")
                .to_string();
            let source_path = path.to_string_lossy().replace('\\', "/");

            db.execute(
                "insert into sessions (workspace_id, tool_id, title, source_path, updated_at, matched_by)
                 values (?1, ?2, ?3, ?4, ?5, ?6)
                 on conflict(workspace_id, tool_id, source_path)
                 do update set title = excluded.title, updated_at = excluded.updated_at, matched_by = excluded.matched_by",
                params![workspace.id, tool.id, title, source_path, updated_at, matched_by],
            )
            .map_err(|err| err.to_string())?;
            found += 1;
        }
    }

    Ok(found)
}

fn is_session_candidate(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    matches!(
        ext.to_lowercase().as_str(),
        "json" | "jsonl" | "md" | "txt" | "log" | "yaml" | "yml"
    )
}

fn read_small_text_file(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > 2_000_000 {
        return None;
    }

    let mut file = fs::File::open(path).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    Some(content)
}

fn updated_at(path: &Path) -> String {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or_else(|_| SystemTime::now());
    let datetime: chrono::DateTime<Utc> = modified.into();
    datetime.to_rfc3339()
}

fn run_tool_session_command(
    cwd: &str,
    tool_name: &str,
    command: &str,
    proxy_url: Option<&str>,
) -> Result<ToolSessionList, String> {
    if let Some(unsupported) = unsupported_background_session_list(tool_name, command) {
        return Ok(unsupported);
    }

    let session_command = tool_session_command(tool_name, command);

    #[cfg(windows)]
    let output = {
        let mut script = String::new();
        if let Some(proxy_url) = proxy_url {
            script.push_str(&format!(
                "$env:HTTP_PROXY='{0}'; $env:HTTPS_PROXY='{0}'; $env:ALL_PROXY='{0}'; ",
                proxy_url
            ));
        }
        script.push_str(&session_command);

        Command::new("powershell.exe")
            .current_dir(cwd)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|err| format!("查询会话列表失败: {err}"))?
    };

    #[cfg(not(windows))]
    let output = {
        let mut shell_command = String::new();
        if let Some(proxy_url) = proxy_url {
            shell_command.push_str(&format!(
                "export HTTP_PROXY='{0}' HTTPS_PROXY='{0}' ALL_PROXY='{0}'; ",
                proxy_url
            ));
        }
        shell_command.push_str(&session_command);

        Command::new("sh")
            .current_dir(cwd)
            .args(["-lc", &shell_command])
            .output()
            .map_err(|err| format!("查询会话列表失败: {err}"))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() && stdout.is_empty() {
        return Err(if stderr.is_empty() {
            "查询会话列表失败".to_string()
        } else {
            stderr
        });
    }

    let lines = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let sessions = parse_tool_sessions(tool_name, &lines);

    Ok(ToolSessionList {
        command: session_command,
        output: stdout,
        stderr,
        lines,
        sessions,
    })
}

fn unsupported_background_session_list(tool_name: &str, command: &str) -> Option<ToolSessionList> {
    let tool_name = tool_name.to_lowercase();
    let command = command.trim();

    if tool_name.contains("codex") {
        let message = "Codex 当前通过交互式选择器恢复会话：codex resume --all。该命令要求真实终端，不能在后台捕获为列表；打开指定会话时使用 codex resume <session_id>。";
        return Some(ToolSessionList {
            command: format!("{command} resume --all"),
            output: message.to_string(),
            stderr: String::new(),
            lines: vec![message.to_string()],
            sessions: Vec::new(),
        });
    }

    if tool_name.contains("claude") {
        let message = "Claude Code 当前通过交互式选择器恢复会话：claude --resume。该命令要求真实终端，不能在后台捕获为列表；打开指定会话时使用 claude --resume <session_id>。";
        return Some(ToolSessionList {
            command: format!("{command} --resume"),
            output: message.to_string(),
            stderr: String::new(),
            lines: vec![message.to_string()],
            sessions: Vec::new(),
        });
    }

    if tool_name.contains("antigravity") {
        let message = "Antigravity CLI 当前通过交互式选择器恢复会话：agy --resume。该命令要求真实终端，不能在后台捕获为列表；打开指定会话时使用 agy --resume <session_id>。";
        return Some(ToolSessionList {
            command: format!("{command} --resume"),
            output: message.to_string(),
            stderr: String::new(),
            lines: vec![message.to_string()],
            sessions: Vec::new(),
        });
    }

    None
}

fn tool_session_command(tool_name: &str, command: &str) -> String {
    let command = command.trim();
    let tool_name = tool_name.to_lowercase();

    if tool_name.contains("opencode") {
        format!("{command} session list")
    } else if tool_name.contains("codex") {
        format!("{command} resume --all")
    } else if tool_name.contains("claude") {
        format!("{command} --resume")
    } else if tool_name.contains("antigravity") {
        format!("{command} --resume")
    } else {
        format!("{command} session")
    }
}

fn tool_open_session_command(
    tool_name: &str,
    command: &str,
    session_id: &str,
) -> Result<String, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }

    let command = command.trim();
    let tool_name = tool_name.to_lowercase();

    if tool_name.contains("opencode") {
        Ok(format!("{command} -s {session_id}"))
    } else if tool_name.contains("codex") {
        Ok(format!("{command} resume {}", shell_arg(session_id)))
    } else if tool_name.contains("claude") {
        Ok(format!("{command} --resume {}", shell_arg(session_id)))
    } else if tool_name.contains("antigravity") {
        Ok(format!("{command} --resume {}", shell_arg(session_id)))
    } else {
        Err(format!("暂不支持打开 {tool_name} 的指定会话"))
    }
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn parse_tool_sessions(tool_name: &str, lines: &[String]) -> Vec<ToolSessionItem> {
    let tool_name = tool_name.to_lowercase();
    if tool_name.contains("opencode") {
        return lines
            .iter()
            .filter_map(|line| parse_opencode_session_line(line))
            .collect();
    }

    lines
        .iter()
        .filter_map(|line| parse_generic_session_line(line))
        .collect()
}

fn parse_opencode_session_line(line: &str) -> Option<ToolSessionItem> {
    let first = line.split_whitespace().next()?;
    if !first.starts_with("ses_") {
        return None;
    }

    let rest = line.strip_prefix(first)?.trim();
    let mut parts = rest.rsplitn(2, char::is_whitespace);
    let updated = parts.next().unwrap_or_default().trim().to_string();
    let title = parts.next().unwrap_or(rest).trim().to_string();

    Some(ToolSessionItem {
        id: first.to_string(),
        title: if title.is_empty() {
            first.to_string()
        } else {
            title
        },
        updated,
        line: line.to_string(),
    })
}

fn parse_generic_session_line(line: &str) -> Option<ToolSessionItem> {
    let first = line.split_whitespace().next()?;
    let looks_like_id = first.len() >= 8
        && first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if !looks_like_id {
        return None;
    }

    let title = line.strip_prefix(first).unwrap_or_default().trim();
    Some(ToolSessionItem {
        id: first.to_string(),
        title: if title.is_empty() {
            first.to_string()
        } else {
            title.to_string()
        },
        updated: String::new(),
        line: line.to_string(),
    })
}

fn spawn_terminal(
    cwd: &str,
    tool_name: &str,
    command: &str,
    proxy_url: Option<&str>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
        let mut script = String::new();
        if let Some(proxy_url) = proxy_url {
            script.push_str(&format!(
                "$env:HTTP_PROXY='{0}'; $env:HTTPS_PROXY='{0}'; $env:ALL_PROXY='{0}'; ",
                proxy_url
            ));
        }
        if is_codex_launch(tool_name, command) {
            script.push_str("chcp 65001 > $null; [Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $OutputEncoding=[Text.UTF8Encoding]::new($false); ");
        }
        script.push_str(command);

        Command::new("powershell.exe")
            .current_dir(cwd)
            .creation_flags(CREATE_NEW_CONSOLE | CREATE_BREAKAWAY_FROM_JOB)
            .args(["-NoExit", "-Command", &script])
            .spawn()
            .map_err(|err| format!("启动终端失败: {err}"))?;

        Ok(())
    }

    #[cfg(not(windows))]
    {
        let mut shell_command = String::new();
        if let Some(proxy_url) = proxy_url {
            shell_command.push_str(&format!(
                "export HTTP_PROXY='{0}' HTTPS_PROXY='{0}' ALL_PROXY='{0}'; ",
                proxy_url
            ));
        }
        shell_command.push_str(command);

        Command::new("sh")
            .current_dir(cwd)
            .args(["-lc", &shell_command])
            .spawn()
            .map_err(|err| format!("启动终端失败: {err}"))?;

        Ok(())
    }
}

fn is_codex_launch(tool_name: &str, command: &str) -> bool {
    tool_name.to_lowercase().contains("codex") || command.to_lowercase().contains("codex")
}

fn open_in_vscode(path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        if detect_command("code").is_none() {
            return Err("未检测到 VS Code 的 code 命令".to_string());
        }

        Command::new("cmd.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/C", "code", path])
            .spawn()
            .map_err(|err| format!("启动 VS Code 失败，请确认已安装 code 命令: {err}"))?;
    }

    #[cfg(not(windows))]
    {
        let vscode_command =
            detect_command("code").ok_or_else(|| "未检测到 VS Code 的 code 命令".to_string())?;
        Command::new(vscode_command)
            .arg(path)
            .spawn()
            .map_err(|err| format!("启动 VS Code 失败，请确认已安装 code 命令: {err}"))?;
    }

    Ok(())
}

#[tauri::command]
fn open_in_explorer(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace = read_workspace(&db, id)?;
    drop(db);

    #[cfg(windows)]
    {
        let win_path = workspace.path.replace('/', "\\");
        Command::new("explorer")
            .arg(&win_path)
            .spawn()
            .map_err(|err| format!("打开资源管理器失败: {err}"))?;
    }

    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(&workspace.path)
        .spawn()
        .map_err(|err| format!("打开 Finder 失败: {err}"))?;

    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(&workspace.path)
        .spawn()
        .map_err(|err| format!("打开文件管理器失败: {err}"))?;

    Ok(())
}

const SINGLE_INSTANCE_PORT: u16 = 14210;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, SINGLE_INSTANCE_PORT);
    let listener = match TcpListener::bind(addr) {
        Ok(l) => Some(l),
        Err(_) => {
            for _ in 0..10 {
                if let Ok(mut stream) = TcpStream::connect(addr) {
                    let _ = stream.write_all(b"show");
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            std::process::exit(0);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            if let Some(listener) = listener {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    for mut stream in listener.incoming().flatten() {
                        let mut buf = [0u8; 4];
                        if stream.read_exact(&mut buf).is_ok() && &buf == b"show" {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });
            }

            let db = init_database(app.handle())?;
            app.manage(AppState { db: Mutex::new(db) });

            let img = image::load_from_memory(include_bytes!("../icons/tray-icon.png"))
                .map_err(|e| e.to_string())?
                .into_rgba8();
            let (w, h) = img.dimensions();
            let icon = Image::new_owned(img.into_raw(), w, h);

            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("CLI Manager")
                .show_menu_on_left_click(true)
                .on_menu_event(
                    |app: &AppHandle, event: MenuEvent| match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    },
                )
                .build(app)?;

            let app_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event: &WindowEvent| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = app_handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_ready,
            list_workspaces,
            add_workspace,
            delete_workspace,
            update_workspace,
            toggle_workspace_favorite,
            toggle_workspace_archived,
            open_workspace_in_vscode,
            open_in_explorer,
            set_workspace_default_tool,
            list_tools,
            get_proxy_config,
            save_proxy_config,
            list_launch_history,
            launch_tool,
            list_sessions,
            scan_sessions,
            list_tool_sessions,
            open_tool_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running CLI Manager");
}
