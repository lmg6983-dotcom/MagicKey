// 매직키 창(magic.js)과 모두 찾기 창(finder.js)이 함께 쓰는 것 — 선 아이콘 · 찾는 중 표시 · 찾은 말 굵게 · 줄마다 할 일
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
  more: 'M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01',
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
    case 'more': return [{ id: 'open', label: '모두 찾기' }];
    default: return [];
  }
}

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
