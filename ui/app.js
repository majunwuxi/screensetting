/* ============================================================
   Wallpaper Studio 2.0 — frontend controller
   ============================================================ */

"use strict";

const TAURI = window.__TAURI__ || {};
let invoke = TAURI.core?.invoke;
const appWindow = TAURI.window?.getCurrentWindow?.();

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

const icon = (id) => `<svg class="ic"><use href="#${id}"></use></svg>`;
const esc = (value) =>
  String(value ?? "").replace(/[&<>"']/g, (ch) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  }[ch]));

const nf = new Intl.NumberFormat("zh-CN");

const FIT_OPTIONS = [
  { value: "cover", label: "填充", hint: "满屏裁切" },
  { value: "blur", label: "氛围", hint: "模糊背景" },
  { value: "contain", label: "适应", hint: "完整显示" },
  { value: "stretch", label: "拉伸", hint: "铺满画面" },
  { value: "center", label: "居中", hint: "原比例" },
  { value: "tile", label: "平铺", hint: "重复纹理" },
];

const state = {
  ready: false,
  settings: null,
  screen: { width: 1920, height: 1080, monitors: 1 },
  isAdmin: false,
  history: [],
  photos: [],
  query: "nature landscape",
  page: 1,
  totalPages: 1,
  total: 0,
  searching: false,
  loadingMore: false,
  fit: "cover",
  targets: { wallpaper: true, lockscreen: false },
  modal: null,
  lockSim: false,
  system: {},
  version: "2.0.0",
  cacheBytes: 0,
  libraryCount: 0,
  clockTimer: null,
  saveTimer: null,
};

/* ---------- small helpers ---------- */

function fmtBytes(bytes) {
  if (!bytes || bytes < 1) return "0 MB";
  const mb = bytes / 1024 / 1024;
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
}

function fmtTime(ts) {
  if (!ts) return "—";
  try {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "long",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "—";
  }
}

function fitLabel(value) {
  return FIT_OPTIONS.find((f) => f.value === value)?.label || "填充";
}

function toast(type, title, desc = "", action = null) {
  const root = $("#toastRoot");
  while (root.children.length >= 4) root.firstElementChild.remove();

  const el = document.createElement("div");
  el.className = `toast ${type}`;
  const iconId =
    type === "success" ? "i-check" : type === "error" ? "i-x" : type === "warn" ? "i-info" : "i-sparkles";

  el.innerHTML = `
    <div class="toast-icon">${icon(iconId)}</div>
    <div class="toast-body">
      <div class="toast-title">${esc(title)}</div>
      ${desc ? `<div class="toast-desc">${esc(desc)}</div>` : ""}
    </div>
    <button class="toast-close" title="关闭">${icon("i-x")}</button>`;

  const remove = () => {
    if (!el.isConnected) return;
    el.classList.add("is-leaving");
    setTimeout(() => el.remove(), 240);
  };

  $(".toast-close", el).addEventListener("click", remove);

  if (action) {
    const button = document.createElement("button");
    button.className = "toast-action";
    button.textContent = action.label;
    button.addEventListener("click", () => {
      action.onClick?.();
      remove();
    });
    $(".toast-body", el).appendChild(button);
  } else {
    setTimeout(remove, 5600);
  }

  root.appendChild(el);
}

function setBusy(button, busy, label = "处理中…") {
  if (!button) return;
  if (busy) {
    if (!button.dataset.restore) button.dataset.restore = button.innerHTML;
    button.classList.add("is-busy");
    button.disabled = true;
    button.innerHTML = `<span class="spinner"></span><span class="btn-label">${esc(label)}</span>`;
  } else {
    button.classList.remove("is-busy");
    button.disabled = false;
    if (button.dataset.restore) {
      button.innerHTML = button.dataset.restore;
      delete button.dataset.restore;
    }
  }
}

function emptyBlock(title, desc, iconId = "i-image-off") {
  return `<div class="empty">
    <div class="empty-icon">${icon(iconId)}</div>
    <strong>${esc(title)}</strong>
    <p>${esc(desc)}</p>
  </div>`;
}

/* ---------- window chrome ---------- */

function bindWindowControls() {
  const min = $("#winMin");
  const max = $("#winMax");
  const close = $("#winClose");

  min?.addEventListener("click", () => appWindow?.minimize());
  close?.addEventListener("click", () => appWindow?.close());
  max?.addEventListener("click", async () => {
    try {
      await appWindow?.toggleMaximize();
      syncMaxIcon();
    } catch (err) {
      console.warn(err);
    }
  });
}

async function syncMaxIcon() {
  try {
    const maximized = await appWindow?.isMaximized();
    const use = $("#winMax use");
    if (use) use.setAttribute("href", maximized ? "#i-restore" : "#i-square");
  } catch {
    /* ignore */
  }
}

/* ---------- navigation ---------- */

function setPage(page) {
  $$(".nav-item").forEach((btn) => btn.classList.toggle("is-active", btn.dataset.page === page));
  $$(".view").forEach((view) => view.classList.toggle("is-active", view.dataset.view === page));
  $("#scroll")?.scrollTo({ top: 0, behavior: "smooth" });
  if (page === "library") refreshHistory(true);
}

function bindNav() {
  $$(".nav-item").forEach((btn) => {
    btn.addEventListener("click", () => setPage(btn.dataset.page));
  });
}

/* ---------- demo bridge (only used when the UI is opened outside Tauri) ---------- */

function createMockBridge() {
  // 预置示例图（仅用于脱离 Tauri 时的界面演示）。
  const img = (seed) => {
    let hash = 0;
    for (const ch of String(seed)) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
    return `samples/${hash % 8}.jpg`;
  };
  const titles = [
    "雾中的山谷与第一缕晨光",
    "海岸线上的长曝光",
    "极简主义混凝土建筑",
    "森林深处的蓝调时刻",
    "沙丘之上的星轨",
    "雨后城市的霓虹倒影",
    "被云层切开的雪峰",
    "冰岛的黑色沙滩",
    "抽象的色彩流体",
    "安静的湖面与倒影",
    "夜航中的灯塔",
    "午后的窗与影子",
  ];
  const authors = ["Aria Chen", "Noah Berg", "Mika Sato", "Lena Volkov", "Ethan Cole", "Sofia Marin"];

  const photos = Array.from({ length: 24 }, (_, i) => ({
    id: `mock-${i}`,
    description: titles[i % titles.length],
    width: 6000 - i * 90,
    height: 4000 - i * 40,
    color: ["#30405c", "#4a5d7e", "#7c5a4a", "#3f5f4f", "#5b4a6b"][i % 5],
    author: authors[i % authors.length],
    authorUrl: "https://unsplash.com",
    thumbUrl: img(`ws${i}`, 720, 450),
    previewUrl: img(`ws${i}`, 1600, 1000),
    fullUrl: img(`ws${i}`, 3840, 2400),
    unsplashUrl: "https://unsplash.com",
    downloadLocation: null,
  }));

  const settings = {
    unsplashKey: "demo-key-000000000000000000000",
    downloadDir: "C:\\Users\\Public\\Pictures\\Wallpapers",
    defaultFit: "cover",
    jpegQuality: 92,
    keepLibraryCopy: true,
    defaultLockscreen: true,
    autoPrune: true,
  };

  const history = [0, 1, 2].map((i) => ({
    id: `mock-history-${i}`,
    title: titles[i],
    source: i === 1 ? "local" : "unsplash",
    origin: i === 1 ? "C:\\Users\\Public\\Pictures\\sample.jpg" : "https://unsplash.com",
    wallpaperPath: "C:\\Users\\Public\\Pictures\\Wallpapers\\demo.jpg",
    thumbPath: null,
    appliedAt: Date.now() - i * 3600_000 * 5,
    fit: i === 1 ? "blur" : "cover",
    resolution: "2560 × 1440",
    lockTarget: i === 0,
    thumb: img(`hist${i}`, 420, 263),
  }));

  const delay = (value, ms = 320) => new Promise((resolve) => setTimeout(() => resolve(value), ms));

  return async function mockInvoke(command, args = {}) {
    switch (command) {
      case "bootstrap":
        return delay({
          settings,
          screen: { width: 2560, height: 1440, virtualWidth: 2560, virtualHeight: 1440, monitors: 1, isWindows: true },
          isAdmin: false,
          history,
          version: "2.0.0",
          cacheBytes: 128_400_000,
          libraryCount: 12,
          system: {
            os: "Windows 11 家庭中文版 (Build 26200)",
            webview: "Microsoft Edge WebView2",
            defaultWallpaper: "C:\\Windows\\Web\\Wallpaper\\Windows\\img0.jpg",
            lockscreenDir: "C:\\Windows\\Web\\Screen",
          },
        });

      case "search_photos": {
        const page = args.page || 1;
        return delay(
          {
            query: args.query,
            page,
            total: 240,
            totalPages: 10,
            photos: photos.map((photo, index) => ({
              ...photo,
              // 不同分页使用不同 id，模拟真实接口的去重行为。
              id: `${photo.id}-p${page}-${index}`,
            })),
          },
          page === 1 ? 800 : 420
        );
      }

      case "preview_image":
        return delay({
          dataUrl: img("local-preview", 1600, 1000),
          path: args.path || "C:\\Users\\Public\\Pictures\\sample.jpg",
          name: "sample.jpg",
          width: 6000,
          height: 4000,
          size: 8_400_000,
        });

      case "pick_image":
        return delay({
          dataUrl: img("local-preview", 1600, 1000),
          path: "C:\\Users\\Public\\Pictures\\sample.jpg",
          name: "sample.jpg",
          width: 6000,
          height: 4000,
          size: 8_400_000,
        }, 500);

      case "pick_folder":
        return delay(settings.downloadDir, 400);

      case "apply_background": {
        const request = args.request || {};
        const source = request.source || {};
        const entry = {
          id: `applied-${Date.now()}`,
          title: source.title || "示例图片",
          source: source.kind === "remote" ? "unsplash" : "local",
          origin: source.url || source.path || "",
          wallpaperPath: "C:\\Users\\Public\\Pictures\\Wallpapers\\demo.jpg",
          thumbPath: null,
          appliedAt: Date.now(),
          fit: request.fit || "cover",
          resolution: "2560 × 1440",
          lockTarget: Boolean(request.lockscreen),
          thumb: img(`applied${Date.now()}`, 420, 263),
        };
        return delay(
          {
            ok: true,
            needsAdmin: Boolean(request.lockscreen),
            wallpaper: request.wallpaper
              ? { success: true, message: "桌面壁纸已更新（2560 × 1440）", hint: null }
              : null,
            lockscreen: request.lockscreen
              ? { success: true, message: "锁屏图像已写入系统目录", hint: "按 Win + L 预览满屏效果。" }
              : null,
            renderedPath: "C:\\Users\\Public\\Pictures\\Wallpapers\\demo.jpg",
            message:
              request.wallpaper && request.lockscreen
                ? "壁纸与锁屏已同时更新"
                : request.lockscreen
                  ? "锁屏已更新"
                  : "壁纸已更新",
            history: entry,
          },
          900
        );
      }

      case "refresh_history":
        return delay(state.history);

      case "remove_history":
        return delay(null, 160);

      case "save_settings":
        return delay(args.settings, 240);

      case "clear_cache":
        return delay(128_400_000, 500);

      case "reset_wallpaper":
        return delay({ success: true, message: "已恢复 Windows 默认壁纸。", hint: null });

      case "reset_lockscreen":
        return delay({ success: true, message: "已移除自定义锁屏配置。", hint: null });

      case "screen_info":
        return delay({ width: 2560, height: 1440, virtualWidth: 2560, virtualHeight: 1440, monitors: 1, isWindows: true }, 60);

      case "reveal_path":
      case "open_url":
      case "relaunch_admin":
        return delay(null, 120);

      default:
        return delay(null);
    }
  };
}

/* ---------- bootstrap ---------- */

async function boot() {
  bindWindowControls();
  bindNav();
  bindSearch();
  bindLocal();
  bindSettings();
  bindHistory();
  bindKeyboard();
  bindDragDrop();
  syncMaxIcon();

  if (!invoke) {
    invoke = createMockBridge();
    state.mock = true;
    if (!new URLSearchParams(location.search).has("static")) {
      setTimeout(
        () => toast("warn", "演示模式", "未检测到 Tauri 运行时，当前使用内置示例数据预览界面。"),
        700
      );
    }
  }

  try {
    const data = await invoke("bootstrap");
    applyBootstrap(data);
  } catch (err) {
    toast("error", "初始化失败", String(err));
    return;
  }

  state.ready = true;
  await runSearch(state.query, 1, true);
  applyPreviewParams();
}

// 仅用于脱离 Tauri 的界面预览（?mock=1&page=settings&modal=1&lock=1）。
function applyPreviewParams() {
  const params = new URLSearchParams(location.search);
  const page = params.get("page");
  if (page && ["explore", "local", "library", "settings"].includes(page)) setPage(page);

  if (params.get("modal") === "1" && state.photos[0]) {
    setTimeout(() => {
      openPhoto(state.photos[0]);
      if (params.get("lock") === "1") {
        state.lockSim = true;
        renderModal();
      }
    }, 300);
  }

  if (params.get("toast") === "1") {
    setTimeout(
      () =>
        toast(
          "success",
          "壁纸与锁屏已同时更新",
          "桌面壁纸已更新（2560 × 1440） · 锁屏图像已写入 C:\\Windows\\Web\\Screen"
        ),
      600
    );
  }
}

function applyBootstrap(data) {
  state.settings = data.settings;
  state.screen = data.screen;
  state.isAdmin = data.isAdmin;
  state.history = data.history || [];
  state.version = data.version;
  state.system = data.system || {};
  state.cacheBytes = data.cacheBytes || 0;
  state.libraryCount = data.libraryCount || 0;

  if (state.settings?.defaultFit) state.fit = state.settings.defaultFit;
  state.targets = {
    wallpaper: true,
    lockscreen: Boolean(state.settings?.defaultLockscreen),
  };

  fillSettings();
  renderHistory();
  renderLocalHistory();
  updateStatus();
}

function updateStatus() {
  const screen = state.screen || {};
  const screenText = `${screen.width} × ${screen.height}`;

  $("#screenPill").innerHTML = `${icon("i-monitor")}<span>${screenText}</span>`;
  $("#localScreenPill").innerHTML = `${icon("i-monitor")}<span>${screenText}</span>`;
  $("#renderPill").textContent = `${screen.monitors || 1} 屏 · 满屏渲染`;

  const card = $("#permissionCard");
  card?.classList.toggle("is-admin", state.isAdmin);
  $("#permissionTitle").textContent = state.isAdmin ? "管理员权限已启用" : "标准用户权限";
  $("#permissionText").textContent = state.isAdmin
    ? "锁屏图像可写入系统目录与注册表，设置立即生效。"
    : "锁屏图像需要管理员权限才能写入系统目录与 HKLM 注册表。";
  $("#elevateBtn").style.display = state.isAdmin ? "none" : "";

  const badge = $("#adminBadge");
  badge.textContent = state.isAdmin ? "已提权" : "标准权限";
  badge.className = `badge ${state.isAdmin ? "ok" : "warn"}`;

  $("#adminState").textContent = state.isAdmin ? "以管理员身份运行" : "标准用户（锁屏可能受限）";
  $("#lockDir").textContent = state.system.lockscreenDir || "—";
  $("#defaultWallpaper").textContent = state.system.defaultWallpaper || "—";
  $("#osInfo").textContent = state.system.os || "—";
  $("#versionText").textContent = `v${state.version}`;
  $("#aboutVersion").textContent = `v${state.version}`;
  $("#navCount").textContent = state.history.length;
  $("#libraryCount").textContent = String(state.libraryCount ?? 0);
  $("#cacheSize").textContent = fmtBytes(state.cacheBytes);
  $("#monitorInfo").textContent = `${screen.monitors || 1} 台`;

  const localHistory = state.history.filter((item) => item.source === "local");
  $("#localMeta").textContent = localHistory.length
    ? `最近 ${localHistory.length} 张`
    : "暂无记录";
}

/* ---------- search & explore ---------- */

function bindSearch() {
  const input = $("#searchInput");
  const button = $("#searchBtn");

  const submit = () => {
    const value = input.value.trim();
    if (!value) {
      toast("warn", "请输入关键词", "例如：山脉、极光、城市夜景。");
      input.focus();
      return;
    }
    runSearch(value, 1, true);
  };

  button.addEventListener("click", submit);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") submit();
  });

  $("#chipRow").addEventListener("click", (event) => {
    const chip = event.target.closest(".chip");
    if (!chip) return;
    $$(".chip").forEach((c) => c.classList.toggle("is-active", c === chip));
    input.value = "";
    runSearch(chip.dataset.query, 1, true);
  });

  $("#loadMoreBtn").addEventListener("click", () => {
    if (state.searching || state.loadingMore) return;
    runSearch(state.query, state.page + 1, false);
  });

  $("#resultsGrid").addEventListener("click", (event) => {
    const card = event.target.closest(".photo-card");
    if (!card) return;
    const photo = state.photos.find((p) => p.id === card.dataset.id);
    if (!photo) return;
    openPhoto(photo);
  });

  $("#resultsGrid").addEventListener("keydown", (event) => {
    if (event.key !== "Enter") return;
    const card = event.target.closest(".photo-card");
    if (!card) return;
    const photo = state.photos.find((p) => p.id === card.dataset.id);
    if (photo) openPhoto(photo);
  });
}

