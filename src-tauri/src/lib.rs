mod builder;
mod state;
mod templates;

use state::{AppState, BuildRequest, FileEntry, ProgressEvent, Template};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

// ── 对话框 Commands ──────────────────────────────────

#[tauri::command]
async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    let files = app.dialog().file().blocking_pick_files();
    Ok(files
        .unwrap_or_default()
        .iter()
        .filter_map(|f| f.as_path().map(|p| p.to_string_lossy().to_string()))
        .collect())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = app.dialog().file().blocking_pick_folder();
    Ok(dir
        .and_then(|f| f.as_path().map(|p| p.to_string_lossy().to_string()))
        .map(|p| vec![p])
        .unwrap_or_default())
}

#[tauri::command]
async fn pick_save_path(app: AppHandle, default_name: String) -> Result<Option<String>, String> {
    let path = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .blocking_save_file();
    Ok(path.and_then(|p| p.as_path().map(|q| q.to_string_lossy().to_string())))
}

// ── 文件管理 Commands ──────────────────────────────────

#[tauri::command]
fn add_files(paths: Vec<String>, state: State<'_, Mutex<AppState>>) -> Result<Vec<FileEntry>, String> {
    eprintln!("[DEBUG add_files] 收到路径: {:?}", paths);
    let mut app = state.lock().map_err(|e| format!("状态锁定失败: {e}"))?;
    let mut added = Vec::new();

    for raw in &paths {
        let p = std::path::Path::new(raw);
        if !p.exists() {
            eprintln!("[DEBUG add_files] 路径不存在, 跳过: {:?}", raw);
            continue;
        }
        if p.is_file() {
            let name = p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let iso_path = format!("/{}", name.to_uppercase());
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);

            let entry = FileEntry { name, path: raw.clone(), iso_path, size };
            eprintln!("[DEBUG add_files] 添加文件: {:?}", entry);
            app.files.push(entry.clone());
            added.push(entry);
        } else if p.is_dir() {
            eprintln!("[DEBUG add_files] 遍历目录: {:?}", raw);
            for entry in walkdir::WalkDir::new(p)
                .into_iter()
                .filter_entry(|e| !e.file_name().to_str().map_or(false, |n| n.starts_with('.')))
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let full = entry.path().to_string_lossy().to_string();
                let rel = entry.path().strip_prefix(p).unwrap_or(entry.path());
                let iso_path = format!("/{}", rel.to_string_lossy().replace('\\', "/"));
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let name = entry.file_name().to_string_lossy().to_string();
                let fe = FileEntry { name, path: full, iso_path, size };
                eprintln!("[DEBUG add_files]   目录中文件: {:?}", fe);
                app.files.push(fe.clone());
                added.push(fe);
            }
        }
    }

    eprintln!("[DEBUG add_files] 共添加 {} 个文件", added.len());
    Ok(added)
}

#[tauri::command]
fn get_files(state: State<'_, Mutex<AppState>>) -> Result<Vec<FileEntry>, String> {
    let app = state.lock().map_err(|e| format!("状态锁定失败: {e}"))?;
    Ok(app.files.clone())
}

#[tauri::command]
fn remove_file(index: usize, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| format!("状态锁定失败: {e}"))?;
    if index < app.files.len() {
        app.files.remove(index);
        Ok(())
    } else {
        Err("索引越界".to_string())
    }
}

#[tauri::command]
fn clear_files(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| format!("状态锁定失败: {e}"))?;
    app.files.clear();
    Ok(())
}

#[tauri::command]
fn estimate_iso_size(files: Vec<FileEntry>) -> u64 {
    builder::estimate_size(&files)
}

#[tauri::command]
fn get_default_output() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

// ── ISO 构建 ──────────────────────────────────────────

#[tauri::command]
async fn build_iso(
    request: BuildRequest,
    app: AppHandle,
) -> Result<String, String> {
    eprintln!("[DEBUG build_iso] 收到构建请求");
    eprintln!("[DEBUG build_iso] volume_label: {:?}", request.volume_label);
    eprintln!("[DEBUG build_iso] output_path: {:?}", request.output_path);
    eprintln!("[DEBUG build_iso] 文件数: {}", request.files.len());
    for (i, f) in request.files.iter().enumerate() {
        eprintln!("[DEBUG build_iso] 文件[{}]: name={:?}, iso_path={:?}, size={}",
            i, f.name, f.iso_path, f.size);
    }

    let files = request.files.clone();
    let volume_label = request.volume_label.clone();
    let output_path = request.output_path.clone();
    let total = files.len();

    let task_id = uuid::Uuid::new_v4().to_string();

    // 发送初始进度事件
    let _ = app.emit("iso-progress", ProgressEvent {
        status: "working".to_string(),
        current: 0,
        total,
        filename: String::new(),
        message: "正在准备构建...".to_string(),
        output_path: None,
        size: None,
    });
    tokio::task::spawn_blocking(move || {
        let result = builder::build_iso(
            &volume_label,
            &files,
            std::path::Path::new(&output_path),
            |current, _total, filename| {
                let event = ProgressEvent {
                    status: "working".to_string(),
                    current,
                    total,
                    filename: filename.to_string(),
                    message: format!("正在添加 {} ({}/{})", filename, current, total),
                    output_path: None,
                    size: None,
                };
                let _ = app.emit("iso-progress", event);
            },
        );

        match result {
            Ok(()) => {
                let size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                let event = ProgressEvent {
                    status: "done".to_string(),
                    current: total,
                    total,
                    filename: String::new(),
                    message: format!("ISO 创建完成: {}", output_path),
                    output_path: Some(output_path),
                    size: Some(size),
                };
                let _ = app.emit("iso-progress", event);
            }
            Err(e) => {
                let event = ProgressEvent {
                    status: "error".to_string(),
                    current: 0,
                    total,
                    filename: String::new(),
                    message: e,
                    output_path: None,
                    size: None,
                };
                let _ = app.emit("iso-progress", event);
            }
        }
    });

    Ok(task_id)
}

// ── 模板 Commands ─────────────────────────────────────

#[tauri::command]
fn get_templates() -> Vec<Template> {
    templates::load_templates()
}

#[tauri::command]
fn save_template(template: Template) -> Result<(), String> {
    templates::save_template(&template).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_template(name: String) -> Result<bool, String> {
    templates::delete_template(&name).map_err(|e| e.to_string())
}

// ── 启动 ──────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            pick_files,
            pick_folder,
            pick_save_path,
            add_files,
            get_files,
            remove_file,
            clear_files,
            build_iso,
            estimate_iso_size,
            get_default_output,
            get_templates,
            save_template,
            delete_template,
        ])
        .run(tauri::generate_context!())
        .expect("启动 ISO Maker 失败");
}
