// 단축키 창 — 알림 영역 메뉴 '단축키 바꾸기' · 매직키 창의 안내에서 연다 (main.rs open_hotkey)
//   열면 바로 기록: 누르는 조합키가 글쇠로 바로 보이고, 키까지 누르면 쓸 수 있는지 알려 준다 → 저장.
//   키는 매직키의 키보드 훅이 받아 넘긴다 (keyhook.rs) — 다른 프로그램이 쓰는 Ctrl+Space · Win+… 도 여기서는 고를 수 있다.
//   훅이 못 받은 키는 이 창이 직접 받는다 (아래 keydown)
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;
const win = getCurrentWindow();
const $ = (id) => document.getElementById(id);
const ui = { field: $('hk-field'), keys: $('hk-keys'), prompt: $('hk-prompt'), status: $('hk-status'), chips: $('hk-chips'), reset: $('hk-reset'), cancel: $('hk-cancel'), save: $('hk-save') };

let info = { now: { parts: [], key: '' }, suggest: [], hooked: true };
let cand = null; // 고른 새 키 { parts, key, problem, warn }
let live = null; // 기록 중 누르고 있는 조합키 ['Ctrl', 'Alt']
let rec = false;
let saved = null; // 저장한 뒤 안내

const same = (a, b) => Boolean(a && b) && a.key === b.key;

function record(on) {
  rec = on;
  live = null;
  invoke('hotkey_record', { on }).catch(() => {});
  render();
}

function keycaps(parts, waiting) {
  const caps = parts.map((p) => Object.assign(document.createElement('kbd'), { textContent: p }));
  if (waiting) caps.push(Object.assign(document.createElement('kbd'), { className: 'wait', textContent: parts.length ? '키' : '…' }));
  ui.keys.replaceChildren(...caps);
}

function render() {
  const shown = cand ?? info.now;
  const changed = cand && !cand.problem && !same(cand, info.now);
  if (rec && (live || !cand)) keycaps(live ?? [], true);
  else keycaps(shown.parts, false);
  ui.field.classList.toggle('rec', rec);
  ui.field.classList.toggle('ok', Boolean(changed));
  if (!cand?.problem) ui.field.classList.remove('bad');
  ui.prompt.textContent = rec ? '새 단축키를 누르세요 · Esc 취소' : '눌러서 다른 키 고르기';
  ui.field.setAttribute('aria-label', `${rec ? '새 단축키를 기다리는 중' : '지금 단축키'}: ${shown.parts.join(' + ')}`);

  let cls = '';
  let text = '';
  if (saved) [cls, text] = ['ok', saved];
  else if (cand?.problem) [cls, text] = ['err', cand.problem];
  else if (cand?.warn && !same(cand, info.now)) [cls, text] = ['warn', cand.warn];
  else if (changed) [cls, text] = ['ok', `✓ 쓸 수 있어요 — 저장하면 ${cand.parts.join('+')} 로 열어요`];
  else if (rec && !info.hooked) [cls, text] = ['warn', '키보드 훅을 쓰지 못해 다른 프로그램이 먼저 쓰는 키는 고를 수 없어요'];
  else if (rec) text = 'Ctrl · Alt · Win 중 하나와 함께 누르세요 (예: Ctrl+Alt+Space)';
  else text = `지금은 ${info.now.parts.join('+')} 로 열어요`;
  ui.status.className = `hk-status${cls ? ` ${cls}` : ''}`;
  ui.status.textContent = text;

  for (const chip of ui.chips.children) chip.setAttribute('aria-pressed', String(same(chip.sug, cand ?? info.now)));
  ui.save.disabled = !changed;
  ui.save.textContent = changed && cand.warn ? '그래도 저장' : '저장';
  ui.reset.disabled = same(info.now, info.suggest[0]) && (!cand || same(cand, info.now));
  ui.cancel.textContent = changed ? '취소' : '닫기';
  fit();
}

// 창 높이를 내용에 맞춘다 (폭은 그대로)
let fitted = 0;
function fit() {
  requestAnimationFrame(() => {
    const h = Math.ceil(document.querySelector('.hk').getBoundingClientRect().height);
    if (h === fitted) return;
    fitted = h;
    win.setSize(new LogicalSize(440, h)).catch(() => {});
  });
}

