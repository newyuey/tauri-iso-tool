use crate::state::Template;
use std::fs;
use std::path::PathBuf;

/// 模板文件路径
fn templates_path() -> PathBuf {
    let home = dirs_home().unwrap_or_else(|| PathBuf::from("."));
    home.join(".iso_maker_templates.json")
}

/// 获取用户主目录
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// 加载所有模板
pub fn load_templates() -> Vec<Template> {
    let path = templates_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// 保存模板（按名称 upsert）
pub fn save_template(template: &Template) -> std::io::Result<()> {
    let mut templates = load_templates();
    if let Some(existing) = templates.iter_mut().find(|t| t.name == template.name) {
        *existing = template.clone();
    } else {
        templates.push(template.clone());
    }
    let content = serde_json::to_string_pretty(&templates)?;
    fs::write(templates_path(), content)?;
    Ok(())
}

/// 按名称删除模板，返回是否真正删除了
pub fn delete_template(name: &str) -> std::io::Result<bool> {
    let mut templates = load_templates();
    let len_before = templates.len();
    templates.retain(|t| t.name != name);
    if templates.len() == len_before {
        return Ok(false);
    }
    let content = serde_json::to_string_pretty(&templates)?;
    fs::write(templates_path(), content)?;
    Ok(true)
}
