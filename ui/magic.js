// 매직키 창 — 치는 대로 계산 · 메모킹 메모 · 앱 · 폴더 · 파일 · 웹 검색 · 웹 AI 줄을 보여 주고, 고른 줄을 실행한다.
//   ↑↓ 고르기 · ← → 할 일 고르기 · Enter 실행 · Shift+Enter 줄 바꿈 (여러 줄 메모: 첫 줄이 제목) · Ctrl+Enter 다른 실행(새 메모: 저장하고 열기) · Ctrl+Shift+C 경로 복사 · Ctrl+Shift+E 폴더 열기 · Esc 지우기 → 닫기
//   아래 캡슐 = 기능, 누르면 켜고 끈다 · 끌면 순서를 바꾼다 (결과 묶음도 그 순서, 이 PC 에 기억) · 단축키 · 기능 설정은 알림 영역 ✦ 오른쪽 클릭 메뉴에도
//   손잡이 · ✦ 를 끌면 창을 옮긴다
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow, LogicalSize, PhysicalPosition } = window.__TAURI__.window;
const win = getCurrentWindow();

const input = document.getElementById('magic-input');
const clearBtn = document.getElementById('magic-clear');
const resultsEl = document.getElementById('magic-results');
const modulesEl = document.getElementById('magic-modules');
const hintEl = document.getElementById('magic-hint');
const box = document.getElementById('magic');

// 기본 순서 = 결과 순서 (계산은 메모 · 파일보다 위). 캡슐을 끌어 바꾼 순서는 magic.order 에
const MODULES = [
  { id: 'memo', chip: '새 메모', name: '새 메모: 쓴 글을 메모킹 새 메모로 저장 (Enter 저장, Ctrl+Enter 저장하고 열기)', memo: true },
  { id: 'opms', chip: 'OPMS', name: 'OPMS 검색: 사건번호 · 출원인 · 발명자 · 초성을 쓰면 MagicSystem 으로 찾기 (사내망)', opms: true, defaultOff: true },
  { id: 'calc', chip: '계산', name: '계산기: 1,200 × 3 · 15% * 80000 · 3주 뒤 · 12/25까지 · 9:15부터 18:30까지' },
  { id: 'notes', chip: '메모', name: '메모 검색: 메모킹 메모의 제목 · 내용에서 찾아 메모킹으로 열기 (메모 폴더는 읽기만)', memo: true },
  { id: 'apps', chip: '앱', name: '앱 검색: 시작 메뉴 프로그램을 바로 실행' },
  { id: 'files', chip: '파일', name: '파일 검색: PC 의 파일 · 폴더 이름을 찾아 열기 (내용은 읽지 않음)' },
  { id: 'web', chip: '웹', name: '웹 검색: 검색 사이트에서 찾거나 사이트 주소를 바로 열기' },
  { id: 'ai', chip: 'AI', name: '웹 AI: ChatGPT · Claude · Gemini · Perplexity 에 묻기' },
];
const store = {
  get: (k) => { try { return JSON.parse(localStorage.getItem(k)); } catch { return null; } },
  set: (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* 기억 못 해도 지금은 된다 */ } },
};
let off = new Set(store.get('magic.off') ?? []); // 캡슐로 꺼 둔 기능 (아직 만지지 않았으면 메모킹 매직키 설정을 따른다 — init 뒤)
let optIn = new Set(store.get('magic.on') ?? []); // 기본값이 꺼짐인 기능(OPMS) 중 켠 것
const DEFAULT_OFF = new Set(MODULES.filter((m) => m.defaultOff).map((m) => m.id));
const isOn = (id) => (DEFAULT_OFF.has(id) ? optIn.has(id) : !off.has(id));
const offList = () => MODULES.map((m) => m.id).filter((id) => !isOn(id));
let order = store.get('magic.order') ?? []; // 캡슐을 끌어 바꾼 순서 (없는 기능은 기본 자리 뒤에)
const rank = (id) => (order.includes(id) ? order.indexOf(id) : 100 + MODULES.findIndex((m) => m.id === id));
// 메모킹이 없으면 '메모' 캡슐, MagicSystem 이 없으면 'OPMS' 캡슐은 없다
const shownModules = () => MODULES.filter((m) => (!m.memo || sites.memo) && (!m.opms || sites.opms)).sort((a, b) => rank(a.id) - rank(b.id));

