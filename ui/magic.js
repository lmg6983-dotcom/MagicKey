// 매직키 창 — 치는 대로 계산 · 앱 · 폴더 · 파일 · 웹 검색 · 웹 AI 줄을 보여 주고, 고른 줄을 실행한다.
//   ↑↓ 고르기 · ← → 할 일 고르기 · Enter 실행 · Ctrl+Shift+C 경로 복사 · Ctrl+Shift+E 폴더 열기 · Esc 지우기 → 닫기
//   아래 캡슐 = 기능, 누르면 켜고 끈다 (이 PC 에 기억) · 오른쪽 위 키 단추 = 여는 단축키 바꾸기
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;

const input = document.getElementById('magic-input');
const clearBtn = document.getElementById('magic-clear');
const keyBtn = document.getElementById('magic-key');
const resultsEl = document.getElementById('magic-results');
const modulesEl = document.getElementById('magic-modules');
const hintEl = document.getElementById('magic-hint');
const box = document.getElementById('magic');

const MODULES = [
  { id: 'apps', chip: '앱', name: '앱 검색: 시작 메뉴 프로그램을 바로 실행' },
  { id: 'files', chip: '파일', name: '파일 검색: PC 의 파일 · 폴더 이름을 찾아 열기 (내용은 읽지 않음)' },
  { id: 'calc', chip: '계산', name: '계산기: 1,200 × 3 · 15% * 80000 · 3주 뒤 · 12/25까지 · 9:15부터 18:30까지' },
  { id: 'web', chip: '웹', name: '웹 검색: 검색 사이트에서 찾거나 사이트 주소를 바로 열기' },
  { id: 'ai', chip: 'AI', name: '웹 AI: ChatGPT · Claude · Gemini · Perplexity 에 묻기' },
];
const store = {
  get: (k) => { try { return JSON.parse(localStorage.getItem(k)); } catch { return null; } },
  set: (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* 기억 못 해도 지금은 된다 */ } },
};
let off = new Set(store.get('magic.off') ?? []); // 캡슐로 꺼 둔 기능
const isOn = (id) => !off.has(id);

let sites = { web: [], ai: [], notice: null, hotkey: '' }; // notice: 단축키를 못 잡았을 때 안내
let recording = false; // 새 단축키를 기다리는 중
let res = null; // 마지막 query 결과
let rows = [];
let selected = 0;
let activeAct = 0;
let seq = 0;

const ICONS = {
  open: 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5',
  copyPath: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1',
  reveal: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  copy: 'M8 8V5a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-3M5 8h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1z',
  web: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3.6 9h16.8M3.6 15h16.8M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z',
  ai: 'M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9zM18.5 15.5l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z',
  date: 'M7 3v3M17 3v3M4 9h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM8 13h3v3H8z',
};
function svg(name) {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('aria-hidden', 'true');
  const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  p.setAttribute('d', ICONS[name] ?? ICONS.open);
  s.append(p);
  return s;
}
const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};

// 주소로 질문을 못 넘기는 사이트(Gemini)는 단추에 미리 알린다 — 브라우저가 열리면 이 창은 숨어 안내를 못 본다
const siteLabel = (s) => (s.paste ? `${s.name} (복사해 열기)` : s.name);

/** 이 줄에서 할 수 있는 일 (첫 번째가 Enter) */
function actionsFor(r) {
  switch (r.kind) {
    case 'app': return [{ id: 'open', label: '실행' }, { id: 'reveal', label: '폴더 열기', key: 'Ctrl+Shift+E' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }];
    case 'folder': return [{ id: 'open', label: '열기' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }, { id: 'reveal', label: '위 폴더에서 보기', key: 'Ctrl+Shift+E' }];
    case 'file': return [{ id: 'open', label: '열기' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }, { id: 'reveal', label: '폴더 열기', key: 'Ctrl+Shift+E' }];
    case 'calc': return [{ id: 'copy', label: '결과 복사' }, { id: 'copyExpr', label: '식까지 복사', text: true }];
    case 'date': return [{ id: 'copy', label: '결과 복사' }, { id: 'copyIso', label: `${r.plain} 복사` }, { id: 'copyExpr', label: '쓴 글까지 복사', text: true }];
    case 'site': return [{ id: 'open', label: '사이트 열기' }];
    case 'web': case 'ai': return r.sites.map((s) => ({ id: s.id, label: siteLabel(s), text: true }));
    default: return [];
  }
}