async function runSearch(query, page, reset) {
  if (state.searching) return;
  state.searching = true;
  state.query = query;

  const button = $("#searchBtn");
  setBusy(button, true, "搜索中");

  if (reset) {
    state.page = 1;
    state.photos = [];
    state.totalPages = 1;
    shelfSkeletons();
    $("#resultsMeta").textContent = "正在检索 Unsplash…";
    $("#loadMoreWrap").hidden = true;
  } else {
    state.loadingMore = true;
    const more = $("#loadMoreBtn");
    setBusy(more, true, "加载中");
  }

  try {
    const result = await invoke("search_photos", {
      query,
      page,
      perPage: 24,
    });

    state.photos = reset ? result.photos : [...state.photos, ...result.photos];
    state.page = result.page;
    state.totalPages = result.totalPages || 1;
    state.total = result.total || state.photos.length;

    renderResults();
    $("#resultsMeta").textContent = `${nf.format(state.total)} 张作品 · 第 ${state.page} / ${state.totalPages} 页`;
    $("#loadMoreWrap").hidden = state.page >= state.totalPages;

    if (!state.photos.length) {
      renderEmptyResult("没有找到匹配的作品", "换一个关键词试试，例如「雾林」「海岸线」「极简」。");
    }
  } catch (err) {
    $("#resultsMeta").textContent = "搜索失败";
    if (reset) renderEmptyResult("搜索失败", String(err), "i-image-off");
    toast("error", "搜索失败", String(err));
  } finally {
    state.searching = false;
    state.loadingMore = false;
    setBusy(button, false);
    setBusy($("#loadMoreBtn"), false);
    $("#loadMoreWrap").hidden = state.page >= state.totalPages || state.photos.length === 0;
  }
}

