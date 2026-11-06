// 업데이트 창 — 알림 영역 메뉴 '업데이트 확인' 또는 조용한 확인에서 새 버전을 찾았을 때 뜬다 (main.rs 자동 업데이트)
//   새 버전: 바뀐 점 → 받기(진행률) → 설치 파일이 MagicKey 를 끄고 새 버전을 켠다 · 나중에 = 이 버전은 다시 띄우지 않음
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);
const ui = {
  title: $('upd-title'), version: $('upd-version'), notesBox: $('upd-notes-box'), notes: $('upd-notes'),
  progress: $('upd-progress'), fill: $('upd-bar-fill'), progressText: $('upd-progress-text'), msg: $('upd-msg'),
  later: $('upd-later'), go: $('upd-go'),
};
let st = { status: 'idle', current: '' };
let shownNotes = null;

// 릴리스 글(docs/releases/v*.md) — 제목 · 목록 · 굵게만, 링크는 글자만 (글은 textContent 로만)
function renderNotes(md) {
  const nodes = [];
  let list = null;
  const inline = (parent, text) => {
    text.split(/(\*\*[^*]+\*\*)/).forEach((part) => {
      if (/^\*\*[^*]+\*\*$/.test(part)) parent.append(Object.assign(document.createElement('strong'), { textContent: part.slice(2, -2) }));
      else if (part) parent.append(part.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/`/g, '')); // 링크는 글자만
    });
    return parent;
  };
  for (const raw of String(md ?? '').split(/\r?\n/)) {
    const line = raw.trim();
    if (/^#\s/.test(line) || /^##\s/.test(line) || !line) { // 맨 위 큰 제목은 창 제목과 겹친다
      list = null;
      continue;
    }
    if (/^#{3,}\s/.test(line)) {
      if (/^#+\s*설치\s*$/.test(line)) break; // '설치' 칸은 손으로 받을 때 이야기 — 여기서는 받기 단추가 한다
      list = null;
      nodes.push(inline(document.createElement('h3'), line.replace(/^#+\s*/, '')));
    } else if (/^[-*]\s/.test(line)) {
      if (!list) nodes.push(list = document.createElement('ul'));
      list.append(inline(document.createElement('li'), line.slice(2)));
    } else {
      list = null;
      nodes.push(inline(document.createElement('p'), line));
    }
  }
  ui.notes.replaceChildren(...nodes);
  ui.notes.scrollTop = 0;
}

function versionLine() {
  if (!st.version || st.status === 'latest') {
    ui.version.textContent = `지금 v${st.current}`;
    return;
  }
  const span = (cls, text) => Object.assign(document.createElement('span'), { className: cls, textContent: text });
  const arrow = span('arrow', '→');
  arrow.setAttribute('aria-hidden', 'true');
  ui.version.replaceChildren(span('from', `v${st.current}`), arrow, span('to', `v${st.version}`));
  ui.version.setAttribute('aria-label', `지금 v${st.current}에서 v${st.version}으로`);
}

function render() {
  const s = st.status;
  const hasUpdate = Boolean(st.version) && (s === 'available' || s === 'downloading' || (s === 'error' && st.version));
  ui.title.textContent = {
    checking: '업데이트 확인 중',
    latest: '최신 버전이에요',
    available: '새 버전이 나왔어요',
    downloading: '새 버전을 받는 중',
    error: st.manual ? '자동 업데이트 정보가 없어요' : hasUpdate ? '업데이트하지 못했어요' : '확인하지 못했어요',
  }[s] ?? 'MagicKey 업데이트';
  versionLine();
  ui.notesBox.hidden = !hasUpdate;
  if (hasUpdate && shownNotes !== st.notes) {
    shownNotes = st.notes;
    renderNotes(st.notes || '바뀐 점이 적혀 있지 않아요');
  }
  ui.progress.hidden = s !== 'downloading';
  ui.fill.style.transform = `scaleX(${(st.percent ?? 0) / 100})`;
  ui.progressText.textContent = s === 'downloading' ? (st.percent >= 100 ? '설치하고 다시 켜요…' : `${st.percent ?? 0}%`) : '';
  ui.msg.className = `upd-msg${s === 'error' ? ' err' : ''}`;
  ui.msg.textContent = s === 'error' ? st.message ?? '' : s === 'latest' ? '새 버전이 나오면 알림 영역 ✦ 메뉴와 이 창으로 알려 드려요' : s === 'available' ? '받으면 MagicKey 가 잠깐 꺼졌다가 새 버전으로 다시 켜져요' : '';
  ui.go.textContent = st.manual ? '릴리스 페이지 열기' : { available: '받아서 설치', downloading: '받는 중', checking: '확인 중' }[s] ?? (hasUpdate ? '다시 받기' : '다시 확인');
  ui.go.disabled = s === 'downloading' || s === 'checking';
  ui.later.textContent = s === 'available' ? '나중에' : s === 'downloading' ? '창 닫기 (계속 받아요)' : '닫기';
}

function update(next) {
  st = next ?? st;
  render();
}

ui.go.addEventListener('click', async () => {
  if (st.manual) {
    invoke('update_page').catch(() => {});
    return;
  }
  if (st.version && (st.status === 'available' || st.status === 'error')) {
    await invoke('update_install').catch(() => {}); // 실패하면 update:state 로 error 가 온다
    return;
  }
  update(await invoke('update_check').catch(() => st));
});
ui.later.addEventListener('click', () => {
  if (st.status === 'available') invoke('update_later');
  else window.__TAURI__.window.getCurrentWindow().close();
});
addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && st.status !== 'downloading') ui.later.click();
});

listen('update:state', (e) => update(e.payload));
invoke('update_state').then(update, () => {});
