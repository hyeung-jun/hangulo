const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const { listen } = window.__TAURI__.event;

const appWindow = getCurrentWindow();

const header = document.querySelector("#header");
const direction = document.querySelector("#direction");
const count = document.querySelector("#count");
const close = document.querySelector("#close");
const body = document.querySelector("#body");
const text = document.querySelector("#text");
const spinner = document.querySelector("#spinner");
const grip = document.querySelector("#grip");
const windowEl = document.getElementById("window");

const MAX_LINES = 12;
const DEFAULT_WIDTH = 360;
const LOADING_WIDTH = 240;
const LOADING_HEIGHT = 80;

function lineHeightPx() {
  return parseFloat(getComputedStyle(text).lineHeight);
}

function renderLoading() {
  body.classList.add("loading");
  spinner.hidden = false;
  text.hidden = true;
  header.classList.add("empty");
  invoke("popup_content_ready", { width: LOADING_WIDTH, height: LOADING_HEIGHT });
}

function render({ direction: dir, count: charCount, text: value }) {
  body.classList.remove("loading");
  spinner.hidden = true;
  text.hidden = false;

  const hasMeta = Boolean(dir || charCount);
  header.classList.toggle("empty", !hasMeta);
  direction.textContent = dir ?? "";
  count.textContent = charCount ? `${charCount}자` : "";
  text.textContent = value;

  // 폭은 항상 기본값으로 되돌린 뒤 그 폭 기준으로 줄바꿈을 측정한다.
  // 실제 OS 창 크기(사용자가 리사이즈했을 수 있는)에 기대면 폭에 따라
  // 줄바꿈이 달라져서 측정값과 최종 창 크기가 어긋난다.
  windowEl.style.width = `${DEFAULT_WIDTH}px`;

  // 처음 띄울 창 높이를 내용에 맞춰 잡는다 (12줄 넘으면 그 높이에서 끊고 스크롤).
  // body 에 max-height 를 걸지는 않는다 — 걸어두면 사용자가 그립으로 창을 세로로
  // 늘려도 본문이 그 높이에 묶여 더 보이지 않는다. 창 높이는 여기서 계산한 값으로
  // 시작하고, 그 뒤로는 flex:1 인 body 가 창 크기를 그대로 따라가게 둔다.
  // 창이 hidden 상태라 requestAnimationFrame 콜백이 아예 실행되지 않으므로
  // (컴포지터가 프레임을 그리지 않음) 다음 틱을 기다리지 않고 바로 측정한다 —
  // offsetHeight/scrollHeight 를 읽는 시점에 브라우저가 동기적으로 레이아웃을
  // 재계산해주므로 hidden 상태에서도 정확한 값이 나온다.
  const bodyPadding =
    parseFloat(getComputedStyle(body).paddingTop) +
    parseFloat(getComputedStyle(body).paddingBottom);
  const capHeight = lineHeightPx() * MAX_LINES + bodyPadding;
  const bodyHeight = Math.min(text.scrollHeight + bodyPadding, capHeight);

  const headerHeight = hasMeta ? header.offsetHeight + 1 : 0; // +1 divider
  const totalHeight = headerHeight + bodyHeight;
  invoke("popup_content_ready", { width: DEFAULT_WIDTH, height: totalHeight });

  // 창 크기를 이 폭에 맞춰 바꿔달라고 요청만 해두고, 화면에는 실제 창 크기(100vw/100vh)를
  // 그대로 따르게 되돌린다 — 그래야 그립으로 리사이즈할 때도 내용이 창 크기를 따라간다.
  windowEl.style.width = "";
}

function renderError(message) {
  render({ direction: "", count: 0, text: message });
}

listen("loading", () => renderLoading());
listen("translation", (event) => render(event.payload));
listen("popup-error", (event) => renderError(event.payload.message));

close.addEventListener("click", () => invoke("popup_hide"));

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    invoke("popup_hide");
    return;
  }
  // 전체 선택은 번역문만 대상으로 한다. 헤더에 user-select:none 을 걸어둬도
  // WebKit 의 기본 전체 선택은 헤더 텍스트까지 함께 집어가기 때문에 직접 잡는다.
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a") {
    event.preventDefault();
    const range = document.createRange();
    range.selectNodeContents(text);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }
});

header.addEventListener("mousedown", async (event) => {
  if (event.target === close) return;
  await appWindow.startDragging();
  invoke("popup_save_position");
});

// macOS 의 tao 백엔드가 startResizeDragging 을 구현하지 않아(항상 실패) 마우스
// 이동량만큼 직접 set_size 를 호출하는 방식으로 리사이즈한다. screenX/Y 는 논리
// 픽셀 기준이라 별도 배율 변환 없이 바로 델타로 쓸 수 있다.
grip.addEventListener("mousedown", (event) => {
  event.preventDefault();
  const startX = event.screenX;
  const startY = event.screenY;
  const startWidth = windowEl.getBoundingClientRect().width;
  const startHeight = windowEl.getBoundingClientRect().height;

  let pendingSize = null;
  let rafId = null;

  const flush = () => {
    rafId = null;
    if (pendingSize) invoke("popup_resize", pendingSize);
  };

  const onMove = (moveEvent) => {
    const width = Math.max(240, startWidth + (moveEvent.screenX - startX));
    const height = Math.max(80, startHeight + (moveEvent.screenY - startY));
    pendingSize = { width, height };
    if (rafId === null) {
      rafId = requestAnimationFrame(flush);
    }
  };
  const onUp = () => {
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      flush();
    }
    invoke("popup_save_position");
  };
  document.addEventListener("mousemove", onMove);
  document.addEventListener("mouseup", onUp);
});