function shelfSkeletons(count = 12) {
  const grid = $("#resultsGrid");
  grid.innerHTML = Array.from({ length: count })
    .map(
      () => `<div class="skeleton-card">
        <div class="sk sk-thumb"></div>
        <div class="sk sk-line"></div>
        <div class="sk sk-line short"></div>
      </div>`
    )
    .join("");
}

function renderEmptyResult(title, desc, iconId = "i-image-off") {
  $("#resultsGrid").innerHTML = emptyBlock(title, desc, iconId);
}

function renderResults() {
  const grid = $("#resultsGrid");
  if (!state.photos.length) {
    grid.innerHTML = emptyBlock("没有匹配的作品", "试试更通用的关键词。");
    return;
  }
  grid.innerHTML = state.photos.map(photoCard).join("");
  grid.querySelectorAll(".photo-card").forEach((card, index) => {
    card.style.animationDelay = `${Math.min(index * 18, 320)}ms`;
  });
}

function photoCard(photo) {
  const title = photo.description || "Unsplash 摄影作品";
  return `<article class="photo-card" data-id="${esc(photo.id)}" tabindex="0">
    <div class="thumb" style="background-image:url('${esc(photo.thumbUrl)}')">
      <span class="thumb-badge">${photo.width} × ${photo.height}</span>
      <span class="thumb-src">${icon("i-compass")}Unsplash</span>
      <div class="card-actions">
        <button class="icon-btn" data-act="preview">${icon("i-search")}预览</button>
        <button class="icon-btn" data-act="apply">${icon("i-sparkles")}应用</button>
      </div>
    </div>
    <div class="card-body">
      <span class="color-dot" style="background:${esc(photo.color || "#6d7cff")}"></span>
      <div class="card-text">
        <span class="card-title">${esc(title)}</span>
        <span class="card-sub">${esc(photo.author)} · ${nf.format(photo.width)}px</span>
      </div>
      <button class="card-apply" data-act="apply" title="应用这张图">${icon("i-arrow-right")}</button>
    </div>
  </article>`;
}