// 움직임 — 움직임 줄이기 설정이면 바로 그 자리에
const calm = matchMedia('(prefers-reduced-motion: reduce)');
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'; // 빨리 출발해 부드럽게 멈춤
const SPRING = 'cubic-bezier(0.34, 1.4, 0.64, 1)'; // 살짝 지나쳤다 돌아옴 (놓을 때 · 켤 때)
// el 을 dx, dy 만큼 떨어진 곳에서 제자리로
function slide(node, dx, dy, easing = EASE) {
  if (calm.matches || (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5)) return;
  node.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: 260, easing });
}

let sites = { web: [], ai: [], notice: null, hotkey: '', memo: false, opms: false }; // notice: 단축키를 못 잡았을 때 안내
let res = null; // 마지막 query 결과
// OPMS 저절로 찾기 (사건번호를 온전히 썼거나 찾아 본 번호와 같은 꼴) — 치기를 잠깐 멈추면 한 번만 묻는다
let auto = { query: '', cases: [], total: 0, error: '', loading: false };
let autoTimer = 0;
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
  check: 'M5 12.5l4.5 4.5L19 7.5',
  // 결과 줄 아이콘 — 이모지 대신 같은 선 그림 (기능 색 칸 안에)
  memo: 'M6 3h9l4 4v14H6zM14 3v5h5M9 12h7M9 16h5',
  lock: 'M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 7 0v3',
  file: 'M7 3h7l5 5v13H7zM14 3v5h5',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  app: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z',
  opms: 'M4 21V5l8-2v18M12 8h8v13M7.5 8h1M7.5 12h1M7.5 16h1M15.5 12h1M15.5 16h1M3 21h18',
  new: 'M12 5v14M5 12h14',
  remind: 'M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15zM10 20a2 2 0 0 0 4 0',
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

// 찾는 중 — 돋보기가 이리저리 살피고 점 세 개가 차례로 (메모킹 매직키와 같은 것)
function searching(text) {
  const box = el('span', 'mk-searching');
  box.setAttribute('role', 'status');
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('class', 'mk-glass');
  s.setAttribute('aria-hidden', 'true');
  for (const [tag, attrs] of [['circle', { cx: 10.5, cy: 10.5, r: 6 }], ['path', { d: 'M7.6 9.2a3.2 3.2 0 012.4-2.3', class: 'mk-glass-shine' }], ['path', { d: 'M15 15l5 5', class: 'mk-glass-handle' }]]) {
    const n = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v));
    s.append(n);
  }
  const dots = el('span', 'mk-dots');
  dots.setAttribute('aria-hidden', 'true');
  for (let i = 0; i < 3; i++) dots.append(el('i', '', '.'));
  const label = el('span', '', text);
  label.append(dots);
  box.append(s, label);
  return box;
}

// 복사한 단추는 잠깐 초록 체크로
function markDone(btn) {
  const path = btn?.querySelector('svg path');
  if (!btn || btn.classList.contains('done')) return;
  const prev = path?.getAttribute('d');
  btn.classList.add('done');
  path?.setAttribute('d', ICONS.check);
  setTimeout(() => {
    btn.classList.remove('done');
    if (prev) path.setAttribute('d', prev);
  }, 1100);
}

// 주소로 질문을 못 넘기는 사이트(Gemini)는 단추에 미리 알린다 — 브라우저가 열리면 이 창은 숨어 안내를 못 본다
const siteLabel = (s) => (s.paste ? `${s.name} (복사해 열기)` : s.name);

