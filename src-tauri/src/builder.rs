use crate::state::FileEntry;
use hadris_iso::read::PathSeparator;
use hadris_iso::write::options::{CreationFeatures, FormatOptions};
use hadris_iso::write::{File, InputFiles, IsoImageWriter};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub fn build_iso(
    volume_label: &str,
    files: &[FileEntry],
    output_path: &Path,
    _progress_cb: impl Fn(usize, usize, &str),
) -> Result<(), String> {
    eprintln!("[DEBUG builder] ========== 开始构建 ISO ==========");
    eprintln!("[DEBUG builder] volume_label: {:?}", volume_label);
    eprintln!("[DEBUG builder] output_path: {:?}", output_path);
    eprintln!("[DEBUG builder] 文件数量: {}", files.len());

    for (i, f) in files.iter().enumerate() {
        eprintln!("[DEBUG builder] 文件[{}]: name={:?}, path={:?}, iso_path={:?}, size={}",
            i, f.name, f.path, f.iso_path, f.size);
    }

    let label = sanitize_volume_label(volume_label);
    eprintln!("[DEBUG builder] 规范化后的卷标: {:?}", label);

    // 构建目录树（从 iso_path 解析目录结构）
    let mut root_children = Vec::new();
    let mut dirs: BTreeMap<String, Vec<File>> = BTreeMap::new();

    for (idx, entry) in files.iter().enumerate() {
        eprintln!("[DEBUG builder] 读取文件[{}]: {:?}", idx, entry.path);
        let data = std::fs::read(&entry.path)
            .map_err(|e| format!("无法读取文件 {}: {e}", entry.name))?;
        eprintln!("[DEBUG builder]   读取成功, 大小: {} bytes", data.len());

        let f = File::File {
            name: Arc::new(entry.name.clone()),
            contents: data,
        };

        // 解析 iso_path 获取目录部分：/foo/bar/file.txt -> foo/bar
        let path = entry.iso_path.trim_start_matches('/');
        if let Some((dir, _)) = path.rsplit_once('/') {
            eprintln!("[DEBUG builder]   放入目录: {:?}, 文件名: {:?}", dir, entry.name);
            dirs.entry(dir.to_string()).or_default().push(f);
        } else {
            eprintln!("[DEBUG builder]   放入根目录, 文件名: {:?}", entry.name);
            root_children.push(f);
        }
    }

    // 构建嵌套目录结构
    eprintln!("[DEBUG builder] 目录分组数: {}", dirs.len());
    for (dir_path, children) in &dirs {
        eprintln!("[DEBUG builder] 构建目录: {:?}, 含 {} 个文件", dir_path, children.len());
        let parts: Vec<&str> = dir_path.split('/').collect();
        insert_nested(&mut root_children, &parts, children);
    }

    eprintln!("[DEBUG builder] 根目录条目数: {}", root_children.len());
    print_file_tree(&root_children, 0);

    let input_files = InputFiles {
        path_separator: PathSeparator::ForwardSlash,
        files: root_children,
    };

    // 预估 ISO 大小，预分配内存缓冲区
    let estimated = estimate_size(files) as usize;
    eprintln!("[DEBUG builder] 预估 ISO 大小: {} bytes", estimated);

    // 使用 Cursor<Vec<u8>> 作为写入目标（hadris-iso 官方推荐用法）
    // format_new 内部会 seek 回去 read_exact 卷描述符来更新元数据，
    // 必须预分配空间并传 &mut 引用，否则空文件会导致 UnexpectedEof
    let mut buffer = std::io::Cursor::new(vec![0u8; estimated]);

    let format_options = FormatOptions {
        volume_name: label,
        system_id: None,
        volume_set_id: None,
        publisher_id: None,
        preparer_id: None,
        application_id: None,
        sector_size: 2048,
        features: CreationFeatures {
            filenames: hadris_iso::write::options::BaseIsoLevel::Level2 {
                supports_lowercase: true,
                supports_rrip: false,
            },
            long_filenames: true,
            joliet: None,
            rock_ridge: None,
            el_torito: None,
            hybrid_boot: None,
        },
        path_separator: PathSeparator::ForwardSlash,
        strict_charset: false,
    };

    eprintln!("[DEBUG builder] 开始写入 ISO（内存缓冲区）...");
    match IsoImageWriter::format_new(&mut buffer, input_files, format_options) {
        Ok(_) => eprintln!("[DEBUG builder] ISO 写入成功"),
        Err(e) => {
            eprintln!("[DEBUG builder] ISO 写入失败: {:?}", e);
            return Err(format!("ISO 构建失败: {e}"));
        }
    }

    // 将内存中的 ISO 数据写入磁盘
    let iso_data = buffer.into_inner();
    // 裁剪尾部多余的零字节（预分配可能大于实际 ISO 大小）
    let actual_len = iso_data.iter().rposition(|&b| b != 0)
        .map(|p| p + 1)
        .unwrap_or(0);
    // ISO 大小按 2048 字节扇区对齐
    let aligned_len = ((actual_len + 2047) / 2048) * 2048;
    let final_len = aligned_len.max(iso_data.len().min(aligned_len));

    eprintln!("[DEBUG builder] ISO 实际数据大小: {} bytes, 对齐后: {} bytes", actual_len, final_len);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("无法创建输出目录: {e}"))?;
    }

    std::fs::write(output_path, &iso_data[..final_len])
        .map_err(|e| format!("无法写入输出文件: {e}"))?;

    eprintln!("[DEBUG builder] ========== ISO 构建完成 ==========");
    Ok(())
}

/// 将文件按路径嵌套插入目录树
fn insert_nested(root: &mut Vec<File>, parts: &[&str], files: &[File]) {
    if parts.is_empty() {
        root.extend(files.iter().cloned());
        return;
    }

    let dir_name = parts[0];

    // 查找或创建目录
    let dir_idx = root.iter().position(|f| {
        matches!(f, File::Directory { name, .. } if name.as_str() == dir_name)
    });

    if let Some(idx) = dir_idx {
        if let File::Directory { children, .. } = &mut root[idx] {
            insert_nested(children, &parts[1..], files);
        }
    } else {
        let mut children = Vec::new();
        insert_nested(&mut children, &parts[1..], files);
        root.push(File::Directory {
            name: Arc::new(dir_name.to_string()),
            children,
        });
    }
}

/// 调试辅助：打印文件树
fn print_file_tree(files: &[File], depth: usize) {
    let indent = "  ".repeat(depth);
    for f in files {
        match f {
            File::File { name, contents } => {
                eprintln!("[DEBUG tree] {}📄 {} ({} bytes)", indent, name, contents.len());
            }
            File::Directory { name, children } => {
                eprintln!("[DEBUG tree] {}📁 {} ({} children)", indent, name, children.len());
                print_file_tree(children, depth + 1);
            }
        }
    }
}

fn sanitize_volume_label(label: &str) -> String {
    let s: String = label
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '_')
        .take(32)
        .collect();
    if s.is_empty() { "CDROM".to_string() } else { s }
}

pub fn estimate_size(files: &[FileEntry]) -> u64 {
    let overhead: u64 = 2 * 1024 * 1024;
    let data: u64 = files.iter().map(|f| {
        let sectors = (f.size + 2047) / 2048;
        sectors * 2048
    }).sum();
    overhead + data
}