/* ---------- local images ---------- */

function bindLocal() {
  $("#pickLocalBtn").addEventListener("click", pickLocalFile);
  $("#dropzone").addEventListener("click", (event) => {
    if (event.target.closest("button")) return;
    pickLocalFile();
  });

  $("#localGrid").addEventListener("click", (event) => {
    const card = event.target.closest(".photo-card");
    if (!card) return;
    const entry = state.history.find((item) => item.id === card.dataset.id);
    if (entry) openHistory(entry);
  });
}

async function pickLocalFile() {
  const button = $("#pickLocalBtn");
  setBusy(button, true, "选择中…");
  try {
    const image = await invoke("pick_image", { maxEdge: 1800 });
    if (image) openLocal(image);
  } catch (err) {
    toast("error", "无法读取图片", String(err));
  } finally {
    setBusy(button, false);
  }
}

function renderLocalHistory() {
  const grid = $("#localGrid");
  if (!grid) return;
  const items = state.history.filter((item) => item.source === "local").slice(0, 8);

  if (!items.length) {
    grid.innerHTML = emptyBlock(
      "还没有导入过本地图片",
      "拖一张照片进来，程序会立即渲染为当前屏幕尺寸的成图。",
      "i-image"
    );
    return;
  }

  grid.innerHTML = items
    .map(
      (item) => `<article class="photo-card" data-id="${esc(item.id)}" tabindex="0">
      <div class="thumb" style="background-image:url('${item.thumb || ""}')">
        <span class="thumb-badge">${esc(item.resolution)}</span>
        <span class="thumb-src">${icon("i-image")}本地</span>
        <div class="card-actions">
          <button class="icon-btn" data-act="preview">${icon("i-search")}查看</button>
        </div>
      </div>
      <div class="card-body">
        <div class="card-text">
          <span class="card-title">${esc(item.title)}</span>
          <span class="card-sub">${fmtTime(item.appliedAt)} · ${fitLabel(item.fit)}</span>
        </div>
        <button class="card-apply" title="打开">${icon("i-arrow-right")}</button>
      </div>
    </article>`
    )
    .join("");
}