/** 이 줄에서 할 수 있는 일 (첫 번째가 Enter) */
function actionsFor(r) {
  switch (r.kind) {
    case 'new': case 'remind': return [{ id: 'save', label: '저장', text: true }, { id: 'saveOpen', label: '저장하고 열기', key: 'Ctrl+Enter', text: true }];
    case 'opms': return [{ id: 'open', label: 'MagicSystem 에서 찾기' }];
    case 'case': return [{ id: 'open', label: 'MagicSystem 에서 열기' }, { id: 'copyRef', label: '관리번호 복사', text: true }];
    case 'memo': return [{ id: 'openMemo', label: '메모킹에서 열기' }];
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
  modulesEl.replaceChildren(...shownModules().map((m) => {
    const chip = el('button', `magic-chip${isOn(m.id) ? ' on' : ''}`);
    chip.type = 'button';
    chip.dataset.mod = m.id;
    chip.title = `${m.name}\n누르면 ${isOn(m.id) ? '꺼요' : '켜요'}, 끌면 순서를 바꿔요`;
    chip.setAttribute('aria-pressed', String(isOn(m.id)));
    chip.append(el('span', 'chip-dot'), m.chip);
    chip.addEventListener('mousedown', (e) => e.preventDefault()); // 입력 칸 포커스 유지
    chip.addEventListener('pointerdown', (e) => dragChip(e, chip));
    chip.addEventListener('mouseenter', () => updateHint([['누르기', isOn(m.id) ? '끄기' : '켜기'], ['끌기', '순서 바꾸기']]));
    chip.addEventListener('mouseleave', () => updateHint());
    chip.addEventListener('click', () => {
      if (chip.dataset.dragged) return; // 끌어 놓은 것은 누른 것이 아님
      setModule(m.id, !isOn(m.id));
      if (!calm.matches) modulesEl.querySelector(`[data-mod="${m.id}"]`)?.animate([{ transform: 'scale(0.9)' }, { transform: 'none' }], { duration: 320, easing: SPRING });
    });
    return chip;
  }));
}

// 캡슐 끌기: 끄는 캡슐은 들려 손을 따라오고, 가리킨 캡슐을 반 넘게 지나면 자리를 바꾼다 (다른 캡슐은 비켜 미끄러짐)
function dragChip(e, chip) {
  if (e.button !== 0 || modulesEl.children.length < 2) return;
  const x0 = e.clientX;
  const y0 = e.clientY;
  let grab = null; // 캡슐 안에서 잡은 곳 (null = 아직 누르기만)
  let last = { x: x0, y: y0 };
  const chips = () => [...modulesEl.children];
  // 제자리(바뀐 순서의 자리)에서 손까지
  const follow = () => {
    chip.style.transform = '';
    const home = chip.getBoundingClientRect();
    chip.style.transform = `translate(${last.x - grab.x - home.left}px, ${last.y - grab.y - home.top}px) scale(1.06)`;
  };
  const move = (ev) => {
    if (!grab) {
      if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < 5) return;
      try {
        chip.setPointerCapture(e.pointerId); // 창 밖으로 나가도 놓을 때까지
      } catch {
        // 붙잡지 못해도 창 안에서는 window 로 받는다
      }
      const r0 = chip.getBoundingClientRect();
      grab = { x: x0 - r0.left, y: y0 - r0.top };
      chip.classList.add('dragging');
      modulesEl.classList.add('sorting');
    }
    last = { x: ev.clientX, y: ev.clientY };
    follow();
    const list = chips();
    const over = list.find((c) => {
      const r = c.getBoundingClientRect();
      return c !== chip && ev.clientX >= r.left && ev.clientX <= r.right && ev.clientY >= r.top - 3 && ev.clientY <= r.bottom + 3;
    });
    if (!over) return;
    const r = over.getBoundingClientRect();
    const after = list.indexOf(over) > list.indexOf(chip);
    if (after ? ev.clientX < r.left + r.width / 2 : ev.clientX > r.left + r.width / 2) return; // 반을 넘어야 (폭이 다른 캡슐끼리 왔다 갔다 하지 않게)
    const before = new Map(list.map((c) => [c, c.getBoundingClientRect()]));
    modulesEl.insertBefore(chip, after ? over.nextSibling : over);
    for (const c of chips()) {
      if (c === chip) continue;
      const a = before.get(c);
      const b = c.getBoundingClientRect();
      slide(c, a.left - b.left, a.top - b.top);
    }
    follow();
  };
  const end = () => {
    removeEventListener('pointermove', move);
    removeEventListener('pointerup', end);
    removeEventListener('pointercancel', end);
    if (!grab) return;
    const from = chip.style.transform; // 놓으면 제자리로 쏙
    chip.style.transform = '';
    chip.classList.remove('dragging');
    modulesEl.classList.remove('sorting');
    if (!calm.matches && from) chip.animate([{ transform: from }, { transform: 'none' }], { duration: 320, easing: SPRING });
    chip.dataset.dragged = '1';
    setTimeout(() => delete chip.dataset.dragged, 0); // 놓을 때 따라오는 click 만 넘긴다
    const ids = chips().map((c) => c.dataset.mod);
    if (ids.join() === order.join()) return;
    order = ids;
    store.set('magic.order', ids);
    render(); // 결과 묶음도 새 순서로
  };
  addEventListener('pointermove', move);
  addEventListener('pointerup', end);
  addEventListener('pointercancel', end);
}

