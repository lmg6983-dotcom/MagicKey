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
  // 글자 단추였던 할 일들 — 같은 줄에 복사가 여럿이면 무엇을 복사하는지 그림으로 갈린다
  saveMemo: 'M6 3h9l4 4v14H6zM14 3v5h5M9 14l2 2 4-4',
  saveOpen: 'M12 21H6V3h9l4 4v4M14 3v5h5M15 17.5h6M18.5 15l2.5 2.5-2.5 2.5',
  calendarAdd: 'M7 3v3M17 3v3M4 9h16M12 20H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v6M18 15v6M15 18h6',
  copyText: 'M8 8V5a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-3M5 8h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1zM7.5 12.5h5M7.5 15.5h3',
  copyHash: 'M8 8V5a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-3M5 8h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1zM7.5 12.5h5M7.5 15.5h5M9.4 11l-.6 6M11.6 11l-.6 6',
  copyDate: 'M8 8V5a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-3M5 8h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1zM7 12h7M9 10.5v3M12 10.5v3',
  more: 'M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01',
};
// 웹 · AI 사이트 단추: 그 사이트의 로고 (simple-icons, CC0) — 색은 앱 글자색 한 가지 (팔레트 밖 브랜드 색을 쓰지 않고, 어두운 화면에서도 보이게)
//   Bing 처럼 로고가 없는 곳은 지구 · AI 선 아이콘
const BRANDS = {
  naver: 'M16.273 12.845 7.376 0H0v24h7.726V11.156L16.624 24H24V0h-7.727v12.845Z',
  google: 'M12.48 10.92v3.28h7.84c-.24 1.84-.853 3.187-1.787 4.133-1.147 1.147-2.933 2.4-6.053 2.4-4.827 0-8.6-3.893-8.6-8.72s3.773-8.72 8.6-8.72c2.6 0 4.507 1.027 5.907 2.347l2.307-2.307C18.747 1.44 16.133 0 12.48 0 5.867 0 .307 5.387.307 12s5.56 12 12.173 12c3.573 0 6.267-1.173 8.373-3.36 2.16-2.16 2.84-5.213 2.84-7.667 0-.76-.053-1.467-.173-2.053H12.48z',
  youtube: 'M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z',
  duckduckgo: 'M12 0C5.373 0 0 5.373 0 12s5.373 12 12 12 12-5.373 12-12S18.627 0 12 0zm0 23C5.925 23 1 18.074 1 12S5.926 1 12 1s11 4.925 11 11-4.925 11-11 11zm10.219-11c0 4.805-3.317 8.833-7.786 9.925-.27-.521-.53-1.017-.749-1.438.645.249 1.93.718 2.208.615.376-.144.282-3.149-.14-3.245-.338-.075-1.632.837-2.141 1.209l.034.156c.078.397.144.993.03 1.247-.001.004-.002.01-.004.013a.218.218 0 0 1-.068.088c-.284.188-1.081.284-1.503.188a.516.516 0 0 1-.064-.02c-.694.396-2.01 1.109-2.25.971-.329-.188-.377-2.676-.329-3.288.035-.46 1.653.286 2.442.679.174-.163.602-.272.98-.31-.57-1.389-.99-2.977-.733-4.105 0 .002.002.002.002.002.356.248 2.73 1.05 3.91 1.027 1.18-.024 3.114-.743 2.903-1.323-.212-.58-2.135.51-4.142.324-1.486-.138-1.748-.804-1.42-1.29.414-.611 1.168.116 2.411-.256 1.245-.371 2.987-1.035 3.632-1.397 1.494-.833-.625-1.177-1.125-.947-.474.22-2.123.637-2.889.82.428-1.516-.603-4.149-1.757-5.3-.376-.376-.951-.612-1.603-.736-.25-.344-.654-.671-1.225-.977a5.772 5.772 0 0 0-3.595-.584l-.024.004-.034.004.004.002c-.148.028-.237.08-.357.098.148.016.705.276 1.057.418-.174.068-.412.108-.596.184a.828.828 0 0 0-.204.056c-.173.08-.303.375-.3.515.84-.086 2.082-.026 2.991.246-.644.09-1.235.258-1.661.482-.016.008-.03.018-.048.028-.054.02-.106.042-.152.066-1.367.72-1.971 2.405-1.611 4.424.323 1.824 1.665 8.088 2.29 11.064-3.973-1.4-6.822-5.186-6.822-9.639C1.781 6.356 6.356 1.781 12 1.781S22.219 6.356 22.219 12zM9.095 9.581a.758.758 0 1 0 0 1.516.758.758 0 0 0 0-1.516zm.338.702a.196.196 0 1 1 0-.392.196.196 0 0 1 0 .392zm4.724-1.043a.65.65 0 1 0 0 1.299.65.65 0 0 0 0-1.3zm.29.601a.168.168 0 1 1 0-.336.168.168 0 0 1 0 .336zM9.313 8.146s-.571-.26-1.125.09c-.554.348-.534.704-.534.704s-.294-.656.49-.978c.786-.32 1.17.184 1.17.184zm5.236-.052s-.41-.234-.73-.23c-.654.008-.831.296-.831.296s.11-.688.945-.55a.84.84 0 0 1 .616.484z',
  chatgpt: 'M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z',
  claude: 'm4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z',
  gemini: 'M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81',
  perplexity: 'M22.3977 7.0896h-2.3106V.0676l-7.5094 6.3542V.1577h-1.1554v6.1966L4.4904 0v7.0896H1.6023v10.3976h2.8882V24l6.932-6.3591v6.2005h1.1554v-6.0469l6.9318 6.1807v-6.4879h2.8882V7.0896zm-3.4657-4.531v4.531h-5.355l5.355-4.531zm-13.2862.0676 4.8691 4.4634H5.6458V2.6262zM2.7576 16.332V8.245h7.8476l-6.1149 6.1147v1.9723H2.7576zm2.8882 5.0404v-3.8852h.0001v-2.6488l5.7763-5.7764v7.0111l-5.7764 5.2993zm12.7086.0248-5.7766-5.1509V9.0618l5.7766 5.7766v6.5588zm2.8882-5.0652h-1.733v-1.9723L13.3948 8.245h7.8478v8.087z',
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

function brandSvg(id) {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('aria-hidden', 'true');
  s.setAttribute('class', 'brand');
  const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  p.setAttribute('d', BRANDS[id]);
  s.append(p);
  return s;
}

// 할 일 단추의 그림 (모든 할 일은 그림 단추 — 이름은 마우스를 올리거나 키보드로 고르면 보인다)
function actionIcon(r, a) {
  if (r.kind === 'web' || r.kind === 'ai') return BRANDS[a.id] ? brandSvg(a.id) : svg(r.kind);
  if (a.id === 'save') return svg(r.kind === 'remind' ? 'calendarAdd' : 'saveMemo');
  const name = { openMemo: 'open', copyMemo: 'copyText', copyExpr: 'copyText', copyRef: 'copyHash', copyIso: 'copyDate' }[a.id] ?? a.id;
  return svg(ICONS[name] ? name : 'open');
}

// 이미 받아 둔 실제 아이콘은 다시 그릴 때 선 그림을 거치지 않고 바로 붙인다 (스크롤 · 다시 그리기 깜빡임 방지)
const iconState = new Map(); // src → true(받음) | false(없음)
const ICON_CACHE_MAX = 2000;
function rememberIcon(src, ok) {
  if (iconState.size >= ICON_CACHE_MAX) iconState.clear();
  iconState.set(src, ok);
}

/**
 * 앱 · 파일 · 폴더의 실제 아이콘 (탐색기에 보이는 그림 — app/src/shellicon.rs 가 fileicon:// 로 준다).
 * 그림이 오면 칸의 기능 색 바탕을 걷고 그림만, 못 얻으면(다른 OS · 지운 파일) 원래 선 그림 그대로.
 */
function realIcon(ico, kind, path) {
  const conv = window.__TAURI__?.core?.convertFileSrc;
  if (!path || !conv) {
    ico.append(svg(kind));
    return;
  }
  const src = conv(path, 'fileicon');
  const cached = iconState.get(src);
  if (cached === false) {
    ico.append(svg(kind));
    return;
  }
  if (cached === true) {
    const img = new Image();
    img.alt = '';
    img.draggable = false;
    img.onerror = () => {
      iconState.delete(src);
      ico.classList.remove('real');
      ico.replaceChildren(svg(kind));
    };
    img.src = src;
    ico.classList.add('real');
    ico.append(img);
    return;
  }
  ico.append(svg(kind));
  const img = new Image(); // 문서에 붙이기 전에 받아 두고, 다 받았을 때만 바꾼다 (깜빡이지 않게)
  img.alt = '';
  img.decoding = 'async';
  img.draggable = false;
  img.onload = () => {
    rememberIcon(src, true);
    ico.replaceChildren(img);
    ico.classList.add('real');
  };
  img.onerror = () => rememberIcon(src, false);
  img.src = src;
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
    case 'new': return [{ id: 'save', label: '저장' }, { id: 'saveOpen', label: '저장하고 열기', key: 'Ctrl+Enter' }];
    case 'remind': return [{ id: 'save', label: '일정 추가' }];
    case 'opms': return [{ id: 'open', label: 'MagicSystem 에서 찾기' }];
    case 'case': return [{ id: 'open', label: 'MagicSystem 에서 열기' }, { id: 'copyRef', label: '관리번호 복사' }];
    case 'memo': return [{ id: 'openMemo', label: '메모킹에서 열기' }, ...(r.locked ? [] : [{ id: 'copyMemo', label: '내용 복사' }])];
    case 'app': return [{ id: 'open', label: '실행' }, { id: 'reveal', label: '폴더 열기', key: 'Ctrl+Shift+E' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }];
    case 'folder': return [{ id: 'open', label: '열기' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }, { id: 'reveal', label: '위 폴더에서 보기', key: 'Ctrl+Shift+E' }, { id: 'copyFile', label: '폴더 복사' }];
    case 'file': return [{ id: 'open', label: '열기' }, { id: 'copyPath', label: '경로 복사', key: 'Ctrl+Shift+C' }, { id: 'reveal', label: '폴더 열기', key: 'Ctrl+Shift+E' }, { id: 'copyFile', label: '파일 복사' }];
    case 'calc': return [{ id: 'copy', label: '결과 복사' }, { id: 'copyExpr', label: '식까지 복사' }];
    case 'date': return [{ id: 'copy', label: '결과 복사' }, { id: 'copyIso', label: `${r.plain} 복사` }, { id: 'copyExpr', label: '쓴 글까지 복사' }];
    case 'site': return [{ id: 'open', label: '사이트 열기' }];
    case 'web': case 'ai': return r.sites.map((s) => ({ id: s.id, label: siteLabel(s) }));
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

// 찾은 파일 · 폴더 줄을 끌어 탐색기 · 바탕화면 · 다른 폴더에 복사해 놓기 (drag_out — Windows 끌어 놓기, 원본은 그대로)
//   조금(6px) 움직여야 끌기 — 그냥 누르기는 지금처럼 열기. 끈 뒤 따라오는 click 은 넘긴다 (row.dataset.dragged)
function dragOut(row, path) {
  row.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.target.closest('.act-btn')) return;
    const x = e.clientX;
    const y = e.clientY;
    const stop = () => {
      row.removeEventListener('pointermove', move);
      row.removeEventListener('pointerup', stop);
    };
    const move = (m) => {
      if (Math.hypot(m.clientX - x, m.clientY - y) < 6) return;
      stop();
      row.dataset.dragged = '1';
      setTimeout(() => delete row.dataset.dragged, 400);
      window.__TAURI__.core.invoke('drag_out', { path }).catch(() => {});
    };
    row.addEventListener('pointermove', move);
    row.addEventListener('pointerup', stop);
  });
}