// ---------- 아래 캡슐 ----------
function renderModules() {
  modulesEl.replaceChildren(...MODULES.map((m) => {
    const chip = el('button', `magic-chip${isOn(m.id) ? ' on' : ''}`);
    chip.type = 'button';
    chip.dataset.mod = m.id;
    chip.title = `${m.name}\n누르면 ${isOn(m.id) ? '꺼요' : '켜요'}`;
    chip.setAttribute('aria-pressed', String(isOn(m.id)));
    chip.append(el('span', 'chip-dot'), m.chip);
    chip.addEventListener('mousedown', (e) => e.preventDefault()); // 입력 칸 포커스 유지
    chip.addEventListener('click', () => {
      if (off.has(m.id)) off.delete(m.id);
      else off.add(m.id);
      store.set('magic.off', [...off]);
      renderModules();
      search();
    });
    return chip;
  }));
}

// ---------- 결과 ----------
// 찾은 낱말을 굵게 (글은 textContent 로만)
function highlighted(text, query) {
  const span = el('span');
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const lower = text.toLowerCase();
  let i = 0;
  while (i < text.length) {
    let hit = null;
    for (const t of terms) {
      const k = lower.indexOf(t, i);
      if (k >= 0 && (!hit || k < hit.k)) hit = { k, len: t.length };
    }
    if (!hit) {
      span.append(text.slice(i));
      break;
    }
    if (hit.k > i) span.append(text.slice(i, hit.k));
    span.append(el('b', '', text.slice(hit.k, hit.k + hit.len)));
    i = hit.k + hit.len;
  }
  return span;
}

function row(r, index, query) {
  const li = el('li', `magic-row kind-${r.kind}`);
  li.setAttribute('role', 'option');
  li.id = `magic-row-${index}`;
  li.dataset.index = String(index);
  li.dataset.mod = r.mod;
  const ico = el('span', 'ico');
  const t = el('div', 't');
  const s = el('div', 's');
  const side = el('span', 'side');
  const txt = el('div', 'txt');
  txt.append(t, s);
  if (r.kind === 'calc') {
    ico.textContent = '=';
    t.textContent = r.value;
    s.textContent = r.detail;
  } else if (r.kind === 'date') {
    ico.append(svg('date'));
    t.textContent = r.value;
    s.textContent = r.detail;
    s.title = r.detail;
  } else if (r.kind === 'site') {
    ico.append(svg('web'));
    t.textContent = '사이트 열기';
    s.textContent = r.url;
  } else if (r.kind === 'web' || r.kind === 'ai') {
    ico.append(svg(r.kind));
    t.textContent = r.kind === 'web' ? '웹에서 찾기' : 'AI에게 묻기';
    s.textContent = r.sites.map(siteLabel).join(', ');
  } else {
    ico.textContent = r.kind === 'folder' ? '📁' : r.kind === 'app' ? '🚀' : '📄';
    t.append(highlighted(r.name, query));
    s.textContent = r.path;
    s.title = r.path;
    if (r.kind === 'app') side.textContent = '프로그램';
  }
  const acts = el('span', 'acts');
  acts.setAttribute('role', 'toolbar');
  actionsFor(r).forEach((a, k) => {
    const b = el('button', `act-btn${a.text ? ' act-text' : ''}`);
    b.type = 'button';
    b.tabIndex = -1;
    b.title = a.key ? `${a.label} (${a.key})` : a.label;
    b.setAttribute('aria-label', a.label);
    if (!a.text) b.append(svg(a.id.startsWith('copy') && a.id !== 'copyPath' ? 'copy' : a.id));
    b.append(el('span', 'act-label', a.label));
    b.addEventListener('mousedown', (e) => e.preventDefault());
    b.addEventListener('click', (e) => {
      e.stopPropagation();
      run(index, k);
    });
    acts.append(b);
  });
  li.append(ico, txt, side, acts);
  li.addEventListener('mousemove', (e) => {
    if ((e.movementX || e.movementY) && selected !== index) select(index); // 목록이 바뀌어 생긴 mousemove 는 무시
  });
  li.addEventListener('mousedown', (e) => e.preventDefault());
  li.addEventListener('click', () => run(index, index === selected ? activeAct : 0));
  return li;
}