function setModule(id, on) {
  const set = DEFAULT_OFF.has(id) ? optIn : off;
  if (on === DEFAULT_OFF.has(id)) set.add(id);
  else set.delete(id);
  if (DEFAULT_OFF.has(id)) store.set('magic.on', [...optIn]);
  else store.set('magic.off', [...off]);
  invoke('sync_modules', { off: offList() }).catch(() => {}); // 알림 영역 메뉴 체크도 같게
  renderModules();
  search();
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
  } else if (r.kind === 'new') {
    ico.append(svg('new'));
    t.textContent = '새 메모로 저장';
    s.textContent = query;
  } else if (r.kind === 'remind') {
    ico.append(svg('remind'));
    t.textContent = '알림 맞춰 저장';
    s.textContent = '글 속 날짜 · 시각에 메모킹이 알림을 맞춰요';
  } else if (r.kind === 'opms') {
    ico.append(svg('opms'));
    t.textContent = r.more ? `OPMS 다른 ${r.more.toLocaleString()}건 더 보기` : 'OPMS 에서 찾기';
    s.textContent = `'${query.slice(0, 40)}' — 사건번호 · 출원인 · 발명자 · 초성`;
  } else if (r.kind === 'case') {
    ico.append(svg('opms'));
    t.append(highlighted([r.ourref, r.title].filter(Boolean).join(' '), query));
    s.textContent = [r.applicant, r.n_app, r.accepted].filter(Boolean).join(' · ');
    s.title = s.textContent;
  } else if (r.kind === 'memo') {
    ico.append(svg(r.locked ? 'lock' : 'memo'));
    t.append(highlighted(r.title, query));
    s.textContent = r.snippet;
    s.title = r.snippet;
  } else if (r.kind === 'site') {
    ico.append(svg('web'));
    t.textContent = '사이트 열기';
    s.textContent = r.url;
  } else if (r.kind === 'web' || r.kind === 'ai') {
    ico.append(svg(r.kind));
    t.textContent = r.kind === 'web' ? '웹에서 찾기' : 'AI에게 묻기';
    s.textContent = r.sites.map(siteLabel).join(', ');
  } else {
    ico.append(svg(r.kind));
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
  // 묶음(캡슐)마다 모아 두었다가 캡슐 순서대로 그린다
  const groups = new Map();
  const add = (mod, it) => (groups.get(mod) ?? groups.set(mod, []).get(mod)).push(it);
  const section = (mod, label, count, sub) => add(mod, { section: [label, count, sub] });
  const note = (mod, text, onClick) => add(mod, { note: text, onClick });
  const put = (mod, r) => add(mod, { row: { mod, ...r } });
  if (query && res) {
    if (res.newMemo) put('memo', { kind: 'new' }); // Enter 로 바로 저장
    if (res.remind) put('memo', { kind: 'remind' }); // 내일 3시 치과 → 그때 알림
    if (res.opms) {
      const a = auto.query === query ? auto : null;
      if (a?.cases.length) {
        section('opms', 'OPMS', a.cases.length);
        a.cases.forEach((c) => put('opms', { kind: 'case', ...c }));
      } else if (a?.loading) note('opms', searching('OPMS 에서 찾는 중'));
      else if (a?.error) note('opms', a.error);
      put('opms', { kind: 'opms', more: a && a.total > a.cases.length ? a.total - a.cases.length : 0 });
    }
    if (res.calc) put('calc', { ...res.calc, text: query });
    if (res.site) add('site', { row: { kind: 'site', mod: 'site', url: res.site } });
    if (res.memos.length) {
      section('notes', '메모', res.memos.length);
      res.memos.forEach((m) => put('notes', { kind: 'memo', ...m }));
    }
    if (res.apps.length) {
      section('apps', '앱', res.apps.length);
      res.apps.forEach((a) => put('apps', { kind: 'app', ...a }));
    }
    if (res.folders.length) {
      section('files', '폴더', res.folders.length);
      res.folders.forEach((f) => put('files', { kind: 'folder', ...f }));
    }
    if (res.files.length) {
      section('files', '파일', res.files.length, res.folders.length > 0);
      res.files.forEach((f) => put('files', { kind: 'file', ...f }));
    }
    const more = res.more + res.memoMore;
    if (more > 0) note('rest', `다른 ${more.toLocaleString()}건은 글을 더 써서 좁혀 보세요`);
    const searched = isOn('apps') || isOn('files') || (sites.memo && isOn('notes'));
    const found = res.calc || res.site || res.memos.length || res.apps.length || res.folders.length || res.files.length;
    if (searched && !res.indexed) note('rest', res.building ? searching('프로그램 · 파일 목록을 처음 만드는 중이에요. 잠시 뒤 찾아져요') : '프로그램 · 파일 목록이 비어 있어요');
    else if (searched && !found) note('rest', `'${query.slice(0, 30)}'와 맞는 ${sites.memo && isOn('notes') ? '메모 · ' : ''}앱 · 파일이 없어요`);
    if (isOn('web')) put('web', { kind: 'web', sites: sites.web });
    if (isOn('ai')) put('ai', { kind: 'ai', sites: sites.ai });
    if (![...groups.values()].some((g) => g.some((it) => it.row)) && !searched) note('rest', `'${query.slice(0, 30)}'와 맞는 결과가 없어요`);
  } else if (sites.notice) note('top', sites.notice, () => invoke('hotkey_open'));

  // 순서: 캡슐 순서 · 사이트 열기는 계산 바로 뒤 · 남은 건수와 '없어요' 안내는 웹 검색 바로 위 · 묻는 글이면 AI 를 사이트 다음으로
  const order = ['top', ...shownModules().map((m) => m.id)];
  order.splice(order.indexOf('calc') + 1, 0, 'site');
  order.splice(order.includes('web') ? order.indexOf('web') : order.length, 0, 'rest');
  if (res?.question && order.includes('ai')) {
    order.splice(order.indexOf('ai'), 1);
    order.splice(order.indexOf('site') + 1, 0, 'ai');
  }
  for (const mod of order) {
    for (const it of groups.get(mod) ?? []) {
      if (it.row) {
        nodes.push(row(it.row, rows.push(it.row) - 1, query));
        continue;
      }
      const li = el('li', it.section ? `magic-section${it.section[2] ? ' sub' : ''}` : `magic-note${it.onClick ? ' action' : ''}`);
      li.setAttribute('role', 'presentation');
      if (it.section) {
        li.dataset.mod = mod;
        li.append(it.section[0]);
        if (it.section[1]) li.append(el('span', 'count', String(it.section[1])));
      } else li.append(it.note);
      if (it.onClick) {
        li.addEventListener('mousedown', (e) => e.preventDefault());
        li.addEventListener('click', it.onClick);
      }
      nodes.push(li);
    }
  }
  resultsEl.replaceChildren(pill, ...nodes);
  select(Math.min(selected, Math.max(0, rows.length - 1)), false, false);
  fit();
}