/* ---------- history ---------- */

function bindHistory() {
  $("#refreshHistory").addEventListener("click", () => refreshHistory(false));
  $("#openLibrary").addEventListener("click", () => {
    const dir = state.settings?.downloadDir;
    if (dir) invoke("reveal_path", { path: dir }).catch((err) => toast("error", "无法打开目录", String(err)));
  });

  $("#historyList").addEventListener("click", async (event) => {
    const item = event.target.closest(".history-item");
    if (!item) return;
    const entry = state.history.find((h) => h.id === item.dataset.id);
    if (!entry) return;
    const action = event.target.closest("[data-act]")?.dataset.act;

    if (action === "open-item") {
      invoke("reveal_path", { path: entry.wallpaperPath }).catch((err) =>
        toast("error", "无法打开位置", String(err))
      );
    } else if (action === "delete") {
      const button = event.target.closest("[data-act]");
      if (button.dataset.confirm !== "1") {
        button.dataset.confirm = "1";
        button.innerHTML = `${icon("i-check")}确认`;
        setTimeout(() => {
          if (!button.isConnected) return;
          button.dataset.confirm = "";
          button.innerHTML = icon("i-trash");
        }, 3200);
        return;
      }
      try {
        await invoke("remove_history", { id: entry.id });
        state.history = state.history.filter((h) => h.id !== entry.id);
        renderHistory();
        renderLocalHistory();
        updateStatus();
        toast("success", "已删除记录", entry.title);
      } catch (err) {
        toast("error", "删除失败", String(err));
      }
    } else {
      openHistory(entry);
    }
  });
}

async function refreshHistory(silent) {
  try {
    const history = await invoke("refresh_history");
    state.history = history;
    renderHistory();
    renderLocalHistory();
    updateStatus();
    if (!silent) toast("success", "记录已刷新");
  } catch (err) {
    if (!silent) toast("error", "刷新失败", String(err));
  }
}

function renderHistory() {
  const list = $("#historyList");
  if (!list) return;

  if (!state.history.length) {
    list.innerHTML = emptyBlock(
      "还没有应用记录",
      "应用任意图片后，这里会保留屏幕尺寸的成图，方便随时换回。",
      "i-layers"
    );
    return;
  }

  list.innerHTML = state.history
    .map(
      (item) => `<div class="history-item" data-id="${esc(item.id)}">
      <div class="history-thumb" style="background-image:url('${item.thumb || ""}')"></div>
      <div class="history-main">
        <div class="history-title">
          ${esc(item.title)}
          <span class="tag ${item.source === "local" ? "local" : ""}">${item.source === "local" ? "本地" : "Unsplash"}</span>
          ${item.lockTarget ? `<span class="tag lock">含锁屏</span>` : ""}
        </div>
        <div class="history-meta">
          <span><b>${esc(item.resolution)}</b></span>
          <span>${fitLabel(item.fit)}</span>
          <span>${fmtTime(item.appliedAt)}</span>
        </div>
      </div>
      <div class="history-actions">
        <button class="btn btn-ghost sm" data-act="use">${icon("i-refresh")}再次使用</button>
        <button class="btn btn-ghost sm" data-act="open-item" title="打开成图位置">${icon("i-folder")}</button>
        <button class="btn btn-ghost sm" data-act="delete" title="删除记录">${icon("i-trash")}</button>
      </div>
    </div>`
    )
    .join("");
}

/* ---------- modal ---------- */

function openPhoto(photo) {
  state.modal = {
    kind: "remote",
    photo,
    preview: photo.previewUrl,
    title: photo.description || "Unsplash 摄影作品",
    author: photo.author,
    authorUrl: photo.authorUrl,
    meta: `${photo.width} × ${photo.height}`,
    source: "Unsplash",
    external: photo.unsplashUrl,
    path: null,
  };
  state.fit = state.settings?.defaultFit || "cover";
  state.targets = {
    wallpaper: true,
    lockscreen: Boolean(state.settings?.defaultLockscreen),
  };
  state.lockSim = false;
  renderModal();
}

function openLocal(image) {
  state.modal = {
    kind: "local",
    image,
    preview: image.dataUrl,
    title: image.name,
    author: "本地文件",
    meta: `${image.width} × ${image.height}`,
    source: "本地",
    size: image.size,
    path: image.path,
    external: null,
  };
  state.fit = state.settings?.defaultFit || "cover";
  state.targets = {
    wallpaper: true,
    lockscreen: Boolean(state.settings?.defaultLockscreen),
  };
  state.lockSim = false;
  renderModal();
}

