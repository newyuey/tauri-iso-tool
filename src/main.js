// ISO Maker — Tauri 版前端
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── 状态 ──────────────────────────────────────────────
const state = { files: [] };
let _id = 0;
const uid = () => `f${Date.now()}_${_id++}`;

// ── DOM 引用 ──────────────────────────────────────────
const $ = (s) => document.querySelector(s);
const el = {
  btnPickFiles:  $('#btnPickFiles'),
  btnPickFolder: $('#btnPickFolder'),
  statsInfo:  $('#statsInfo'),
  volumeLabel:    $('#volumeLabel'),
  outputFilename: $('#outputFilename'),
  estSize:    $('#estSize'),
  btnCreate:  $('#btnCreate'),
  progressBox: $('#progressBox'),
  progLabel:   $('#progLabel'),
  progPct:     $('#progPct'),
  progFill:    $('#progFill'),
  progStatus:  $('#progStatus'),
  resultBox:  $('#resultBox'),
  resultInfo: $('#resultInfo'),
  toastBox: $('#toastBox'),
  dropPlaceholder: $('#dropPlaceholder'),
  fileTree: $('#fileTree'),
};

// ── 初始化 ────────────────────────────────────────────
async function init() {
  el.btnPickFiles.onclick  = () => pickAndAdd(false);
  el.btnPickFolder.onclick = () => pickAndAdd(true);
  el.btnCreate.onclick = buildIso;

  el.volumeLabel.oninput = () => {
    const label = el.volumeLabel.value.trim() || 'CDROM';
    el.outputFilename.value = label + '.iso';
  };

  await listen('iso-progress', (ev) => {
    const d = ev.payload;
    if (d.status === 'working') {
      setProgress(d.current, d.total, d.message || d.filename);
    } else if (d.status === 'done') {
      setProgress(d.total, d.total, '完成！');
      setTimeout(() => {
        el.progressBox.classList.remove('active');
        showResult(d);
        toast('ISO 制作完成！', 'success');
      }, 500);
    } else if (d.status === 'error') {
      el.progressBox.classList.remove('active');
      toast('失败: ' + d.message, 'error');
    }
  });

  render();
}

// ── 文件选择（通过 Rust command 打开对话框）────────────
async function pickAndAdd(folder) {
  try {
    const paths = folder
      ? await invoke('pick_folder')
      : await invoke('pick_files');

    if (!paths || !paths.length) return;

    toast('正在读取文件...', 'info');
    state.files = [];
    await invoke('clear_files');
    const added = await invoke('add_files', { paths });
    added.forEach((f) => {
      state.files.push({ ...f, id: uid() });
    });
    render();
    toast(`已添加 ${added.length} 个文件`, 'success');
  } catch (e) {
    toast('添加失败: ' + e, 'error');
  }
}

// ── 构建 ISO ──────────────────────────────────────────
async function buildIso() {
  if (!state.files.length) return;

  const volumeLabel = el.volumeLabel.value.trim() || 'CDROM';
  const defaultName = volumeLabel + '.iso';

  // 通过 Rust 打开保存对话框
  const outputPath = await invoke('pick_save_path', { defaultName });
  if (!outputPath) return;

  el.resultBox.classList.remove('active');
  el.progressBox.classList.add('active');
  setProgress(0, state.files.length, '准备中...');

  try {
    await invoke('build_iso', {
      request: {
        volume_label: volumeLabel,
        files: state.files.map(f => ({
          name: f.name, path: f.path,
          iso_path: f.isoPath || f.iso_path, size: f.size,
        })),
        output_path: outputPath,
      },
    });
  } catch (e) {
    toast('ISO 制作失败: ' + e, 'error');
    el.progressBox.classList.remove('active');
  }
}

function render() {
  const n = state.files.length;

  if (n === 0) {
    el.statsInfo.innerHTML = '📭 还没有添加文件';
    el.btnCreate.disabled = true;
    el.estSize.textContent = '—';
    el.dropPlaceholder.style.display = '';
    el.fileTree.style.display = 'none';
    el.fileTree.innerHTML = '';
    return;
  }

  el.btnCreate.disabled = false;

  const total = state.files.reduce((s, f) => s + (f.size || 0), 0);
  el.statsInfo.innerHTML = `📊 <strong>${n}</strong> 个文件 · <strong>${fmtSize(total)}</strong>`;

  el.dropPlaceholder.style.display = 'none';
  el.fileTree.style.display = '';
  renderFileTree(state.files);

  invoke('estimate_iso_size', { files: state.files.map(f => ({ name: f.name, path: f.path, iso_path: f.isoPath || f.iso_path, size: f.size })) })
    .then(sz => { el.estSize.textContent = fmtSize(sz); })
    .catch(() => { el.estSize.textContent = '—'; });
}

// ── 进度 ──────────────────────────────────────────────
function setProgress(cur, total, msg) {
  const pct = total > 0 ? Math.round((cur / total) * 100) : 0;
  el.progPct.textContent = pct + '%';
  el.progFill.style.width = pct + '%';
  el.progStatus.textContent = msg || '';
  el.progLabel.textContent = `正在制作... (${cur}/${total})`;
}

// ── 结果 ──────────────────────────────────────────────
function showResult(data) {
  el.resultBox.classList.add('active');
  el.resultInfo.innerHTML = `
    <div>💾 大小: <strong>${fmtSize(data.size || 0)}</strong></div>
    <div style="color:var(--text-muted);margin-top:2px">ISO 9660 · 兼容 Mac / Windows / Linux</div>
  `;
}

// ── Toast ─────────────────────────────────────────────
function toast(msg, type = 'info') {
  const icons = { success: '✅', error: '❌', info: 'ℹ️' };
  const t = document.createElement('div');
  t.className = `toast ${type}`;
  t.innerHTML = `<span>${icons[type] || 'ℹ️'}</span><span>${esc(msg)}</span>`;
  el.toastBox.appendChild(t);
  setTimeout(() => { t.classList.add('out'); t.onanimationend = () => t.remove(); }, 3000);
}

// ── 工具函数 ──────────────────────────────────────────
function fmtSize(b) {
  if (!b) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(b) / Math.log(1024));
  return (b / 1024 ** i).toFixed(i > 0 ? 1 : 0) + ' ' + u[i];
}

function esc(s) {
  const d = document.createElement('div');
  d.textContent = s || '';
  return d.innerHTML;
}


// ── 文件树渲染 ───────────────────────────────────────
function renderFileTree(files) {
  const tree = {};
  for (const f of files) {
    const ip = (f.isoPath || f.iso_path || '');
    const dir = ip.substring(0, ip.lastIndexOf('/') + 1) || '/';
    if (!tree[dir]) tree[dir] = [];
    tree[dir].push(f);
  }
  const dirs = Object.keys(tree).sort();
  let html = '';
  for (const dir of dirs) {
    html += `<div class="ft-dir"><span class="ft-dir-icon">📁</span>${esc(dir === '/' ? '/' : dir)}</div>`;
    for (const f of tree[dir]) {
      html += `<div class="ft-file"><span class="ft-file-name">${esc(f.name)}</span><span class="ft-file-size">${fmtSize(f.size)}</span></div>`;
    }
  }
  el.fileTree.innerHTML = html;
}

document.addEventListener('DOMContentLoaded', init);
