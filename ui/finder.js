// 모두 찾기 창 — 매직키 창에서 "다른 n건 더 보기" 를 고르면 열린다 (메모킹 Electron 매직키의 창과 같은 것, main.rs open_all)
//   기본은 비슷한 것부터, 열 머리를 누르면 이름 · 경로 · 크기 · 고친 때로 (한 번 더 누르면 거꾸로, 세 번째는 다시 비슷한 순)
//   ↑↓ PgUp PgDn Ctrl+Home/End 고르기 · ← → 할 일(열기 · 경로 복사 · 폴더 열기) · Enter 실행 · Ctrl+Enter 폴더 열기 · 두 번 누르면 열기
//   많아도 가볍게: 보이는 줄만 그린다. 크기 · 고친 때는 보이는 줄만 물어 온다
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const input = document.getElementById('finder-input');
const tabsEl = document.getElementById('finder-tabs');
const head = document.querySelector('.finder-head');
const scroller = document.getElementById('finder-scroll');
const listEl = document.getElementById('finder-list');
const emptyEl = document.getElementById('finder-empty');
const countEl = document.getElementById('finder-count');
const statusEl = document.getElementById('finder-status');

const ROW_H = 34;
const MOD = { memo: 'notes', app: 'apps', folder: 'files', file: 'files' }; // 기능 색 (magic.css [data-mod])

let mods = [];
let all = []; // 찾은 것 (비슷한 순)
let total = 0; // 맞는 것 전체 (많으면 비슷한 것부터 일부만 받는다)
let kind = 'all';
let sort = null; // { key, dir } — null 이면 비슷한 순
let view = []; // 지금 보이는 순서
let selected = 0;
let activeAct = 0;
let query = '';
let seq = 0;
let loading = false;
const stats = new Map(); // path → { size, mtime }
const asked = new Set();