// 고른 줄 표시 하나가 줄 사이를 미끄러져 따라간다 (다시 그릴 때는 바로 그 자리에)
const pill = el('li', 'magic-pill');
pill.setAttribute('role', 'presentation');
function select(i, scroll = true, glide = true) {
  if (i !== selected) activeAct = 0;
  selected = i;
  for (const li of resultsEl.querySelectorAll('.magic-row')) li.setAttribute('aria-selected', String(Number(li.dataset.index) === i));
  const li = resultsEl.querySelector(`.magic-row[data-index="${i}"]`);
  pill.hidden = !li;
  pill.classList.toggle('instant', !glide || calm.matches);
  if (li) {
    pill.style.transform = `translateY(${li.offsetTop}px)`;
    pill.style.height = `${li.offsetHeight}px`;
  }
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
function updateHint(parts = keyHint()) {
  if (Date.now() < flashUntil) return;
  hintEl.replaceChildren(...parts.map(([k, label]) => {
    const span = el('span');
    span.append(el('kbd', '', k), ` ${label}`);
    return span;
  }));
}
function keyHint() {
  const r = rows[selected];
  const parts = [];
  if (r) {
    const acts = actionsFor(r);
    parts.push(['↵', acts[activeAct]?.label ?? '']);
    if (acts.length > 1) parts.push(['← →', '단추']);
  }
  parts.push(['Esc', input.value ? '지우기' : '닫기']);
  if (rows.length > 1) parts.push(['↑↓', '고르기']);
  return parts;
}

let flashTimer = 0;
function flash(ok, text) {
  hintEl.replaceChildren(el('span', `magic-flash${ok ? '' : ' err'}`, `${ok ? '✓ ' : ''}${text}`));
  flashUntil = Date.now() + 1800;
  clearTimeout(flashTimer);
  flashTimer = setTimeout(updateHint, 1800);
}

// 창 높이를 내용에 맞춘다 (창 그림자 자리 10px 씩)
let winHeight = 0;
function fit() {
  requestAnimationFrame(() => {
    const h = Math.ceil(box.getBoundingClientRect().height) + 20;
    if (h === winHeight) return;
    winHeight = h;
    win.setSize(new LogicalSize(680, h)).catch(() => {});
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
  const out = await invoke('query', { text, mods: shownModules().map((m) => m.id).filter(isOn) }).catch(() => null);
  if (my !== seq || !out) return; // 더 새 입력이 있으면 버린다
  res = out;
  render();
  if (out.opmsAuto) askOpms(text.trim());
}

// 저절로 찾기: 같은 글을 다시 묻지 않고, 늦게 온 답은 지금 글과 같을 때만 쓴다
function askOpms(q) {
  if (auto.query === q && !auto.error) return;
  clearTimeout(autoTimer);
  autoTimer = setTimeout(async () => {
    auto = { query: q, cases: [], total: 0, error: '', loading: true };
    render();
    const got = await invoke('opms_auto', { text: q }).then((r) => ({ ...r, error: '' }), (err) => ({ cases: [], total: 0, error: String(err) }));
    if (input.value.trim() !== q) return;
    auto = { query: q, ...got, loading: false };
    render();
  }, 250);
}

const hide = () => invoke('hide');

async function run(i, act = 0) {
  const r = rows[i];
  if (!r) return;
  const acts = actionsFor(r);
  const k = acts[act] ? act : 0;
  const a = acts[k];
  const btn = () => resultsEl.querySelectorAll(`.magic-row[data-index="${i}"] .act-btn`)[k];
  try {
    if (r.kind === 'calc' || r.kind === 'date') {
      const text = a.id === 'copyExpr' ? `${r.text} = ${r.value}` : a.id === 'copyIso' ? r.plain : r.kind === 'date' ? r.value : r.plain;
      flash(true, await invoke('copy', { text }));
      markDone(btn());
      return;
    }
    let msg;
    if (r.kind === 'new' || r.kind === 'remind') {
      msg = await invoke('new_memo', { text: input.value, open: a.id === 'saveOpen', remind: r.kind === 'remind' });
      input.value = ''; // 저장한 글은 다음에 열 때 남기지 않는다
      grow();
    } else if (r.kind === 'opms') msg = await invoke('open_opms', { text: input.value });
    else if (r.kind === 'case') {
      const ref = r.ourref || r.n_app || r.yourref;
      if (a.id === 'copyRef') {
        flash(true, await invoke('copy', { text: ref }));
        markDone(btn());
        return;
      }
      msg = await invoke('open_opms', { text: ref });
    }
    else if (r.kind === 'memo') msg = await invoke('open_memo', { id: r.id });
    else if (r.kind === 'site') msg = await invoke('open_web', { kind: 'site', site: '', text: r.url });
    else if (r.kind === 'web' || r.kind === 'ai') msg = await invoke('open_web', { kind: r.kind, site: a.id, text: input.value });
    else msg = await invoke('act', { action: a.id, path: r.path });
    if (a.id === 'copyPath') {
      flash(true, msg);
      markDone(btn());
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

// 입력칸 높이를 글 줄 수에 맞춘다 (6줄까지, 그 위로는 칸 안에서 넘김)
function grow() {
  input.style.height = 'auto';
  input.style.height = `${input.scrollHeight}px`;
}

let searchTimer = 0;
let pending = false; // 친 글을 아직 찾지 않았다 (Enter 가 지난 글의 결과를 실행하지 않게)
input.addEventListener('input', () => {
  grow();
  selected = 0;
  pending = true;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(search, 60);
  clearBtn.hidden = !input.value;
});

function clearText() {
  input.value = '';
  grow();
  selected = 0;
  search();
  input.focus();
}
clearBtn.addEventListener('mousedown', (e) => e.preventDefault());
clearBtn.addEventListener('click', clearText);

// ---------- 창 옮기기 ----------
// 손잡이 · ✦ 를 끌면 창이 따라온다. OS 끌기(startDragging)는 입력 칸의 포커스를 빼앗아 '다른 곳을 눌렀다' 로 보고 창이 숨었다
function dragWindow(e) {
  if (e.button !== 0) return;
  e.preventDefault(); // 입력 칸 포커스 유지
  const handle = e.currentTarget;
  handle.setPointerCapture(e.pointerId);
  const x0 = e.screenX;
  const y0 = e.screenY;
  const dpr = devicePixelRatio;
  let from = null;
  let at = null;
  let queued = false;
  const place = () => {
    queued = false;
    if (from && at) win.setPosition(new PhysicalPosition(Math.round(from.x + (at.x - x0) * dpr), Math.round(from.y + (at.y - y0) * dpr))).catch(() => {});
  };
  win.outerPosition().then((p) => {
    from = p;
    place();
  }, () => {});
  const move = (ev) => {
    at = { x: ev.screenX, y: ev.screenY };
    if (!queued) {
      queued = true;
      requestAnimationFrame(place);
    }
  };
  const end = () => {
    handle.removeEventListener('pointermove', move);
    handle.removeEventListener('pointerup', end);
    handle.removeEventListener('pointercancel', end);
    input.focus();
  };
  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', end);
  handle.addEventListener('pointercancel', end);
}
document.querySelectorAll('[data-drag]').forEach((h) => h.addEventListener('pointerdown', dragWindow));

input.addEventListener('keydown', (e) => {
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
  } else if (e.key === 'Enter' && !e.shiftKey) { // Shift+Enter 는 줄 바꿈
    e.preventDefault();
    const act = e.ctrlKey ? 1 : activeAct; // Ctrl+Enter: 두 번째 할 일
    (pending ? search() : Promise.resolve()).then(() => rows.length && run(selected, act));
  }
});

// 창이 떠오를 때마다: 지난 글은 남겨 두고 통째로 골라 둔다 (이어서 찾으려면 그대로, 새로 찾으려면 바로 치면 바뀐다)
function onShow() {
  // 켤 때 키를 못 잡았다는 안내는, 그 뒤 메모킹이 키를 넘겨줬으면 지워졌다 — 열 때마다 다시 받는다
  invoke('init').then((s) => {
    if (s.notice !== sites.notice || s.hotkey !== sites.hotkey) {
      Object.assign(sites, { notice: s.notice, hotkey: s.hotkey });
      render();
    }
  }).catch(() => {});
  grow();
  flashUntil = 0;
  selected = 0;
  activeAct = 0;
  search();
  input.focus();
  input.select();
}

(async () => {
  sites = await invoke('init').catch(() => sites);
  if (store.get('magic.off') == null && sites.memokingOff) off = new Set(sites.memokingOff); // 저장하지 않는다: 캡슐을 누르기 전까지 메모킹을 따라간다
  renderModules();
  render();
  invoke('sync_modules', { off: offList() }).catch(() => {});
  await listen('magic:module', (e) => setModule(...e.payload)); // 알림 영역 메뉴 '설정 › 기능'
  await listen('magic:show', onShow);
})();