function render() {
  const query = input.value.trim();
  clearBtn.hidden = !input.value;
  rows = [];
  const nodes = [];
  const section = (label, count, mod, sub) => {
    const li = el('li', `magic-section${sub ? ' sub' : ''}`, label);
    li.dataset.mod = mod;
    li.setAttribute('role', 'presentation');
    if (count) li.append(el('span', 'count', String(count)));
    nodes.push(li);
  };
  const note = (text, onClick) => {
    const li = el('li', `magic-note${onClick ? ' action' : ''}`, text);
    li.setAttribute('role', 'presentation');
    if (onClick) {
      li.addEventListener('mousedown', (e) => e.preventDefault());
      li.addEventListener('click', onClick);
    }
    nodes.push(li);
  };
  const put = (r) => nodes.push(row(r, rows.push(r) - 1, query));
  keyBtn.textContent = sites.hotkey;
  keyBtn.classList.toggle('on', recording);
  if (recording) note(`새 단축키를 누르세요 (Ctrl · Alt · Win 과 함께, 예: Ctrl+Shift+Space). 지금: ${sites.hotkey} · Esc 취소`);
  else if (query && res) {
    if (res.calc) put({ ...res.calc, mod: 'calc', text: query });
    if (res.site) put({ kind: 'site', mod: 'site', url: res.site });
    const ask = () => isOn('ai') && put({ kind: 'ai', mod: 'ai', sites: sites.ai });
    if (res.question) ask(); // 묻는 글이면 AI 줄을 위로
    if (res.apps.length) {
      section('앱', res.apps.length, 'apps');
      res.apps.forEach((a) => put({ kind: 'app', mod: 'apps', ...a }));
    }
    if (res.folders.length) {
      section('폴더', res.folders.length, 'files');
      res.folders.forEach((f) => put({ kind: 'folder', mod: 'files', ...f }));
    }
    if (res.files.length) {
      section('파일', res.files.length, 'files', res.folders.length > 0);
      res.files.forEach((f) => put({ kind: 'file', mod: 'files', ...f }));
    }
    if (res.more > 0) note(`다른 ${res.more.toLocaleString()}건은 글을 더 써서 좁혀 보세요`);
    const searched = isOn('apps') || isOn('files');
    const found = res.calc || res.site || res.apps.length || res.folders.length || res.files.length;
    if (searched && !res.indexed) note(res.building ? '프로그램 · 파일 목록을 만드는 중이에요. 처음 한 번만, 잠시 뒤 찾아져요' : '프로그램 · 파일 목록이 비어 있어요');
    else if (searched && !found) note(`'${query.slice(0, 30)}'와 맞는 앱 · 파일이 없어요`); // 웹 · AI 줄 위에
    if (isOn('web')) put({ kind: 'web', mod: 'web', sites: sites.web });
    if (!res.question) ask();
    if (!rows.length && !searched) note(`'${query.slice(0, 30)}'와 맞는 결과가 없어요`);
  } else if (sites.notice) note(sites.notice, () => record(true));
  resultsEl.replaceChildren(...nodes);
  select(Math.min(selected, Math.max(0, rows.length - 1)), false);
  fit();
}

function select(i, scroll = true) {
  if (i !== selected) activeAct = 0;
  selected = i;
  for (const li of resultsEl.querySelectorAll('.magic-row')) li.setAttribute('aria-selected', String(Number(li.dataset.index) === i));
  markAct();
  input.setAttribute('aria-activedescendant', rows[i] ? `magic-row-${i}` : '');
  if (scroll) resultsEl.querySelector(`[data-index="${i}"]`)?.scrollIntoView({ block: 'nearest' });
  updateHint();
}

function markAct() {
  resultsEl.querySelector(`.magic-row[data-index="${selected}"]`)?.querySelectorAll('.act-btn').forEach((b, k) => {
    b.classList.toggle('on', k === activeAct);
    b.setAttribute('aria-pressed', String(k === activeAct));
  });
}

let flashUntil = 0;
function updateHint() {
  if (Date.now() < flashUntil) return;
  const r = rows[selected];
  const parts = [];
  if (r) {
    const acts = actionsFor(r);
    parts.push(['↵', acts[activeAct]?.label ?? '']);
    if (acts.length > 1) parts.push(['← →', '할 일']);
  }
  parts.push(['Esc', recording ? '취소' : input.value ? '지우기' : '닫기']);
  if (rows.length > 1) parts.push(['↑↓', '고르기']);
  hintEl.replaceChildren(...parts.map(([k, label]) => {
    const span = el('span');
    span.append(el('kbd', '', k), ` ${label}`);
    return span;
  }));
}

let flashTimer = 0;
function flash(ok, text) {
  hintEl.replaceChildren(el('span', `magic-flash${ok ? '' : ' err'}`, `${ok ? '✓ ' : ''}${text}`));
  flashUntil = Date.now() + 1800;
  clearTimeout(flashTimer);
  flashTimer = setTimeout(updateHint, 1800);
}

// 창 높이를 내용에 맞춘다 (창 그림자 자리 8px 씩)
let winHeight = 0;
function fit() {
  requestAnimationFrame(() => {
    const h = Math.ceil(box.getBoundingClientRect().height) + 16;
    if (h === winHeight) return;
    winHeight = h;
    getCurrentWindow().setSize(new LogicalSize(680, h)).catch(() => {});
  });
}