function pick(next) {
  cand = next;
  saved = null;
  if (next.problem) { // 다시 누르게 — 칸을 흔들어 알린다
    ui.field.classList.remove('bad');
    void ui.field.offsetWidth;
    ui.field.classList.add('bad');
  } else {
    record(false);
    document.activeElement?.blur(); // 단추에 초점이 있으면 다음 Space 가 그 단추를 누른다 — 저장은 Enter 나 단추로
  }
  render();
}

// 고른 키 하나 (훅이 넘겼거나 이 창이 받은 것) — Esc 는 그만, Backspace 는 다시
function take(p) {
  live = null;
  const alone = p.parts.length === 1;
  if (alone && p.parts[0] === 'Esc') return record(false);
  if (alone && p.parts[0] === 'Backspace') {
    cand = null;
    return render();
  }
  pick(p);
}

listen('hotkey:pressed', ({ payload: p }) => {
  if (!rec) return;
  if (!p.key) { // 조합키만 누르는 중
    live = p.parts.length ? p.parts : null;
    render();
    return;
  }
  take(p);
});

ui.field.addEventListener('click', () => {
  saved = null;
  if (!rec) cand = null; // 새로 기록 — 빈 글쇠부터
  record(!rec);
});
ui.reset.addEventListener('click', () => pick({ ...info.suggest[0] }));
ui.cancel.addEventListener('click', () => {
  if (cand && !same(cand, info.now)) {
    cand = null;
    render();
  } else win.close();
});
ui.save.addEventListener('click', async () => {
  if (!cand || ui.save.disabled) return;
  try {
    const label = await invoke('set_hotkey', { key: cand.key });
    info.now = { ...cand };
    cand = null;
    saved = `✓ 이제 ${label} 로 열어요`;
    render();
    setTimeout(() => win.close(), 1200);
  } catch (err) {
    cand = { ...cand, problem: String(err) };
    render();
  }
});
// 기록 중: 훅이 받은 키는 여기 오지 않는다. 그래도 오는 키(훅을 못 걸었을 때)는 여기서 받고, 단추를 누르지 않게 막는다
const MODS = [['ctrlKey', 'ctrl', 'Ctrl'], ['altKey', 'alt', 'Alt'], ['shiftKey', 'shift', 'Shift'], ['metaKey', 'super', 'Win']];
addEventListener('keydown', (e) => {
  if (!rec) {
    if (e.key === 'Escape') ui.cancel.click();
    else if (e.key === 'Enter' && !ui.save.disabled && !e.target.closest?.('button')) ui.save.click();
    return;
  }
  e.preventDefault();
  e.stopPropagation();
  if (e.isComposing || e.repeat) return;
  const held = MODS.filter(([k]) => e[k]);
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(e.code) || !e.code) { // 조합키만
    live = held.length ? held.map(([, , name]) => name) : null;
    return render();
  }
  if (e.code === 'Escape' && !held.length) return take({ parts: ['Esc'] });
  if (e.code === 'Backspace' && !held.length) return take({ parts: ['Backspace'] });
  invoke('hotkey_info', { key: [...held.map(([, k]) => k), e.code].join('+') })
    .then((p) => take(p), (err) => take({ parts: [...held.map(([, , n]) => n), e.code], key: null, problem: String(err) }));
}, true);
addEventListener('keyup', (e) => rec && e.preventDefault(), true);
// 다른 창을 눌렀다 돌아오면 기록을 이어서 (창이 앞에 없으면 main.rs 가 기록을 멈춘다 — 다른 곳의 키를 가로채지 않게)
addEventListener('focus', () => rec && invoke('hotkey_record', { on: true }).catch(() => {}));

invoke('hotkey_state').then((s) => {
  info = s;
  ui.chips.replaceChildren(...s.suggest.map((sug, i) => {
    const chip = Object.assign(document.createElement('button'), { type: 'button', className: 'hk-chip', textContent: sug.parts.join('+') });
    if (i === 0) chip.append(Object.assign(document.createElement('small'), { textContent: '기본' }));
    chip.sug = sug;
    chip.title = sug.warn ?? '';
    chip.addEventListener('click', () => pick({ ...sug }));
    return chip;
  }));
  record(true);
}, () => render());