function openHistory(entry) {
  state.modal = {
    kind: "history",
    entry,
    preview: entry.thumb || "",
    title: entry.title,
    author: entry.source === "local" ? "本地文件" : "Unsplash",
    meta: entry.resolution,
    source: entry.source === "local" ? "本地" : "Unsplash",
    path: entry.wallpaperPath,
    external: null,
  };
  state.fit = entry.fit || state.settings?.defaultFit || "cover";
  state.targets = { wallpaper: true, lockscreen: false };
  state.lockSim = false;
  renderModal();

  // 历史缩略图只有 420px，异步换成屏幕尺寸的预览。
  if (entry.wallpaperPath) {
    invoke("preview_image", { path: entry.wallpaperPath, maxEdge: 1800 })
      .then((image) => {
        if (!image?.dataUrl) return;
        if (state.modal?.kind !== "history" || state.modal.entry?.id !== entry.id) return;
        state.modal.preview = image.dataUrl;
        const stageImage = $("#modalRoot .modal-stage > img");
        if (stageImage) stageImage.src = image.dataUrl;
        const lock = $("#modalRoot .lock-sim");
        if (lock) lock.style.backgroundImage = `url('${image.dataUrl}')`;
      })
      .catch(() => {});
  }
}

function closeModal() {
  clearInterval(state.clockTimer);
  state.clockTimer = null;
  state.modal = null;
  state.lockSim = false;
  $("#modalRoot").innerHTML = "";
}

function modalSourceBadges() {
  const modal = state.modal;
  if (!modal) return "";
  const parts = [`<span class="tag ${modal.kind === "local" ? "local" : ""}">${esc(modal.source)}</span>`];
  if (modal.kind === "remote") parts.push(`<span class="tag">${icon("i-compass")}在线</span>`);
  return parts.join("");
}

function lockscreenMock(preview) {
  return `<div class="lock-sim" style="background-image:url('${preview}')">
    <div>
      <div class="lock-clock" id="lockClock">--:--</div>
      <div class="lock-date" id="lockDate">正在读取时间…</div>
    </div>
    <div class="lock-bottom">
      <span class="avatar">WS</span>
      <span>按 Win + L 在真实锁屏上查看效果</span>
    </div>
  </div>`;
}

function startClock() {
  clearInterval(state.clockTimer);
  const tick = () => {
    const now = new Date();
    const clock = $("#lockClock");
    const date = $("#lockDate");
    if (!clock || !date) return;
    clock.textContent = now.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
    const monthDay = now.toLocaleDateString("zh-CN", { month: "long", day: "numeric" });
    const weekday = now.toLocaleDateString("zh-CN", { weekday: "long" });
    date.textContent = `${monthDay} · ${weekday}`;
  };
  tick();
  state.clockTimer = setInterval(tick, 1000);
}

function renderModal() {
  const modal = state.modal;
  if (!modal) return;

  const isRemote = modal.kind === "remote";
  const metaParts = [modal.meta, `${state.screen.width} × ${state.screen.height} 成图`];
  if (modal.size) metaParts.push(fmtBytes(modal.size));

  const fitButtons = FIT_OPTIONS.map(
    (item) => `<button class="seg ${state.fit === item.value ? "is-active" : ""}" data-fit="${item.value}" title="${item.hint}">${item.label}</button>`
  ).join("");

  $("#modalRoot").innerHTML = `
    <div class="modal-backdrop" id="modalBackdrop">
      <div class="modal" role="dialog" aria-modal="true">
        <button class="modal-close" id="modalClose" title="关闭（Esc）">${icon("i-x")}</button>

        <div class="modal-stage">
          <div class="stage-toolbar">
            <button class="stage-chip ${state.lockSim ? "is-on" : ""}" id="lockSimBtn">
              ${icon("i-lock")}${state.lockSim ? "返回预览" : "锁屏预览"}
            </button>
          </div>
          <img src="${modal.preview}" alt="${esc(modal.title)}" draggable="false" />
          ${state.lockSim ? lockscreenMock(modal.preview) : ""}
        </div>

        <aside class="modal-panel">
          <div class="panel-eyebrow">${modalSourceBadges()}<span>${isRemote ? "UNSPLASH PHOTO" : "LOCAL IMAGE"}</span></div>
          <h2 class="panel-title">${esc(modal.title)}</h2>
          <div class="panel-author">${modal.authorUrl ? `<button class="link" id="authorLink">${esc(modal.author)} ${icon("i-external")}</button>` : `摄影 / 作者 <b>${esc(modal.author)}</b>`}</div>

          <div class="panel-meta">
            ${metaParts.map((m) => `<span class="pill">${esc(m)}</span>`).join("")}
          </div>

          <div class="panel-block">
            <div class="panel-label"><span>显示方式</span><em>${FIT_OPTIONS.find((f) => f.value === state.fit)?.hint || ""}</em></div>
            <div class="segmented" id="fitSegmented">${fitButtons}</div>
          </div>

          <div class="panel-block">
            <div class="panel-label"><span>应用目标</span><em>可多选</em></div>
            <div class="targets">
              <button class="target ${state.targets.wallpaper ? "is-on" : ""}" data-target="wallpaper">
                <span class="target-icon">${icon("i-monitor")}</span>
                <span class="target-text"><strong>桌面壁纸</strong><span>按屏幕比例满屏渲染</span></span>
                <span class="target-check">${icon("i-check")}</span>
              </button>
              <button class="target ${state.targets.lockscreen ? "is-on" : ""}" data-target="lockscreen">
                <span class="target-icon">${icon("i-lock")}</span>
                <span class="target-text"><strong>锁屏图像</strong><span>同时关闭聚焦轮播</span></span>
                <span class="target-check">${icon("i-check")}</span>
              </button>
            </div>
          </div>

          ${targetNoteHTML()}

          <div class="apply-row">
            <button class="btn btn-primary btn-apply" id="applyBtn">
              ${icon("i-sparkles")}<span class="btn-label">${applyLabel()}</span>
            </button>
            <div class="panel-links">
              ${
                modal.external
                  ? `<button class="link" id="externalLink">${icon("i-external")}在 Unsplash 查看</button>`
                  : ""
              }
              ${
                modal.path
                  ? `<button class="link" id="revealLink">${icon("i-folder")}打开所在位置</button>`
                  : ""
              }
            </div>
          </div>
        </aside>
      </div>
    </div>`;

  bindModalEvents();
  if (state.lockSim) startClock();
}

