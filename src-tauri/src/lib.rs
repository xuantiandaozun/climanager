use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

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
struct LaunchInput {
    workspace_id: i64,
    tool_id: i64,
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
            "select id, name, path, created_at, last_opened_at
             from workspaces
             order by coalesce(last_opened_at, created_at) desc",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
                last_opened_at: row.get(4)?,
            })
        })
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
    let name = input.name.unwrap_or_else(|| workspace_name_from_path(&path));
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
            Ok(LaunchRecord {
                id: row.get(0)?,
                workspace_name: row.get(1)?,
                workspace_path: row.get(2)?,
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
fn launch_tool(
    state: tauri::State<'_, AppState>,
    input: LaunchInput,
) -> Result<Vec<LaunchRecord>, String> {
    let db = state.db.lock().map_err(|err| err.to_string())?;
    let workspace: Workspace = db
        .query_row(
            "select id, name, path, created_at, last_opened_at from workspaces where id = ?1",
            params![input.workspace_id],
            |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                    last_opened_at: row.get(4)?,
                })
            },
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
    spawn_terminal(&workspace.path, &tool.command, proxy_url.as_deref())?;

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
            last_opened_at text
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

    seed_tools(&db)?;
    Ok(db)
}

fn seed_tools(db: &Connection) -> Result<(), String> {
    let tools = [
        ("opencode", "opencode"),
        ("codex", "codex"),
        ("claude code", "claude"),
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
            "select id, name, path, created_at, last_opened_at
             from workspaces
             order by coalesce(last_opened_at, created_at) desc",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
                last_opened_at: row.get(4)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
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
        .query_row("select value from settings where key = 'proxy'", [], |row| row.get(0))
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

    Some(format!("http://{}:{}", config.host.trim(), config.port.trim()))
}

fn normalize_path(path: &str) -> Result<String, String> {
    let path = PathBuf::from(path);
    let canonical = path
        .canonicalize()
        .map_err(|_| "请选择一个存在的工作区目录".to_string())?;

    if !canonical.is_dir() {
        return Err("工作区必须是目录".to_string());
    }

    Ok(canonical.to_string_lossy().replace('\\', "/"))
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
        Command::new("where.exe").arg(binary).output().ok()?
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
            home.join(".local").join("share").join("opencode"),
            home.join("AppData").join("Roaming").join("opencode"),
            home.join("AppData").join("Roaming").join("Claude"),
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

fn spawn_terminal(cwd: &str, command: &str, proxy_url: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    {
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        let mut script = String::new();
        if let Some(proxy_url) = proxy_url {
            script.push_str(&format!(
                "$env:HTTP_PROXY='{0}'; $env:HTTPS_PROXY='{0}'; $env:ALL_PROXY='{0}'; ",
                proxy_url
            ));
        }
        script.push_str(command);

        Command::new("powershell.exe")
            .current_dir(cwd)
            .creation_flags(CREATE_NEW_CONSOLE)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = init_database(app.handle())?;
            app.manage(AppState { db: Mutex::new(db) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_ready,
            list_workspaces,
            add_workspace,
            delete_workspace,
            list_tools,
            get_proxy_config,
            save_proxy_config,
            list_launch_history,
            launch_tool,
            list_sessions,
            scan_sessions
        ])
        .run(tauri::generate_context!())
        .expect("error while running CLI Manager");
}
