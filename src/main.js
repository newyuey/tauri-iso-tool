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
  btnClear:   $('#btnClear'),
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
};

// ── 初始化 ────────────────────────────────────────────
async function init() {
  el.btnPickFiles.onclick  = () => pickAndAdd(false);
  el.btnPickFolder.onclick = () => pickAndAdd(true);
  el.btnClear.onclick = clearAll;
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

  // ── 拖拽支持（Tauri 原生拖拽事件）─────────────────────
  const dropZone = document.getElementById('pickZone');

  await listen('tauri://drag-enter', () => {
    dropZone.classList.add('drag-over');
  });

  await listen('tauri://drag-leave', () => {
    dropZone.classList.remove('drag-over');
  });

  await listen('tauri://drag-drop', async (event) => {
    dropZone.classList.remove('drag-over');
    const paths = event.payload?.paths;
    if (!paths || !paths.length) return;

    try {
      toast('正在读取文件...', 'info');
      const added = await invoke('add_files', { paths });
      added.forEach((f) => {
        state.files.push({ ...f, id: uid() });
      });
      render();
      toast(`已添加 ${added.length} 个文件`, 'success');
    } catch (e) {
      toast('添加失败: ' + e, 'error');
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

// ── 清空 ──────────────────────────────────────────────
function clearAll() {
  if (!state.files.length) return;
  state.files = [];
  invoke('clear_files');
  render();
  toast('已清空', 'info');
}

// ── 渲染 ──────────────────────────────────────────────
function render() {
  const n = state.files.length;

  if (n === 0) {
    el.statsInfo.innerHTML = '📭 还没有添加文件';
    el.btnCreate.disabled = true;
    el.estSize.textContent = '—';
    return;
  }

  el.btnCreate.disabled = false;

  const total = state.files.reduce((s, f) => s + (f.size || 0), 0);
  el.statsInfo.innerHTML = `📊 <strong>${n}</strong> 个文件 · <strong>${fmtSize(total)}</strong>`;

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

document.addEventListener('DOMContentLoaded', init);