function targetNote() {
  if (!state.targets.lockscreen) {
    return { cls: "neutral", icon: "i-sliders", text: "仅更新桌面壁纸，不影响锁屏画面。" };
  }
  if (!state.isAdmin) {
    return {
      cls: "",
      icon: "i-info",
      text: "当前为标准权限，锁屏写入可能被系统拒绝。可先在「偏好设置」中以管理员身份重启。",
    };
  }
  return {
    cls: "",
    icon: "i-info",
    text: "锁屏设置会固定为这张图，并停用 Windows 聚焦与轮播。",
  };
}

function targetNoteHTML() {
  const note = targetNote();
  return `<div class="panel-note ${note.cls}" id="targetNote">${icon(note.icon)}<span>${note.text}</span></div>`;
}

function applyLabel() {
  const { wallpaper, lockscreen } = state.targets;
  if (wallpaper && lockscreen) return "应用到桌面 + 锁屏";
  if (lockscreen) return "设为锁屏图像";
  if (wallpaper) return "设为桌面壁纸";
  return "请选择应用目标";
}

function bindModalEvents() {
  $("#modalClose").addEventListener("click", closeModal);
  $("#modalBackdrop").addEventListener("click", (event) => {
    if (event.target.id === "modalBackdrop") closeModal();
  });

  $("#lockSimBtn").addEventListener("click", () => {
    state.lockSim = !state.lockSim;
    renderModal();
  });

  $$("#fitSegmented [data-fit]").forEach((button) => {
    button.addEventListener("click", () => {
      state.fit = button.dataset.fit;
      $$("#fitSegmented [data-fit]").forEach((b) =>
        b.classList.toggle("is-active", b.dataset.fit === state.fit)
      );
      const hint = $("#fitSegmented").previousElementSibling?.querySelector("em");
      if (hint) hint.textContent = FIT_OPTIONS.find((f) => f.value === state.fit)?.hint || "";
    });
  });

  $$(".target").forEach((button) => {
    button.addEventListener("click", () => {
      const key = button.dataset.target;
      state.targets[key] = !state.targets[key];
      button.classList.toggle("is-on", state.targets[key]);
      $("#applyBtn .btn-label").textContent = applyLabel();

      const note = targetNote();
      const noteEl = $("#targetNote");
      if (noteEl) {
        noteEl.className = `panel-note ${note.cls}`;
        noteEl.innerHTML = `${icon(note.icon)}<span>${note.text}</span>`;
      }
    });
  });

  $("#applyBtn").addEventListener("click", applyFromModal);

  $("#authorLink")?.addEventListener("click", () => {
    const url = state.modal?.authorUrl;
    if (url) invoke("open_url", { url }).catch((err) => toast("error", "无法打开链接", String(err)));
  });

  $("#externalLink")?.addEventListener("click", () => {
    const url = state.modal?.external;
    if (url) invoke("open_url", { url }).catch((err) => toast("error", "无法打开链接", String(err)));
  });

  $("#revealLink")?.addEventListener("click", () => {
    const path = state.modal?.path;
    if (path)
      invoke("reveal_path", { path }).catch((err) => toast("error", "无法打开位置", String(err)));
  });
}

function buildSource(modal) {
  if (modal.kind === "remote") {
    return {
      kind: "remote",
      id: modal.photo.id,
      url: modal.photo.fullUrl,
      title: modal.title,
      downloadLocation: modal.photo.downloadLocation,
    };
  }
  if (modal.kind === "local") {
    return { kind: "local", path: modal.image.path, title: modal.image.name };
  }
  return {
    kind: "history",
    id: modal.entry.id,
    path: modal.entry.wallpaperPath,
    title: modal.entry.title,
    origin: modal.entry.origin,
  };
}

async function applyFromModal() {
  const modal = state.modal;
  if (!modal) return;

  if (!state.targets.wallpaper && !state.targets.lockscreen) {
    toast("warn", "请选择应用目标", "至少勾选桌面壁纸或锁屏图像。");
    return;
  }

  const button = $("#applyBtn");
  setBusy(button, true, "正在渲染满屏成图…");

  try {
    const report = await invoke("apply_background", {
      request: {
        source: buildSource(modal),
        fit: state.fit,
        wallpaper: state.targets.wallpaper,
        lockscreen: state.targets.lockscreen,
        quality: state.settings?.jpegQuality ?? 92,
      },
    });

    if (report.history) {
      state.history = [
        report.history,
        ...state.history.filter((item) => item.id !== report.history.id),
      ].slice(0, 80);
      renderHistory();
      renderLocalHistory();
      state.libraryCount = state.libraryCount + 1;
      updateStatus();
    }

    const steps = [report.wallpaper, report.lockscreen].filter(Boolean);
    if (report.ok) {
      toast(
        "success",
        report.message,
        steps.map((step) => step.message).join(" · "),
        null
      );
      setTimeout(closeModal, 720);
    } else {
      const failed = steps.find((step) => !step.success);
      toast("error", report.message, failed?.message || "请检查系统权限。");
    }

    if (report.needsAdmin && !state.isAdmin) {
      setTimeout(() => {
        toast("warn", "锁屏需要管理员权限", report.lockscreen?.hint || "以管理员身份重启后再试。", {
          label: "以管理员身份重启",
          onClick: elevate,
        });
      }, 400);
    }
  } catch (err) {
    toast("error", "应用失败", String(err));
  } finally {
    setBusy(button, false);
  }
}