// ---------- 글 ----------
function sizeLabel(n) {
  if (n == null) return '';
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v).toLocaleString()} ${units[u]}`;
}

const pad = (n) => String(n).padStart(2, '0');
function timeLabel(ms) {
  if (!ms) return '';
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const nameOf = (r) => (r.kind === 'memo' ? r.title || '제목 없음' : r.name);
const whereOf = (r) => (r.kind === 'memo' ? '메모킹 메모' : r.path);
const timeOf = (r) => (r.kind === 'memo' ? Date.parse(r.updated ?? '') || 0 : stats.get(r.path)?.mtime ?? 0);
const sizeOf = (r) => (r.kind === 'file' ? stats.get(r.path)?.size ?? -1 : -1);

// ---------- 목록 ----------
function rebuild({ keep = false } = {}) {
  const key = view[selected];
  view = all.filter((r) => kind === 'all' || r.kind === kind);
  if (sort) {
    const by = { name: (r) => nameOf(r).toLowerCase(), where: (r) => whereOf(r).toLowerCase(), size: sizeOf, time: timeOf }[sort.key];
    const coll = new Intl.Collator('ko', { numeric: true });
    view = view.slice().sort((a, b) => {
      const x = by(a);
      const y = by(b);
      return sort.dir * (typeof x === 'string' ? coll.compare(x, y) : x - y);
    });
  }
  selected = keep && key ? Math.max(0, view.indexOf(key)) : 0;
  activeAct = keep ? activeAct : 0;
  listEl.style.height = `${view.length * ROW_H}px`;
  for (const b of head.querySelectorAll('[data-sort]')) {
    const on = sort?.key === b.dataset.sort;
    b.classList.toggle('sorted', on);
    b.dataset.dir = on ? (sort.dir > 0 ? 'asc' : 'desc') : '';
    b.setAttribute('aria-sort', on ? (sort.dir > 0 ? 'ascending' : 'descending') : 'none');
  }
  const counts = { all: all.length, memo: 0, app: 0, folder: 0, file: 0 };
  for (const r of all) counts[r.kind]++;
  for (const t of tabsEl.querySelectorAll('[data-kind]')) {
    t.querySelector('.n').textContent = counts[t.dataset.kind] ? counts[t.dataset.kind].toLocaleString() : '';
    t.setAttribute('aria-selected', String(t.dataset.kind === kind));
    t.hidden = t.dataset.kind !== 'all' && !counts[t.dataset.kind] && t.dataset.kind !== kind; // 없는 종류 탭은 숨긴다
  }
  countEl.textContent = !query.trim() ? '' : total > all.length ? `${total.toLocaleString()}건 중 비슷한 ${all.length.toLocaleString()}건` : `${all.length.toLocaleString()}건`;
  emptyEl.hidden = Boolean(view.length) || !query.trim();
  if (loading && !view.length) emptyEl.replaceChildren(searching('찾는 중'));
  else if (!view.length) emptyEl.textContent = `'${query.trim().slice(0, 40)}'와 맞는 것이 없어요`;
  draw();
  if (!keep) scroller.scrollTop = 0;
  status();
}

function draw() {
  const top = scroller.scrollTop;
  const h = scroller.clientHeight;
  const from = Math.max(0, Math.floor(top / ROW_H) - 6);
  const to = Math.min(view.length, Math.ceil((top + h) / ROW_H) + 6);
  const nodes = [];
  const need = [];
  for (let i = from; i < to; i++) {
    nodes.push(rowEl(view[i], i));
    if (view[i].kind !== 'memo' && !asked.has(view[i].path)) need.push(view[i].path);
  }
  listEl.replaceChildren(...nodes);
  if (need.length) loadStats(need);
}

function rowEl(r, i) {
  const row = el('div', `frow kind-${r.kind}`);
  row.setAttribute('role', 'option');
  row.id = `frow-${i}`;
  row.dataset.mod = MOD[r.kind];
  row.style.transform = `translateY(${i * ROW_H}px)`;
  row.setAttribute('aria-selected', String(i === selected));
  const name = el('span', 'col-name');
  const ico = el('span', 'ico');
  if (r.kind === 'app' || r.kind === 'file' || r.kind === 'folder') realIcon(ico, r.kind, r.path); // 매직키 창과 같은 실제 아이콘
  else ico.append(svg(r.kind === 'memo' && r.locked ? 'lock' : r.kind));
  const label = el('span', 'nm');
  label.append(highlighted(nameOf(r), query));
  name.append(ico, label);
  if (i === selected) {
    const acts = el('span', 'acts');
    actionsFor(r).forEach((a, k) => {
      const b = el('button', `act-btn${k === activeAct ? ' on' : ''}`);
      b.type = 'button';
      b.tabIndex = -1;
      b.dataset.action = a.id;
      b.title = a.key ? `${a.label} (${a.key})` : a.label;
      b.setAttribute('aria-label', a.label);
      b.append(svg(a.id === 'openMemo' ? 'open' : a.id), el('span', 'act-label', a.label));
      b.addEventListener('mousedown', (e) => e.preventDefault());
      b.addEventListener('click', () => run(i, k));
      acts.append(b);
    });
    name.append(acts);
  }
  const where = el('span', 'col-where', whereOf(r));
  where.title = r.path ?? '';
  const size = el('span', 'col-size', r.kind === 'file' ? sizeLabel(stats.get(r.path)?.size) : '');
  const time = el('span', 'col-time', timeLabel(timeOf(r)));
  row.append(name, where, size, time);
  row.addEventListener('mousedown', (e) => {
    if (e.target.closest('.act-btn')) return;
    e.preventDefault();
    select(i, { scroll: false });
  });
  if (r.kind === 'file' || r.kind === 'folder') dragOut(row, r.path);
  row.addEventListener('dblclick', (e) => {
    if (!e.target.closest('.act-btn') && !row.dataset.dragged) run(i, 0);
  });
  return row;
}

let statTimer = 0;
const pending = new Set();
function loadStats(paths) {
  for (const p of paths) {
    asked.add(p);
    pending.add(p);
  }
  clearTimeout(statTimer);
  statTimer = setTimeout(async () => {
    const list = [...pending];
    pending.clear();
    const my = seq;
    for (let k = 0; k < list.length; k += 400) {
      const res = await invoke('file_stats', { paths: list.slice(k, k + 400) }).catch(() => ({}));
      if (my !== seq) return;
      for (const [p, st] of Object.entries(res)) stats.set(p, st);
    }
    if (sort && (sort.key === 'size' || sort.key === 'time')) rebuild({ keep: true });
    else draw();
  }, 40);
}

function select(i, { scroll = true } = {}) {
  if (!view.length) return;
  i = Math.max(0, Math.min(view.length - 1, i));
  if (i !== selected) activeAct = 0;
  selected = i;
  if (scroll) {
    const y = i * ROW_H;
    if (y < scroller.scrollTop) scroller.scrollTop = y;
    else if (y + ROW_H > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = y + ROW_H - scroller.clientHeight;
  }
  input.setAttribute('aria-activedescendant', `frow-${i}`);
  draw();
  status();
}

function status(text, ok = true) {
  statusEl.classList.toggle('err', !ok);
  statusEl.classList.toggle('flash', Boolean(text));
  if (text) {
    statusEl.textContent = text;
    return;
  }
  const r = view[selected];
  const acts = r ? actionsFor(r) : [];
  statusEl.textContent = r ? `${r.path ?? nameOf(r)}    ↵ ${acts[activeAct]?.label ?? ''}${acts.length > 1 ? '    ← → 할 일' : ''}` : '';
}

let flashTimer = 0;
async function run(i, act = activeAct) {
  const r = view[i];
  if (!r) return;
  const a = actionsFor(r)[act] ?? actionsFor(r)[0];
  try {
    const msg = r.kind === 'memo' ? await invoke(a.id === 'copyMemo' ? 'copy_memo' : 'open_memo', { id: r.id }) : await invoke('act', { action: a.id, path: r.path });
    if (a.id === 'copyPath' || a.id === 'copyMemo' || a.id === 'copyFile') markDone(document.getElementById(`frow-${i}`)?.querySelector(`.act-btn[data-action="${a.id}"]`));
    if (msg) {
      status(`✓ ${msg}`);
      clearTimeout(flashTimer);
      flashTimer = setTimeout(() => status(), 1800);
    }
  } catch (err) {
    status(String(err), false);
  }
}

// ---------- 찾기 ----------
async function search() {
  query = input.value;
  const my = ++seq;
  loading = true;
  document.body.classList.add('searching');
  if (!view.length && query.trim()) {
    emptyEl.hidden = false;
    emptyEl.replaceChildren(searching('찾는 중'));
  }
  const res = await invoke('search_all', { text: query, mods }).catch(() => null);
  if (my !== seq) return;
  loading = false;
  document.body.classList.remove('searching');
  stats.clear();
  asked.clear();
  all = res ? [
    ...res.memos.map((m) => ({ kind: 'memo', ...m })),
    ...res.found.apps.map((a) => ({ kind: 'app', ...a })),
    ...res.found.folders.map((f) => ({ kind: 'folder', ...f })),
    ...res.found.files.map((f) => ({ kind: 'file', ...f })),
  ] : [];
  total = res?.total ?? 0;
  rebuild();
  if (!res) emptyEl.textContent = '찾지 못했어요. 다시 써 보세요';
  else if (!all.length && query.trim() && !res.indexed) emptyEl.textContent = res.building ? '프로그램 · 파일 목록을 만드는 중이에요. 잠시 뒤 다시 찾아 보세요' : '프로그램 · 파일 목록이 비어 있어요';
}

let timer = 0;
input.addEventListener('input', () => {
  clearTimeout(timer);
  timer = setTimeout(search, 90);
});

input.addEventListener('keydown', (e) => {
  if (e.isComposing) return;
  const r = view[selected];
  const acts = r ? actionsFor(r) : [];
  if (r && e.ctrlKey && e.shiftKey && /^[ce]$/i.test(e.key)) {
    const k = acts.findIndex((a) => a.id === (e.key.toLowerCase() === 'c' ? 'copyPath' : 'reveal'));
    if (k >= 0) {
      e.preventDefault();
      run(selected, k);
    }
    return;
  }
  if (r && acts.length > 1 && ((e.key === 'ArrowRight' && (activeAct > 0 || input.selectionStart === input.value.length)) || (e.key === 'ArrowLeft' && activeAct > 0))) {
    e.preventDefault();
    activeAct = Math.max(0, Math.min(acts.length - 1, activeAct + (e.key === 'ArrowRight' ? 1 : -1)));
    draw();
    status();
    return;
  }
  const page = Math.max(1, Math.floor(scroller.clientHeight / ROW_H) - 1);
  const move = { ArrowDown: 1, ArrowUp: -1, PageDown: page, PageUp: -page }[e.key];
  if (move && view.length) {
    e.preventDefault();
    select(selected + move);
  } else if ((e.key === 'Home' || e.key === 'End') && e.ctrlKey && view.length) {
    e.preventDefault();
    select(e.key === 'Home' ? 0 : view.length - 1);
  } else if (e.key === 'Enter' && r) {
    e.preventDefault();
    const reveal = acts.findIndex((a) => a.id === 'reveal');
    run(selected, e.ctrlKey && reveal >= 0 ? reveal : activeAct);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    if (input.value) {
      input.value = '';
      search();
    } else window.__TAURI__.window.getCurrentWindow().close();
  }
});

tabsEl.addEventListener('click', (e) => {
  const t = e.target.closest('[data-kind]');
  if (!t) return;
  kind = t.dataset.kind;
  rebuild();
  input.focus();
});
// 열 머리: 오름 → 내림 → 비슷한 순 (크기 · 고친 때는 큰 것 · 새것부터)
head.addEventListener('click', (e) => {
  const b = e.target.closest('[data-sort]');
  if (!b) return;
  const key = b.dataset.sort;
  const first = key === 'size' || key === 'time' ? -1 : 1;
  sort = sort?.key !== key ? { key, dir: first } : sort.dir === first ? { key, dir: -first } : null;
  // 크기 · 고친 때로 줄 세우려면 모든 줄의 것이 있어야 한다
  if (sort && (key === 'size' || key === 'time')) loadStats(all.filter((r) => r.kind !== 'memo' && !asked.has(r.path)).map((r) => r.path));
  rebuild({ keep: true });
  input.focus();
});
scroller.addEventListener('scroll', () => requestAnimationFrame(draw), { passive: true });
new ResizeObserver(() => draw()).observe(scroller);

function start(q) {
  mods = q.mods ?? [];
  input.value = q.text ?? '';
  kind = 'all';
  sort = null;
  search();
  input.focus();
  input.setSelectionRange(input.value.length, input.value.length); // 커서는 끝 — → 로 바로 할 일을 고를 수 있게
}
listen('finder:query', (e) => start(e.payload));
invoke('finder_query').then(start, () => {});