async function search() {
  pending = false;
  clearTimeout(searchTimer);
  const my = ++seq;
  const text = input.value;
  if (!text.trim()) {
    res = null;
    render();
    return;
  }
  const out = await invoke('query', { text, mods: MODULES.map((m) => m.id).filter(isOn) }).catch(() => null);
  if (my !== seq || !out) return; // 더 새 입력이 있으면 버린다
  res = out;
  render();
}

const hide = () => invoke('hide');

async function run(i, act = 0) {
  const r = rows[i];
  if (!r) return;
  const a = actionsFor(r)[act] ?? actionsFor(r)[0];
  try {
    if (r.kind === 'calc' || r.kind === 'date') {
      const text = a.id === 'copyExpr' ? `${r.text} = ${r.value}` : a.id === 'copyIso' ? r.plain : r.kind === 'date' ? r.value : r.plain;
      flash(true, await invoke('copy', { text }));
      return;
    }
    let msg;
    if (r.kind === 'site') msg = await invoke('open_web', { kind: 'site', site: '', text: r.url });
    else if (r.kind === 'web' || r.kind === 'ai') msg = await invoke('open_web', { kind: r.kind, site: a.id, text: input.value });
    else msg = await invoke('act', { action: a.id, path: r.path });
    if (a.id === 'copyPath') {
      flash(true, msg);
      return;
    }
    if (msg) {
      flash(true, msg);
      setTimeout(hide, 2400); // 안내를 읽을 틈을 두고
    } else hide();
  } catch (err) {
    flash(false, String(err));
  }
}

let searchTimer = 0;
let pending = false; // 친 글을 아직 찾지 않았다 (Enter 가 지난 글의 결과를 실행하지 않게)
input.addEventListener('input', () => {
  selected = 0;
  pending = true;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(search, 60);
  clearBtn.hidden = !input.value;
});

function clearText() {
  input.value = '';
  selected = 0;
  search();
  input.focus();
}
clearBtn.addEventListener('mousedown', (e) => e.preventDefault());
clearBtn.addEventListener('click', clearText);

// ---------- 단축키 바꾸기 ----------
function record(on) {
  recording = on;
  render();
  input.focus();
}
keyBtn.addEventListener('mousedown', (e) => e.preventDefault());
keyBtn.addEventListener('click', () => record(!recording));

const MOD_KEYS = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;
async function takeKey(e) {
  e.preventDefault();
  if (e.key === 'Escape') return record(false);
  if (MOD_KEYS.test(e.code) || !e.code) return; // 조합키만 누른 동안은 기다린다
  const mods = [e.ctrlKey && 'ctrl', e.altKey && 'alt', e.shiftKey && 'shift', e.metaKey && 'super'].filter(Boolean);
  try {
    sites.hotkey = await invoke('set_hotkey', { key: [...mods, e.code].join('+') });
    sites.notice = null;
    record(false);
    flash(true, `이제 ${sites.hotkey} 로 열어요`);
  } catch (err) {
    flash(false, String(err));
  }
}

input.addEventListener('keydown', (e) => {
  if (recording) return takeKey(e);
  if (e.isComposing) return; // 한글 조합 중
  const r = rows[selected];
  const acts = r ? actionsFor(r) : [];
  if (e.key === 'Escape') {
    e.preventDefault();
    if (input.value) clearText();
    else hide();
  } else if (e.ctrlKey && e.shiftKey && /^[ce]$/i.test(e.key)) {
    const id = e.key.toLowerCase() === 'c' ? 'copyPath' : 'reveal';
    const k = acts.findIndex((a) => a.id === id);
    if (k >= 0) {
      e.preventDefault();
      run(selected, k);
    }
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    if (!rows.length) return;
    e.preventDefault();
    select((selected + (e.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length);
  } else if (e.key === 'ArrowRight' && acts.length > 1 && (activeAct > 0 || input.selectionStart === input.value.length)) {
    e.preventDefault();
    activeAct = Math.min(acts.length - 1, activeAct + 1);
    markAct();
    updateHint();
  } else if (e.key === 'ArrowLeft' && activeAct > 0) {
    e.preventDefault();
    activeAct--;
    markAct();
    updateHint();
  } else if (e.key === 'Enter') {
    e.preventDefault();
    (pending ? search() : Promise.resolve()).then(() => rows.length && run(selected, activeAct));
  }
});

// 창이 떠오를 때마다: 지난 글은 남겨 두고 통째로 골라 둔다 (이어서 찾으려면 그대로, 새로 찾으려면 바로 치면 바뀐다)
function onShow() {
  recording = false;
  flashUntil = 0;
  selected = 0;
  activeAct = 0;
  search();
  input.focus();
  input.select();
}

(async () => {
  sites = await invoke('init').catch(() => sites);
  renderModules();
  render();
  await listen('magic:show', onShow);
  await listen('magic:hotkey', () => record(true)); // 알림 영역 메뉴의 '단축키 바꾸기'
})();