/* ---------- settings ---------- */

function bindSettings() {
  const quality = $("#qualityRange");
  quality.addEventListener("input", () => {
    $("#qualityValue").textContent = quality.value;
    scheduleSave();
  });

  $("#fitSelect").addEventListener("change", scheduleSave);
  $("#switchLockscreen").addEventListener("change", scheduleSave);
  $("#switchKeep").addEventListener("change", scheduleSave);
  $("#keyInput").addEventListener("change", scheduleSave);

  $("#toggleKey").addEventListener("click", () => {
    const input = $("#keyInput");
    const show = input.type === "password";
    input.type = show ? "text" : "password";
    $("#toggleKey").textContent = show ? "隐藏" : "显示";
  });

  $("#chooseDirBtn").addEventListener("click", async () => {
    try {
      const dir = await invoke("pick_folder");
      if (dir) {
        $("#downloadDirInput").value = dir;
        scheduleSave();
      }
    } catch (err) {
      toast("error", "选择目录失败", String(err));
    }
  });

  $("#openDirBtn").addEventListener("click", () => {
    const dir = state.settings?.downloadDir;
    if (dir) invoke("reveal_path", { path: dir }).catch((err) => toast("error", "无法打开目录", String(err)));
  });

  $("#clearCacheBtn").addEventListener("click", async () => {
    try {
      const freed = await invoke("clear_cache");
      state.cacheBytes = 0;
      $("#cacheSize").textContent = fmtBytes(0);
      toast("success", "缓存已清理", `释放 ${fmtBytes(freed)} 空间。`);
    } catch (err) {
      toast("error", "清理失败", String(err));
    }
  });

  $("#elevateBtn").addEventListener("click", elevate);
  $("#elevateBtn2").addEventListener("click", elevate);

  $("#resetWallBtn").addEventListener("click", async () => {
    try {
      const step = await invoke("reset_wallpaper");
      toast(step.success ? "success" : "error", step.message, step.hint || "");
    } catch (err) {
      toast("error", "恢复失败", String(err));
    }
  });

  $("#resetLockBtn").addEventListener("click", async () => {
    try {
      const step = await invoke("reset_lockscreen");
      toast(step.success ? "success" : "error", step.message, step.hint || "");
    } catch (err) {
      toast("error", "恢复失败", String(err));
    }
  });
}

function fillSettings() {
  const settings = state.settings;
  if (!settings) return;
  $("#keyInput").value = settings.unsplashKey || "";
  $("#fitSelect").value = settings.defaultFit || "cover";
  $("#qualityRange").value = settings.jpegQuality ?? 92;
  $("#qualityValue").textContent = settings.jpegQuality ?? 92;
  $("#switchLockscreen").checked = Boolean(settings.defaultLockscreen);
  $("#switchKeep").checked = Boolean(settings.keepLibraryCopy);
  $("#downloadDirInput").value = settings.downloadDir || "";
}

function scheduleSave() {
  clearTimeout(state.saveTimer);
  state.saveTimer = setTimeout(saveSettings, 450);
}

async function saveSettings(silent = true) {
  const settings = {
    ...state.settings,
    unsplashKey: $("#keyInput").value.trim() || state.settings.unsplashKey,
    downloadDir: $("#downloadDirInput").value.trim() || state.settings.downloadDir,
    defaultFit: $("#fitSelect").value,
    jpegQuality: Number($("#qualityRange").value),
    defaultLockscreen: $("#switchLockscreen").checked,
    keepLibraryCopy: $("#switchKeep").checked,
    autoPrune: true,
  };

  try {
    const saved = await invoke("save_settings", { settings });
    state.settings = saved;
    if (!silent) toast("success", "设置已保存");
    if (saved.defaultFit && !state.modal) state.fit = saved.defaultFit;
  } catch (err) {
    toast("error", "保存设置失败", String(err));
  }
}

async function elevate() {
  try {
    await invoke("relaunch_admin");
  } catch (err) {
    toast("error", "提权失败", String(err));
  }
}

/* ---------- drag & drop, keyboard ---------- */

function bindDragDrop() {
  const overlay = $("#dropOverlay");
  if (!appWindow?.onDragDropEvent) return;

  try {
    appWindow.onDragDropEvent(async (event) => {
      const payload = event?.payload;
      if (!payload) return;

      if (payload.type === "enter" || payload.type === "over") {
        overlay.classList.add("is-visible");
        return;
      }
      if (payload.type === "leave") {
        overlay.classList.remove("is-visible");
        return;
      }
      if (payload.type === "drop") {
        overlay.classList.remove("is-visible");
        const path = payload.paths?.[0];
        if (!path) return;
        setPage("local");
        try {
          const image = await invoke("preview_image", { path, maxEdge: 1800 });
          openLocal(image);
        } catch (err) {
          toast("error", "无法读取这张图片", String(err));
        }
      }
    });
  } catch (err) {
    console.warn("drag drop unavailable", err);
  }
}

function bindKeyboard() {
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && state.modal) {
      closeModal();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "o") {
      event.preventDefault();
      setPage("local");
      pickLocalFile();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      setPage("explore");
      $("#searchInput").focus();
      $("#searchInput").select();
      return;
    }
    if (event.ctrlKey || event.metaKey) {
      const views = { "1": "explore", "2": "local", "3": "library", "4": "settings" };
      const view = views[event.key];
      if (view) {
        event.preventDefault();
        setPage(view);
      }
    }
  });
}

/* ---------- go ---------- */

document.addEventListener("DOMContentLoaded", boot);
