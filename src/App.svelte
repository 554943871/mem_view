<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
  import MarkdownIt from "markdown-it";
  import mermaid from "mermaid";
  import { onDestroy, onMount, tick } from "svelte";

  type TreeNode = {
    id: string;
    title: string;
    path: string | null;
    kind: string;
    children: TreeNode[];
  };

  type DocMeta = {
    id: string;
    title: string;
    path: string;
    relative_path: string;
    kind: string;
    has_mermaid: boolean;
  };

  type RepoSnapshot = {
    root_path: string;
    tree: TreeNode[];
    docs: DocMeta[];
    counts: {
      markdown: number;
      mermaid: number;
      requirements: number;
    };
  };

  type Document = {
    id: string;
    title: string;
    path: string;
    relative_path: string;
    kind: string;
    markdown: string;
    has_mermaid: boolean;
    read_chain: Array<{
      label: string;
      title: string;
      path: string;
    }>;
  };

  type FlatNode = TreeNode & { depth: number };
  type DocHeading = { id: string; title: string; level: number };
  type SvgBox = { x: number; y: number; width: number; height: number };
  type MarkdownRenderEnv = { headingCounts: Map<string, number> };
  type ViewType = "repo" | "file";
  type CopyDiagramState = "idle" | "copying" | "copied" | "error";
  type OpenView = {
    id: string;
    type: ViewType;
    title: string;
    path: string;
  };
  type LoadRepoOptions = {
    preserveCurrentDocument?: boolean;
  };
  type OpenDocumentOptions = {
    restoreScrollTop?: number;
  };
  type LinkTarget = { type: ViewType; path: string; anchor: string };
  type Locale = "zh-CN" | "en";
  type StatusKey = "idle" | "loading" | "indexing" | "ready" | "opening" | "error";
  type UpdateState = "idle" | "checking" | "downloading" | "installing";
  type ToastTone = "info" | "error";
  type MessagePack = {
    docs: string;
    diagrams: string;
    refresh: string;
    memoryRepo: string;
    recentRepos: string;
    noRecentRepos: string;
    chooseNewRepo: string;
    openMarkdownFile: string;
    checkUpdate: string;
    updateNow: string;
    updateAvailable: string;
    checkingUpdate: string;
    downloadingUpdate: string;
    installingUpdate: string;
    noUpdate: string;
    updateReady: string;
    updateFailed: string;
    updateVersion: string;
    updateDate: string;
    updateReleaseNotes: string;
    updateNoReleaseNotes: string;
    updateDialogIntro: string;
    updateLater: string;
    installUpdate: string;
    updateProgress: string;
    closeUpdateDialog: string;
    dropMarkdownFile: string;
    dropUnsupported: string;
    closeView: string;
    openViews: string;
    chooseRepoTitle: string;
    chooseFileTitle: string;
    noRepoTitle: string;
    noRepoBody: string;
    noRepoSelected: string;
    chooseRepoFirst: string;
    searchPlaceholder: string;
    memoryFiles: string;
    noMatches: string;
    language: string;
    hide: string;
    info: string;
    outline: string;
    readChain: string;
    noChain: string;
    showSidebar: string;
    hideSidebar: string;
    showDetails: string;
    hideDetails: string;
    file: string;
    kind: string;
    path: string;
    mermaid: string;
    yes: string;
    no: string;
    diagram: string;
    diagramDetail: string;
    diagramViewer: string;
    mermaidDiagram: string;
    zoomOut: string;
    zoomIn: string;
    fitDiagram: string;
    fitShort: string;
    closeDiagram: string;
    enlargeDiagram: string;
    copyDiagram: string;
    copyingDiagram: string;
    copiedDiagram: string;
    copyDiagramFailed: string;
    findDocument: string;
    findPlaceholder: string;
    findPrevious: string;
    findNext: string;
    closeFind: string;
    findNoMatches: string;
    status: Record<StatusKey, string>;
    kinds: Record<string, string>;
    chainLabels: Record<string, string>;
    folderTitles: Record<string, string>;
  };

  const localeStorageKey = "memView.locale";
  const repoPathStorageKey = "memView.repoPath";
  const recentRepoPathsStorageKey = "memView.recentRepoPaths";
  const repoViewId = "repo";
  const fileViewPrefix = "file:";
  const recentRepoLimit = 8;
  const messages: Record<Locale, MessagePack> = {
    "zh-CN": {
      docs: "文档",
      diagrams: "图",
      refresh: "刷新",
      memoryRepo: "记忆库",
      recentRepos: "快捷切换",
      noRecentRepos: "暂无最近打开",
      chooseNewRepo: "打开新记忆库",
      openMarkdownFile: "打开 Markdown 文件",
      checkUpdate: "检查更新",
      updateNow: "更新",
      updateAvailable: "发现新版本",
      checkingUpdate: "正在检查更新",
      downloadingUpdate: "正在下载更新",
      installingUpdate: "正在安装更新",
      noUpdate: "已是最新版本",
      updateReady: "更新已安装，正在重启",
      updateFailed: "更新失败",
      updateVersion: "版本",
      updateDate: "发布日期",
      updateReleaseNotes: "Release Notes",
      updateNoReleaseNotes: "这个版本没有提供 release note。",
      updateDialogIntro: "检测到可用的新版本。确认后会开始下载并安装，完成后自动重启应用。",
      updateLater: "稍后",
      installUpdate: "安装更新",
      updateProgress: "更新进度",
      closeUpdateDialog: "关闭更新弹窗",
      dropMarkdownFile: "松开以打开 Markdown 文件",
      dropUnsupported: "请拖入 .md 文件",
      closeView: "关闭视图",
      openViews: "打开的视图",
      chooseRepoTitle: "选择记忆库目录",
      chooseFileTitle: "打开 Markdown 文件",
      noRepoTitle: "选择一个本地记忆库",
      noRepoBody: "请选择一个 Git 记忆库目录；如果选到仓库内的子目录，memView 会自动打开该 Git 仓库根目录。",
      noRepoSelected: "未选择记忆库",
      chooseRepoFirst: "先选择记忆库",
      searchPlaceholder: "搜索标题或路径",
      memoryFiles: "记忆文件",
      noMatches: "没有匹配",
      language: "语言",
      hide: "隐藏",
      info: "信息",
      outline: "文档目录",
      readChain: "阅读链",
      noChain: "这个文件没有阅读链。",
      showSidebar: "展开左侧栏",
      hideSidebar: "收起左侧栏",
      showDetails: "展开信息栏",
      hideDetails: "收起信息栏",
      file: "文件",
      kind: "类型",
      path: "路径",
      mermaid: "Mermaid",
      yes: "是",
      no: "否",
      diagram: "图",
      diagramDetail: "图详情",
      diagramViewer: "图查看器",
      mermaidDiagram: "Mermaid 图",
      zoomOut: "缩小",
      zoomIn: "放大",
      fitDiagram: "适配图",
      fitShort: "适配",
      closeDiagram: "关闭图",
      enlargeDiagram: "放大图",
      copyDiagram: "复制图片",
      copyingDiagram: "复制中",
      copiedDiagram: "已复制",
      copyDiagramFailed: "复制失败",
      findDocument: "查找当前文档",
      findPlaceholder: "查找当前文档",
      findPrevious: "上一个匹配",
      findNext: "下一个匹配",
      closeFind: "关闭查找",
      findNoMatches: "没有匹配",
      status: {
        idle: "待选择",
        loading: "加载中",
        indexing: "索引中",
        ready: "就绪",
        opening: "打开中",
        error: "错误"
      },
      kinds: {
        repo: "仓库",
        baseline: "基线",
        requirement: "需求",
        mission: "任务组",
        task: "任务",
        document: "文档",
        markdown_file: "Markdown 文件",
        folder: "目录"
      },
      chainLabels: {
        baseline: "基线",
        requirement: "需求",
        missions: "任务组",
        tasks: "任务"
      },
      folderTitles: {
        Baseline: "基线",
        Requirements: "需求",
        Missions: "任务组",
        Tasks: "任务"
      }
    },
    en: {
      docs: "docs",
      diagrams: "diagrams",
      refresh: "Refresh",
      memoryRepo: "Memory Repo",
      recentRepos: "Quick switch",
      noRecentRepos: "No recent repos",
      chooseNewRepo: "Open New Repo",
      openMarkdownFile: "Open Markdown File",
      checkUpdate: "Check for Updates",
      updateNow: "Update",
      updateAvailable: "Update available",
      checkingUpdate: "Checking for updates",
      downloadingUpdate: "Downloading update",
      installingUpdate: "Installing update",
      noUpdate: "Already up to date",
      updateReady: "Update installed, relaunching",
      updateFailed: "Update failed",
      updateVersion: "Version",
      updateDate: "Published",
      updateReleaseNotes: "Release Notes",
      updateNoReleaseNotes: "No release notes were provided for this version.",
      updateDialogIntro: "A new version is available. Confirm to download and install it, then the app will relaunch.",
      updateLater: "Later",
      installUpdate: "Install update",
      updateProgress: "Update progress",
      closeUpdateDialog: "Close update dialog",
      dropMarkdownFile: "Drop to open Markdown file",
      dropUnsupported: "Drop .md files only",
      closeView: "Close view",
      openViews: "Open views",
      chooseRepoTitle: "Choose Memory Repo",
      chooseFileTitle: "Open Markdown File",
      noRepoTitle: "Choose a local memory repo",
      noRepoBody: "Choose a Git memory repo folder. If you choose a child folder, memView opens the Git repository root.",
      noRepoSelected: "No repo selected",
      chooseRepoFirst: "Choose a repo first",
      searchPlaceholder: "Search title or path",
      memoryFiles: "Memory files",
      noMatches: "No matches",
      language: "Language",
      hide: "Hide",
      info: "Info",
      outline: "Outline",
      readChain: "Read Chain",
      noChain: "No chain for this file.",
      showSidebar: "Show sidebar",
      hideSidebar: "Hide sidebar",
      showDetails: "Show details",
      hideDetails: "Hide details",
      file: "File",
      kind: "Kind",
      path: "Path",
      mermaid: "Mermaid",
      yes: "Yes",
      no: "No",
      diagram: "Diagram",
      diagramDetail: "Diagram detail",
      diagramViewer: "Diagram viewer",
      mermaidDiagram: "Mermaid diagram",
      zoomOut: "Zoom out",
      zoomIn: "Zoom in",
      fitDiagram: "Fit diagram",
      fitShort: "Fit",
      closeDiagram: "Close diagram",
      enlargeDiagram: "Enlarge diagram",
      copyDiagram: "Copy image",
      copyingDiagram: "Copying",
      copiedDiagram: "Copied",
      copyDiagramFailed: "Copy failed",
      findDocument: "Find in document",
      findPlaceholder: "Find in current document",
      findPrevious: "Previous match",
      findNext: "Next match",
      closeFind: "Close find",
      findNoMatches: "No matches",
      status: {
        idle: "Choose Repo",
        loading: "Loading",
        indexing: "Indexing",
        ready: "Ready",
        opening: "Opening",
        error: "Error"
      },
      kinds: {
        repo: "repo",
        baseline: "baseline",
        requirement: "requirement",
        mission: "mission",
        task: "task",
        document: "document",
        markdown_file: "Markdown file",
        folder: "folder"
      },
      chainLabels: {
        baseline: "baseline",
        requirement: "requirement",
        missions: "missions",
        tasks: "tasks"
      },
      folderTitles: {
        Baseline: "Baseline",
        Requirements: "Requirements",
        Missions: "Missions",
        Tasks: "Tasks"
      }
    }
  };
  const markdown = new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true
  });
  const defaultHeadingOpenRenderer = markdown.renderer.rules.heading_open;
  const defaultTableOpenRenderer = markdown.renderer.rules.table_open;
  const defaultTableCloseRenderer = markdown.renderer.rules.table_close;

  markdown.renderer.rules.heading_open = (tokens, index, options, env, self) => {
    const token = tokens[index];
    const inlineToken = tokens[index + 1];
    if (inlineToken?.type === "inline") {
      token.attrSet("id", getUniqueHeadingId(inlineToken.content, env as MarkdownRenderEnv));
      token.attrJoin("class", "reader-heading");
      token.attrSet("tabindex", "-1");
    }

    return defaultHeadingOpenRenderer
      ? defaultHeadingOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.table_open = (tokens, index, options, env, self) => {
    const tableOpen = defaultTableOpenRenderer
      ? defaultTableOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
    return `<div class="table-scroll">${tableOpen}`;
  };

  markdown.renderer.rules.table_close = (tokens, index, options, env, self) => {
    const tableClose = defaultTableCloseRenderer
      ? defaultTableCloseRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
    return `${tableClose}</div>`;
  };

  mermaid.initialize({
    startOnLoad: false,
    theme: "neutral",
    securityLevel: "strict",
    flowchart: {
      htmlLabels: false
    }
  });

  let snapshot: RepoSnapshot | null = null;
  let current: Document | null = null;
  let repoCurrent: Document | null = null;
  let renderedHtml = "";
  let query = "";
  let status: StatusKey = "idle";
  let error = "";
  let updateState: UpdateState = "idle";
  let updateMessage = "";
  let updateError = false;
  let updateProgress: number | null = null;
  let updateDialogOpen = false;
  let pendingUpdate: Update | null = null;
  let pendingUpdateVersion = "";
  let pendingUpdateDate = "";
  let pendingUpdateNotes = "";
  let updateToastMessage = "";
  let updateToastTone: ToastTone = "info";
  let updateToastTimer: number | null = null;
  let repoPath = getInitialRepoPath();
  let openViews: OpenView[] = repoPath ? [createRepoView(repoPath)] : [];
  let activeViewId = openViews[0]?.id ?? "";
  let fileDocuments = new Map<string, Document>();
  let collapsedFolderIds = new Set<string>();
  let recentRepoPaths = getInitialRecentRepoPaths(repoPath);
  let selectedRecentRepoPath = recentRepoPaths[0] ?? repoPath;
  let sidebarOpen = false;
  let contextOpen = true;
  let zoomedDiagramHtml = "";
  let zoomedDiagramTitle = "";
  let zoomLevel = 1;
  let diagramViewport: HTMLDivElement | null = null;
  let copyDiagramState: CopyDiagramState = "idle";
  let copyDiagramErrorMessage = "";
  let copyDiagramResetTimer: number | null = null;
  let panX = 32;
  let panY = 32;
  let isPanning = false;
  let panStartX = 0;
  let panStartY = 0;
  let panOriginX = 0;
  let panOriginY = 0;
  let locale: Locale = getInitialLocale();
  let isDragHovering = false;
  let dragDropUnlisten: UnlistenFn | null = null;
  let findOpen = false;
  let findQuery = "";
  let findMatchCount = 0;
  let activeFindIndex = 0;
  let findInput: HTMLInputElement | null = null;

  $: t = messages[locale];
  $: activeView = getOpenView(activeViewId);
  $: activeViewIsFile = activeView?.type === "file";
  $: showSidebar = sidebarOpen && !activeViewIsFile;
  $: repoBusy = status === "indexing" || status === "opening";
  $: updateBusy = updateState !== "idle";
  $: updateInstalling = updateState === "downloading" || updateState === "installing";
  $: renderedUpdateNotes = renderUpdateNotes(pendingUpdateNotes);
  $: flatTree = snapshot ? flattenTree(snapshot.tree, 0, collapsedFolderIds) : [];
  $: visibleNodes = snapshot
    ? query.trim()
      ? flattenDocs(searchDocs(snapshot.docs, query))
      : flatTree
    : [];
  $: docHeadings = current ? getDocumentHeadings(current.markdown) : [];
  $: headerKind = activeViewIsFile ? "markdown_file" : current?.kind ?? "repo";
  $: headerTitle = current?.title ?? (repoPath ? t.status[status] : t.noRepoTitle);
  $: headerPath = activeViewIsFile
    ? current?.path ?? activeView?.path ?? t.noRepoSelected
    : current?.relative_path ?? snapshot?.root_path ?? (repoPath || t.noRepoSelected);
  $: findStatus = formatFindStatus();
  $: copyDiagramButtonText = getCopyDiagramStateText(copyDiagramState);
  $: copyDiagramButtonTitle = getCopyDiagramButtonTitle();

  onMount(() => {
    document.documentElement.lang = locale;
    document.addEventListener("click", handleDocumentClick);
    window.addEventListener("resize", handleWindowResize);
    void setupDragDrop();
    if (repoPath) {
      void loadRepo(repoPath);
    }
  });

  onDestroy(() => {
    document.removeEventListener("click", handleDocumentClick);
    window.removeEventListener("resize", handleWindowResize);
    dragDropUnlisten?.();
    if (copyDiagramResetTimer !== null) {
      window.clearTimeout(copyDiagramResetTimer);
    }
    if (updateToastTimer !== null) {
      window.clearTimeout(updateToastTimer);
    }
    void pendingUpdate?.close().catch((err) => {
      console.warn("Failed to close pending update", err);
    });
  });

  async function setupDragDrop() {
    try {
      dragDropUnlisten = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          isDragHovering = true;
          return;
        }

        if (event.payload.type === "leave") {
          isDragHovering = false;
          return;
        }

        isDragHovering = false;
        void openDroppedMarkdownFiles(event.payload.paths);
      });
    } catch (err) {
      console.warn("File drag and drop setup failed", err);
    }
  }

  async function openDroppedMarkdownFiles(paths: string[]) {
    const markdownPaths = paths.filter(isMarkdownPath);
    if (!markdownPaths.length) {
      error = t.dropUnsupported;
      status = "error";
      return;
    }

    for (const path of markdownPaths) {
      await openMarkdownFile(path);
    }
  }

  function isMarkdownPath(path: string) {
    return /\.md$/i.test(path.trim());
  }

  function getInitialLocale(): Locale {
    if (typeof localStorage !== "undefined") {
      const saved = localStorage.getItem(localeStorageKey);
      if (saved === "zh-CN" || saved === "en") {
        return saved;
      }
    }

    const browserLanguage = typeof navigator === "undefined" ? "" : navigator.language.toLowerCase();
    return browserLanguage.startsWith("zh") ? "zh-CN" : "en";
  }

  function getInitialRepoPath() {
    if (typeof localStorage === "undefined") {
      return "";
    }

    return localStorage.getItem(repoPathStorageKey) ?? "";
  }

  function getInitialRecentRepoPaths(initialRepoPath: string) {
    const paths = parseStoredRecentRepoPaths();
    return uniqueRepoPaths(initialRepoPath ? [initialRepoPath, ...paths] : paths).slice(
      0,
      recentRepoLimit
    );
  }

  function parseStoredRecentRepoPaths() {
    if (typeof localStorage === "undefined") {
      return [];
    }

    try {
      const parsed = JSON.parse(localStorage.getItem(recentRepoPathsStorageKey) ?? "[]");
      return Array.isArray(parsed)
        ? parsed.filter((path): path is string => typeof path === "string")
        : [];
    } catch {
      return [];
    }
  }

  function uniqueRepoPaths(paths: string[]) {
    const seen = new Set<string>();
    const unique: string[] = [];
    for (const path of paths) {
      const trimmed = path.trim();
      if (!trimmed || seen.has(trimmed)) {
        continue;
      }
      seen.add(trimmed);
      unique.push(trimmed);
    }
    return unique;
  }

  function rememberRepoPath(path: string) {
    recentRepoPaths = uniqueRepoPaths([path, ...recentRepoPaths]).slice(0, recentRepoLimit);
    selectedRecentRepoPath = path;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(repoPathStorageKey, path);
      localStorage.setItem(recentRepoPathsStorageKey, JSON.stringify(recentRepoPaths));
    }
  }

  function setLocale(nextLocale: Locale) {
    locale = nextLocale;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(localeStorageKey, nextLocale);
    }
    if (typeof document !== "undefined") {
      document.documentElement.lang = nextLocale;
    }
    void rerenderCurrentDocument();
  }

  async function rerenderCurrentDocument() {
    if (!current) {
      return;
    }

    renderedHtml = renderMarkdown(current.markdown);
    await completeRenderedDocumentUpdate();
  }

  async function completeRenderedDocumentUpdate() {
    await tick();
    enhanceRenderedTables();
    await renderMermaid();
    await refreshFindHighlights();
  }

  function formatKind(kind: string | null | undefined) {
    if (!kind) {
      return "-";
    }
    return t.kinds[kind] ?? kind;
  }

  function formatChainLabel(label: string) {
    return t.chainLabels[label] ?? label;
  }

  function displayNodeTitle(node: TreeNode) {
    return node.path ? node.title : t.folderTitles[node.title] ?? node.title;
  }

  function repoName(path: string) {
    const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
    return normalized.split("/").filter(Boolean).pop() ?? path;
  }

  function createRepoView(path: string): OpenView {
    return {
      id: repoViewId,
      type: "repo",
      title: repoName(path) || "Memory Repo",
      path
    };
  }

  function createFileView(doc: Document): OpenView {
    return {
      id: fileViewId(doc.path),
      type: "file",
      title: doc.title || repoName(doc.path),
      path: doc.path
    };
  }

  function fileViewId(path: string) {
    return `${fileViewPrefix}${normalizePathname(path)}`;
  }

  function getOpenView(id: string) {
    return openViews.find((view) => view.id === id) ?? null;
  }

  function upsertOpenView(view: OpenView) {
    const existing = openViews.findIndex((item) => item.id === view.id);
    openViews = existing === -1
      ? [...openViews, view]
      : openViews.map((item, index) => index === existing ? view : item);
  }

  function findFileView(path: string) {
    const normalized = normalizePathname(path);
    return openViews.find(
      (view) => view.type === "file" && normalizePathname(view.path) === normalized
    ) ?? null;
  }

  async function activateView(id: string) {
    if (!getOpenView(id)) {
      return;
    }

    activeViewId = id;
    error = "";
    await renderActiveView();
  }

  function closeView(event: MouseEvent, id: string) {
    event.stopPropagation();
    if (openViews.length <= 1) {
      return;
    }

    const closingIndex = openViews.findIndex((view) => view.id === id);
    if (closingIndex === -1) {
      return;
    }

    const closingView = openViews[closingIndex];
    const nextViews = openViews.filter((view) => view.id !== id);
    if (closingView.type === "file") {
      const nextFileDocuments = new Map(fileDocuments);
      nextFileDocuments.delete(id);
      fileDocuments = nextFileDocuments;
    }
    openViews = nextViews;

    if (activeViewId !== id) {
      return;
    }

    const nextView = nextViews[Math.min(closingIndex, nextViews.length - 1)] ?? null;
    activeViewId = nextView?.id ?? "";
    void renderActiveView();
  }

  async function renderActiveView() {
    const view = getOpenView(activeViewId);
    if (!view) {
      current = null;
      renderedHtml = "";
      status = "idle";
      return;
    }

    current = view.type === "repo" ? repoCurrent : fileDocuments.get(view.id) ?? null;
    renderedHtml = current ? renderMarkdown(current.markdown) : "";
    status = current || (view.type === "repo" && snapshot) ? "ready" : "idle";
    if (!current) {
      return;
    }

    await completeRenderedDocumentUpdate();
  }

  async function browseRepo() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t.chooseRepoTitle
    });

    if (typeof selected !== "string") {
      return;
    }

    await loadRepo(selected);
  }

  async function browseMarkdownFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: t.chooseFileTitle,
      filters: [{ name: "Markdown", extensions: ["md"] }]
    });

    if (typeof selected !== "string") {
      return;
    }

    await openMarkdownFile(selected);
  }

  async function checkForUpdates() {
    if (updateState !== "idle" || updateDialogOpen) {
      return;
    }

    updateState = "checking";
    updateError = false;
    updateProgress = null;
    updateMessage = "";

    try {
      const update = await check({ timeout: 30000 });
      if (!update) {
        showUpdateToast(t.noUpdate);
        return;
      }

      openUpdateDialog(update);
    } catch (err) {
      showUpdateToast(`${t.updateFailed}: ${getErrorMessage(err)}`, "error");
    } finally {
      if (updateState === "checking") {
        updateState = "idle";
      }
    }
  }

  function openUpdateDialog(update: Update) {
    if (pendingUpdate && pendingUpdate !== update) {
      void pendingUpdate.close().catch((err) => {
        console.warn("Failed to close previous update", err);
      });
    }

    pendingUpdate = update;
    pendingUpdateVersion = update.version;
    pendingUpdateDate = update.date ?? "";
    pendingUpdateNotes = update.body?.trim() ?? "";
    updateDialogOpen = true;
    updateError = false;
    updateMessage = "";
    updateProgress = null;
  }

  async function closeUpdateDialog() {
    if (updateInstalling) {
      return;
    }

    updateDialogOpen = false;
    updateError = false;
    updateMessage = "";
    updateProgress = null;
    pendingUpdateVersion = "";
    pendingUpdateDate = "";
    pendingUpdateNotes = "";

    const update = pendingUpdate;
    pendingUpdate = null;
    if (update) {
      try {
        await update.close();
      } catch (err) {
        console.warn("Failed to close pending update", err);
      }
    }
  }

  async function installPendingUpdate() {
    if (!pendingUpdate || updateState !== "idle") {
      return;
    }

    let downloaded = 0;
    let contentLength = 0;
    updateState = "downloading";
    updateError = false;
    updateMessage = `${t.downloadingUpdate} ${pendingUpdate.version}`;
    updateProgress = 0;

    try {
      await pendingUpdate.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === "Started") {
          downloaded = 0;
          contentLength = event.data.contentLength ?? 0;
          updateState = "downloading";
          updateProgress = contentLength > 0 ? 0 : null;
          updateMessage = `${t.downloadingUpdate} ${pendingUpdateVersion}`;
          return;
        }

        if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          updateProgress = contentLength > 0
            ? Math.min(99, Math.floor((downloaded / contentLength) * 100))
            : null;
          return;
        }

        updateState = "installing";
        updateMessage = t.installingUpdate;
        updateProgress = 100;
      });

      updateState = "installing";
      updateProgress = 100;
      updateMessage = t.updateReady;
      await relaunch();
    } catch (err) {
      updateState = "idle";
      updateError = true;
      updateProgress = null;
      updateMessage = `${t.updateFailed}: ${getErrorMessage(err)}`;
    }
  }

  function showUpdateToast(message: string, tone: ToastTone = "info") {
    updateToastMessage = message;
    updateToastTone = tone;

    if (updateToastTimer !== null) {
      window.clearTimeout(updateToastTimer);
    }

    updateToastTimer = window.setTimeout(() => {
      updateToastMessage = "";
      updateToastTimer = null;
    }, 2600);
  }

  function renderUpdateNotes(source: string) {
    if (!source.trim()) {
      return "";
    }

    return markdown.render(source, { headingCounts: new Map<string, number>() });
  }

  function formatUpdateDate(value: string) {
    if (!value) {
      return "";
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return value;
    }

    return new Intl.DateTimeFormat(locale, {
      year: "numeric",
      month: "short",
      day: "numeric"
    }).format(date);
  }

  function handleRecentRepoChange(event: Event) {
    const nextPath = event.currentTarget instanceof HTMLSelectElement
      ? event.currentTarget.value
      : selectedRecentRepoPath;
    if (!nextPath || repoBusy) {
      return;
    }

    if (nextPath === repoPath && snapshot) {
      upsertOpenView(createRepoView(repoPath));
      void activateView(repoViewId);
      return;
    }

    void loadRepo(nextPath);
  }

  async function loadRepo(path = repoPath, options: LoadRepoOptions = {}) {
    const nextRepoPath = path.trim();
    if (!nextRepoPath) {
      status = "idle";
      snapshot = null;
      current = null;
      renderedHtml = "";
      error = "";
      return;
    }

    const preservedRelativePath = options.preserveCurrentDocument ? repoCurrent?.relative_path : "";
    const preservedScrollTop = options.preserveCurrentDocument ? getReaderScrollTop() : undefined;
    const preservedCollapsedFolderIds = options.preserveCurrentDocument
      ? new Set(collapsedFolderIds)
      : null;

    status = "indexing";
    error = "";
    try {
      snapshot = await invoke<RepoSnapshot>("scan_repo", { repoPath: nextRepoPath });
      repoPath = snapshot.root_path;
      upsertOpenView(createRepoView(snapshot.root_path));
      activeViewId = repoViewId;
      collapsedFolderIds = preservedCollapsedFolderIds ?? getDefaultCollapsedFolderIds(snapshot.tree);
      rememberRepoPath(snapshot.root_path);
      status = "ready";
      const preservedEntry = preservedRelativePath
        ? snapshot.docs.find((doc) => doc.relative_path === preservedRelativePath)
        : null;
      const entry = preservedEntry ??
        snapshot.docs.find((doc) => doc.relative_path === "README.md") ??
        snapshot.docs.find((doc) => doc.relative_path === "baseline/README.md") ??
        snapshot.docs[0];
      if (entry) {
        await openDocument(entry.path, {
          restoreScrollTop: preservedEntry ? preservedScrollTop : undefined
        });
      }
    } catch (err) {
      snapshot = null;
      repoCurrent = null;
      if (getOpenView(activeViewId)?.type !== "file") {
        current = null;
        renderedHtml = "";
      }
      error = String(err);
      status = "error";
    }
  }

  function getReaderScrollTop() {
    return document.querySelector<HTMLElement>(".reader")?.scrollTop ?? 0;
  }

  async function openDocument(path: string, options: OpenDocumentOptions = {}) {
    if (!repoPath) {
      status = "idle";
      return;
    }

    upsertOpenView(createRepoView(repoPath));
    activeViewId = repoViewId;
    status = "opening";
    error = "";
    try {
      repoCurrent = await invoke<Document>("read_document", { repoPath, path });
      current = repoCurrent;
      renderedHtml = renderMarkdown(repoCurrent.markdown);
      status = "ready";
      await completeRenderedDocumentUpdate();
      if (options.restoreScrollTop !== undefined) {
        const reader = document.querySelector<HTMLElement>(".reader");
        if (reader) {
          reader.scrollTop = options.restoreScrollTop;
        }
      }
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  async function openMarkdownFile(path: string, anchor = "") {
    const existing = findFileView(path);
    if (existing && fileDocuments.has(existing.id)) {
      await activateView(existing.id);
      if (anchor) {
        await scrollToReaderAnchor(anchor);
      }
      return;
    }

    status = "opening";
    error = "";
    try {
      const doc = await invoke<Document>("read_markdown_file", { path });
      const view = createFileView(doc);
      const nextFileDocuments = new Map(fileDocuments);
      nextFileDocuments.set(view.id, doc);
      fileDocuments = nextFileDocuments;
      upsertOpenView(view);
      activeViewId = view.id;
      current = doc;
      renderedHtml = renderMarkdown(doc.markdown);
      status = "ready";
      await completeRenderedDocumentUpdate();
      if (anchor) {
        await scrollToReaderAnchor(anchor);
      }
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  function renderMarkdown(source: string) {
    return markdown.render(source, { headingCounts: new Map<string, number>() }).replace(
      /<pre><code class="language-mermaid">([\s\S]*?)<\/code><\/pre>/g,
      (_, encoded: string) => `
        <figure class="diagram-frame">
          <div class="diagram-actions">
            <button class="diagram-copy" type="button" aria-label="${t.copyDiagram}" title="${t.copyDiagram}"></button>
            <button class="diagram-zoom" type="button" aria-label="${t.enlargeDiagram}" title="${t.enlargeDiagram}"></button>
          </div>
          <div class="mermaid">${decodeHtml(encoded)}</div>
        </figure>
      `
    );
  }

  function getDocumentHeadings(source: string): DocHeading[] {
    const env: MarkdownRenderEnv = { headingCounts: new Map<string, number>() };
    const tokens = markdown.parse(source, {});
    const headings: DocHeading[] = [];

    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (token.type !== "heading_open") {
        continue;
      }

      const inlineToken = tokens[index + 1];
      if (inlineToken?.type !== "inline") {
        continue;
      }

      const title = inlineToken.content.trim();
      const level = Number(token.tag.slice(1));
      if (!title || !Number.isFinite(level)) {
        continue;
      }

      headings.push({
        id: getUniqueHeadingId(title, env),
        title,
        level
      });
    }

    return headings;
  }

  function headingIndent(level: number) {
    return `${Math.max(0, level - 1) * 10}px`;
  }

  function enhanceRenderedTables() {
    const tables = document.querySelectorAll<HTMLTableElement>(".reader table");
    tables.forEach((table) => {
      const headers = Array.from(table.querySelectorAll<HTMLTableCellElement>("thead th"));
      const inlineColumns = headers
        .map((header, index) => shouldKeepTableColumnInline(header.textContent ?? "") ? index : -1)
        .filter((index) => index >= 0);
      const wideColumns = headers
        .map((header, index) => shouldUseWideTableColumn(header.textContent ?? "") ? index : -1)
        .filter((index) => index >= 0);

      Array.from(table.rows).forEach((row) => {
        inlineColumns.forEach((index) => row.cells[index]?.classList.add("table-cell-nowrap"));
        wideColumns.forEach((index) => row.cells[index]?.classList.add("table-cell-wide"));
      });
    });
  }

  function shouldKeepTableColumnInline(header: string) {
    return /(^|[_\s-])(id|key|code|path|file|class|method|api|url|uri)($|[_\s-])|编号|序号|顺序|状态|类型|路径|位置|消费|来源|目标|接口|方法|规则|回调|rule|order|status|type|path|file|class|method|api/i.test(
      header.trim()
    );
  }

  function shouldUseWideTableColumn(header: string) {
    if (shouldKeepTableColumnInline(header)) {
      return false;
    }

    return /条件|结果|行为|说明|描述|备注|内容|原因|风险|决策|condition|result|behavior|description|detail|note/i.test(
      header.trim()
    );
  }

  async function renderMermaid() {
    const nodes = document.querySelectorAll<HTMLElement>(".reader .mermaid");
    if (!nodes.length) {
      return;
    }
    try {
      await mermaid.run({ nodes });
    } catch (err) {
      console.warn("Mermaid render failed", err);
    }
  }

  function decodeHtml(value: string) {
    const textarea = document.createElement("textarea");
    textarea.innerHTML = value;
    return textarea.value;
  }

  function getUniqueHeadingId(content: string, env: MarkdownRenderEnv) {
    const baseId = slugifyHeading(content);
    const count = env.headingCounts.get(baseId) ?? 0;
    env.headingCounts.set(baseId, count + 1);
    return count === 0 ? baseId : `${baseId}-${count + 1}`;
  }

  function slugifyHeading(value: string) {
    const slug = value
      .trim()
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/[^\p{L}\p{N}\s-]/gu, "")
      .trim()
      .replace(/\s+/g, "-");

    return slug || "section";
  }

  function flattenTree(nodes: TreeNode[], depth = 0, collapsed = new Set<string>()): FlatNode[] {
    return nodes.flatMap((node) => [
      { ...node, depth },
      ...(collapsed.has(node.id) ? [] : flattenTree(node.children, depth + 1, collapsed))
    ]);
  }

  function getDefaultCollapsedFolderIds(nodes: TreeNode[]) {
    const collapsed = new Set<string>();
    const visit = (node: TreeNode) => {
      if (!node.path && node.children.length) {
        collapsed.add(node.id);
      }
      node.children.forEach(visit);
    };

    nodes.forEach(visit);
    return collapsed;
  }

  function searchDocs(docs: DocMeta[], value: string): TreeNode[] {
    const needle = value.trim().toLowerCase();
    return docs
      .filter((doc) => {
        const haystack = `${doc.title} ${doc.relative_path} ${doc.kind}`.toLowerCase();
        return haystack.includes(needle);
      })
      .map((doc) => ({
        id: doc.id,
        title: doc.title,
        path: doc.path,
        kind: doc.kind,
        children: []
      }));
  }

  function flattenDocs(nodes: TreeNode[]): FlatNode[] {
    return nodes.map((node) => ({ ...node, depth: 0 }));
  }

  function nodeClass(node: FlatNode) {
    return [
      "nav-row",
      node.path ? "doc" : "folder",
      activeView?.type === "repo" && current?.path === node.path ? "active" : "",
      !node.path && collapsedFolderIds.has(node.id) ? "collapsed" : "",
      node.kind
    ].join(" ");
  }

  function toggleFolder(node: FlatNode) {
    const next = new Set(collapsedFolderIds);
    if (next.has(node.id)) {
      next.delete(node.id);
    } else {
      next.add(node.id);
    }
    collapsedFolderIds = next;
  }

  function handleNodeClick(node: FlatNode) {
    if (node.path) {
      void openDocument(node.path);
      return;
    }

    if (node.children.length) {
      toggleFolder(node);
    }
  }

  async function handleDocumentClick(event: MouseEvent) {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const link = target.closest<HTMLAnchorElement>(".reader a");
    if (link && await handleReaderLinkClick(event, link)) {
      return;
    }

    const copyButton = target.closest<HTMLButtonElement>(".diagram-copy");
    if (copyButton && copyButton.closest(".reader")) {
      const svg = getFrameDiagramSvg(copyButton);
      if (!svg) {
        return;
      }

      await copyInlineDiagram(svg, copyButton);
      return;
    }

    const zoomButton = target.closest<HTMLButtonElement>(".diagram-zoom");
    if (!zoomButton || !zoomButton.closest(".reader")) {
      return;
    }

    const svg = getFrameDiagramSvg(zoomButton);
    if (!svg) {
      return;
    }

    zoomedDiagramHtml = serializeDiagramSvg(svg);
    zoomedDiagramTitle = current?.title ?? t.mermaidDiagram;
    setCopyDiagramState("idle");
    resetDiagramView();
    await tick();
    fitDiagramToViewport();
  }

  function getFrameDiagramSvg(button: HTMLButtonElement) {
    const frame = button.closest<HTMLElement>(".diagram-frame");
    return frame?.querySelector<SVGSVGElement>(".mermaid svg") ?? null;
  }

  async function handleReaderLinkClick(event: MouseEvent, link: HTMLAnchorElement) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return false;
    }

    const href = link.getAttribute("href")?.trim() ?? "";
    if (!href) {
      return false;
    }

    if (href.startsWith("#")) {
      event.preventDefault();
      await scrollToReaderAnchor(href.slice(1));
      return true;
    }

    const target = resolveLinkedDocument(href);
    if (!target) {
      return false;
    }

    event.preventDefault();
    if (target.type === "file") {
      await openMarkdownFile(target.path, target.anchor);
      return true;
    }

    await openDocument(target.path);
    if (target.anchor) {
      await scrollToReaderAnchor(target.anchor);
    }
    return true;
  }

  function resolveLinkedDocument(href: string): LinkTarget | null {
    if (!current) {
      return null;
    }

    const { pathPart, anchor } = splitHref(href);
    if (isExternalHref(pathPart)) {
      return null;
    }

    if (activeView?.type === "file") {
      const path = resolveStandaloneFilePath(pathPart);
      return path ? { type: "file", path, anchor } : null;
    }

    if (!snapshot) {
      return null;
    }

    const normalizedPath = resolveRepoRelativePath(pathPart);
    if (normalizedPath === null) {
      return null;
    }

    const doc = findDocByLinkPath(normalizedPath);
    return doc ? { type: "repo", path: doc.path, anchor } : null;
  }

  function splitHref(href: string) {
    const hashIndex = href.indexOf("#");
    if (hashIndex === -1) {
      return { pathPart: stripQuery(href), anchor: "" };
    }

    return {
      pathPart: stripQuery(href.slice(0, hashIndex)),
      anchor: safeDecodeURIComponent(href.slice(hashIndex + 1))
    };
  }

  function stripQuery(value: string) {
    const queryIndex = value.indexOf("?");
    return queryIndex === -1 ? value : value.slice(0, queryIndex);
  }

  function isExternalHref(pathPart: string) {
    return /^[a-z][a-z0-9+.-]*:/i.test(pathPart) && !pathPart.toLowerCase().startsWith("file:");
  }

  function resolveRepoRelativePath(pathPart: string) {
    if (!current) {
      return null;
    }

    if (!pathPart || pathPart === ".") {
      return current.relative_path;
    }

    if (pathPart.toLowerCase().startsWith("file:")) {
      try {
        return normalizePathname(safeDecodeURIComponent(new URL(pathPart).pathname));
      } catch {
        return null;
      }
    }

    const decodedPath = normalizePathname(safeDecodeURIComponent(pathPart));
    if (decodedPath.startsWith("/")) {
      return decodedPath;
    }

    const baseDir = current.relative_path.includes("/")
      ? current.relative_path.slice(0, current.relative_path.lastIndexOf("/") + 1)
      : "";
    return normalizePathSegments(`${baseDir}${decodedPath}`);
  }

  function resolveStandaloneFilePath(pathPart: string) {
    if (!current) {
      return null;
    }

    if (!pathPart || pathPart === ".") {
      return current.path;
    }

    if (pathPart.toLowerCase().startsWith("file:")) {
      try {
        return normalizeAbsolutePathSegments(
          normalizePathname(safeDecodeURIComponent(new URL(pathPart).pathname))
        );
      } catch {
        return null;
      }
    }

    const decodedPath = normalizePathname(safeDecodeURIComponent(pathPart));
    if (decodedPath.startsWith("/")) {
      return normalizeAbsolutePathSegments(decodedPath);
    }

    const baseDir = current.path.includes("/")
      ? current.path.slice(0, current.path.lastIndexOf("/"))
      : "";
    return normalizeAbsolutePathSegments(`${baseDir}/${decodedPath}`);
  }

  function normalizePathname(value: string) {
    return value.replace(/\\/g, "/");
  }

  function normalizeAbsolutePathSegments(value: string) {
    const normalized = normalizePathSegments(value);
    if (normalized === null) {
      return null;
    }
    return value.startsWith("/") ? `/${normalized}` : normalized;
  }

  function normalizePathSegments(value: string) {
    const segments: string[] = [];
    for (const segment of value.split("/")) {
      if (!segment || segment === ".") {
        continue;
      }
      if (segment === "..") {
        if (!segments.length) {
          return null;
        }
        segments.pop();
        continue;
      }
      segments.push(segment);
    }

    return segments.join("/");
  }

  function findDocByLinkPath(path: string) {
    if (!snapshot) {
      return null;
    }

    const normalized = normalizePathname(path);
    const absoluteDoc = normalized.startsWith("/")
      ? snapshot.docs.find((doc) => normalizePathname(doc.path) === normalized)
      : null;
    if (absoluteDoc) {
      return absoluteDoc;
    }

    const relativePath = normalized.replace(/^\/+/, "");
    const candidates = [
      relativePath,
      relativePath.endsWith("/") ? `${relativePath}README.md` : `${relativePath}/README.md`,
      relativePath.toLowerCase().endsWith(".md") ? relativePath : `${relativePath}.md`
    ];
    return snapshot.docs.find((doc) => candidates.includes(doc.relative_path)) ?? null;
  }

  async function scrollToReaderAnchor(anchor: string) {
    const decodedAnchor = safeDecodeURIComponent(anchor).trim();
    if (!decodedAnchor) {
      return;
    }

    await tick();
    requestAnimationFrame(() => {
      const target = findReaderAnchor(decodedAnchor);
      if (!target) {
        return;
      }

      target.scrollIntoView({ behavior: "smooth", block: "start" });
      target.focus({ preventScroll: true });
    });
  }

  function findReaderAnchor(anchor: string) {
    const reader = document.querySelector<HTMLElement>(".reader");
    if (!reader) {
      return null;
    }

    const ids = [anchor, slugifyHeading(anchor)];
    return Array.from(reader.querySelectorAll<HTMLElement>("[id], a[name]")).find((element) =>
      ids.includes(element.id) || ids.includes(element.getAttribute("name") ?? "")
    ) ?? null;
  }

  async function openFind() {
    if (!current) {
      return;
    }

    findOpen = true;
    await tick();
    findInput?.focus();
    findInput?.select();
    await refreshFindHighlights();
  }

  function closeFind() {
    findOpen = false;
    clearFindHighlights();
    findMatchCount = 0;
    activeFindIndex = 0;
  }

  function handleFindInput(event: Event) {
    if (event.currentTarget instanceof HTMLInputElement) {
      findQuery = event.currentTarget.value;
    }
    activeFindIndex = 0;
    void refreshFindHighlights({ scroll: true });
  }

  function formatFindStatus() {
    if (!findQuery.trim()) {
      return "";
    }

    return `${findMatchCount ? activeFindIndex + 1 : 0} / ${findMatchCount}`;
  }

  async function refreshFindHighlights({ scroll = false } = {}) {
    await tick();
    const reader = document.querySelector<HTMLElement>(".reader");
    clearFindHighlights(reader);

    const needle = findQuery.trim();
    if (!reader || !findOpen || !current || !needle) {
      findMatchCount = 0;
      activeFindIndex = 0;
      return;
    }

    const matches = highlightReaderMatches(reader, needle);
    findMatchCount = matches.length;
    if (!matches.length) {
      activeFindIndex = 0;
      return;
    }

    activeFindIndex = Math.min(Math.max(activeFindIndex, 0), matches.length - 1);
    setActiveFindMatch(matches, scroll);
  }

  function highlightReaderMatches(reader: HTMLElement, needle: string) {
    const matches: HTMLElement[] = [];
    const textNodes: Text[] = [];
    const lowerNeedle = needle.toLowerCase();
    const walker = document.createTreeWalker(reader, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement;
        if (!parent || parent.closest(".mermaid, .diagram-actions, mark.find-highlight")) {
          return NodeFilter.FILTER_REJECT;
        }

        const text = node.nodeValue ?? "";
        return text.toLowerCase().includes(lowerNeedle)
          ? NodeFilter.FILTER_ACCEPT
          : NodeFilter.FILTER_REJECT;
      }
    });

    while (walker.nextNode()) {
      textNodes.push(walker.currentNode as Text);
    }

    for (const node of textNodes) {
      const text = node.nodeValue ?? "";
      const lowerText = text.toLowerCase();
      const fragment = document.createDocumentFragment();
      let cursor = 0;
      let matchIndex = lowerText.indexOf(lowerNeedle);

      while (matchIndex !== -1) {
        if (matchIndex > cursor) {
          fragment.append(document.createTextNode(text.slice(cursor, matchIndex)));
        }

        const mark = document.createElement("mark");
        mark.className = "find-highlight";
        mark.textContent = text.slice(matchIndex, matchIndex + needle.length);
        fragment.append(mark);
        matches.push(mark);

        cursor = matchIndex + needle.length;
        matchIndex = lowerText.indexOf(lowerNeedle, cursor);
      }

      if (cursor < text.length) {
        fragment.append(document.createTextNode(text.slice(cursor)));
      }

      node.replaceWith(fragment);
    }

    return matches;
  }

  function clearFindHighlights(reader = document.querySelector<HTMLElement>(".reader")) {
    if (!reader) {
      return;
    }

    const highlights = Array.from(reader.querySelectorAll<HTMLElement>("mark.find-highlight"));
    for (const highlight of highlights) {
      const parent = highlight.parentNode;
      if (!parent) {
        continue;
      }

      while (highlight.firstChild) {
        parent.insertBefore(highlight.firstChild, highlight);
      }
      parent.removeChild(highlight);
      parent.normalize();
    }
  }

  function setActiveFindMatch(matches = getFindMatches(), scroll = false) {
    matches.forEach((match, index) => {
      match.classList.toggle("active", index === activeFindIndex);
    });

    if (scroll) {
      matches[activeFindIndex]?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }

  function getFindMatches() {
    return Array.from(document.querySelectorAll<HTMLElement>(".reader mark.find-highlight"));
  }

  function moveFindMatch(delta: number) {
    if (!findQuery.trim()) {
      findInput?.focus();
      return;
    }

    if (!findMatchCount) {
      void refreshFindHighlights({ scroll: true });
      return;
    }

    activeFindIndex = (activeFindIndex + delta + findMatchCount) % findMatchCount;
    setActiveFindMatch(undefined, true);
  }

  function isFindShortcut(event: KeyboardEvent) {
    return (event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "f";
  }

  function safeDecodeURIComponent(value: string) {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  }

  function closeDiagram() {
    zoomedDiagramHtml = "";
    isPanning = false;
    setCopyDiagramState("idle");
  }

  function getCopyDiagramStateText(state: CopyDiagramState) {
    if (state === "copying") {
      return t.copyingDiagram;
    }
    if (state === "copied") {
      return t.copiedDiagram;
    }
    if (state === "error") {
      return t.copyDiagramFailed;
    }
    return t.copyDiagram;
  }

  function setCopyDiagramState(state: CopyDiagramState, message = "") {
    if (copyDiagramResetTimer !== null) {
      window.clearTimeout(copyDiagramResetTimer);
      copyDiagramResetTimer = null;
    }

    copyDiagramErrorMessage = state === "error" ? message : "";
    copyDiagramState = state;
    if (state === "copied" || state === "error") {
      copyDiagramResetTimer = window.setTimeout(() => {
        copyDiagramState = "idle";
        copyDiagramErrorMessage = "";
        copyDiagramResetTimer = null;
      }, 1800);
    }
  }

  function getCopyDiagramButtonTitle() {
    return copyDiagramErrorMessage
      ? `${t.copyDiagramFailed}: ${copyDiagramErrorMessage}`
      : copyDiagramButtonText;
  }

  async function copyInlineDiagram(svg: SVGSVGElement, button: HTMLButtonElement) {
    if (button.disabled) {
      return;
    }

    button.disabled = true;
    try {
      await copyDiagramSvgToClipboard(svg);
      setInlineDiagramCopyState(button, "copied");
    } catch (err) {
      console.warn("Copy diagram failed", err);
      setInlineDiagramCopyState(button, "error", getErrorMessage(err));
    } finally {
      button.disabled = false;
    }
  }

  function setInlineDiagramCopyState(
    button: HTMLButtonElement,
    state: Extract<CopyDiagramState, "copied" | "error">,
    message = ""
  ) {
    button.classList.remove("copied", "error");
    button.classList.add(state);
    const label = message ? `${getCopyDiagramStateText(state)}: ${message}` : getCopyDiagramStateText(state);
    button.setAttribute("aria-label", label);
    button.title = label;

    window.setTimeout(() => {
      if (!button.isConnected) {
        return;
      }
      button.classList.remove("copied", "error");
      button.setAttribute("aria-label", t.copyDiagram);
      button.title = t.copyDiagram;
    }, 1800);
  }

  async function copyZoomedDiagram() {
    if (copyDiagramState === "copying") {
      return;
    }

    const svg = diagramViewport?.querySelector<SVGSVGElement>(".diagram-canvas svg");
    if (!svg) {
      setCopyDiagramState("error", "Diagram SVG not found");
      return;
    }

    setCopyDiagramState("copying");
    try {
      await copyDiagramSvgToClipboard(svg);
      setCopyDiagramState("copied");
    } catch (err) {
      console.warn("Copy diagram failed", err);
      setCopyDiagramState("error", getErrorMessage(err));
    }
  }

  function getErrorMessage(err: unknown) {
    return err instanceof Error ? err.message : String(err);
  }

  async function copyDiagramSvgToClipboard(svg: SVGSVGElement) {
    const svgMarkup = serializeDiagramSvg(svg);
    const raster = await rasterizeDiagramSvgMarkup(svgMarkup, svg);
    const copiedInBrowser = await copyPngBlobWithBrowserClipboard(raster.pngBlob);
    if (copiedInBrowser) {
      return;
    }

    try {
      await invoke<void>("copy_image_to_clipboard", {
        image: {
          pngBase64: raster.pngBase64
        }
      });
      return;
    } catch (pngErr) {
      console.warn("Native PNG clipboard path failed", pngErr);
      try {
        await invoke<void>("copy_svg_to_clipboard", {
          image: {
            svg: svgMarkup
          }
        });
      } catch (svgErr) {
        throw new Error(`PNG copy failed: ${getErrorMessage(pngErr)}; SVG fallback failed: ${getErrorMessage(svgErr)}`);
      }
    }
  }

  async function rasterizeDiagramSvgMarkup(svgMarkup: string, svg: SVGSVGElement) {
    const image = await loadSerializedSvgImage(svgMarkup);
    const fallbackSize = getSvgSize(svg);
    const width = Math.max(1, Math.ceil(image.naturalWidth || fallbackSize.width));
    const height = Math.max(1, Math.ceil(image.naturalHeight || fallbackSize.height));
    const maxPixels = 24000000;
    if (width * height > maxPixels) {
      throw new Error(`Diagram image is too large to copy: ${width}x${height}`);
    }

    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Canvas is not available");
    }

    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, width, height);
    context.drawImage(image, 0, 0, width, height);
    const pngBlob = await canvasToPngBlob(canvas);

    return {
      width,
      height,
      pngBase64: bytesToBase64(new Uint8Array(await pngBlob.arrayBuffer())),
      pngBlob
    };
  }

  function loadSerializedSvgImage(svgMarkup: string) {
    return new Promise<HTMLImageElement>((resolve, reject) => {
      const image = new Image();
      const url = `data:image/svg+xml;base64,${bytesToBase64(new TextEncoder().encode(svgMarkup))}`;
      image.onload = () => {
        resolve(image);
      };
      image.onerror = () => {
        reject(new Error("Failed to load diagram image"));
      };
      image.src = url;
    });
  }

  function canvasToPngBlob(canvas: HTMLCanvasElement) {
    return new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((blob) => {
        if (blob) {
          resolve(blob);
          return;
        }
        reject(new Error("Failed to encode diagram image"));
      }, "image/png");
    });
  }

  function bytesToBase64(bytes: Uint8Array | Uint8ClampedArray) {
    let binary = "";
    const chunkSize = 0x8000;
    for (let index = 0; index < bytes.length; index += chunkSize) {
      binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
    }
    return btoa(binary);
  }

  async function copyPngBlobWithBrowserClipboard(blob: Blob) {
    if (
      typeof ClipboardItem === "undefined" ||
      !navigator.clipboard ||
      typeof navigator.clipboard.write !== "function"
    ) {
      return false;
    }

    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      return true;
    } catch (err) {
      console.warn("Browser image clipboard fallback failed", err);
      return false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (isFindShortcut(event) && current) {
      event.preventDefault();
      void openFind();
      return;
    }

    if (event.key === "Escape") {
      if (updateDialogOpen && !updateInstalling) {
        void closeUpdateDialog();
        return;
      }

      if (zoomedDiagramHtml) {
        closeDiagram();
        return;
      }

      if (findOpen) {
        closeFind();
      }
      return;
    }

    if (findOpen && event.key === "Enter" && event.target === findInput) {
      event.preventDefault();
      moveFindMatch(event.shiftKey ? -1 : 1);
    }
  }

  function adjustZoom(delta: number) {
    setZoom(zoomLevel * (delta > 0 ? 1.2 : 0.8));
  }

  function resetDiagramView() {
    isPanning = false;
    if (diagramViewport) {
      fitDiagramToViewport();
      return;
    }

    zoomLevel = 1;
    panX = 0;
    panY = 0;
  }

  function setZoom(value: number, clientX?: number, clientY?: number) {
    const nextZoom = Math.min(8, Math.max(0.05, Number(value.toFixed(3))));
    if (!diagramViewport || clientX === undefined || clientY === undefined) {
      zoomLevel = nextZoom;
      return;
    }

    const rect = diagramViewport.getBoundingClientRect();
    const pointerX = clientX - rect.left;
    const pointerY = clientY - rect.top;
    const localX = (pointerX - panX) / zoomLevel;
    const localY = (pointerY - panY) / zoomLevel;

    panX = pointerX - localX * nextZoom;
    panY = pointerY - localY * nextZoom;
    zoomLevel = nextZoom;
  }

  function fitDiagramToViewport() {
    if (!diagramViewport) {
      return;
    }

    const canvas = diagramViewport.querySelector<HTMLElement>(".diagram-canvas");
    const svg = canvas?.querySelector<SVGSVGElement>("svg");
    if (!canvas || !svg) {
      return;
    }

    const viewportRect = diagramViewport.getBoundingClientRect();
    const diagramSize = getSvgSize(svg);
    if (!diagramSize.width || !diagramSize.height || !viewportRect.width || !viewportRect.height) {
      return;
    }

    const canvasStyle = getComputedStyle(canvas);
    const horizontalPadding =
      parseFloat(canvasStyle.paddingLeft) +
      parseFloat(canvasStyle.paddingRight) +
      parseFloat(canvasStyle.borderLeftWidth) +
      parseFloat(canvasStyle.borderRightWidth);
    const verticalPadding =
      parseFloat(canvasStyle.paddingTop) +
      parseFloat(canvasStyle.paddingBottom) +
      parseFloat(canvasStyle.borderTopWidth) +
      parseFloat(canvasStyle.borderBottomWidth);
    const canvasWidth = diagramSize.width + horizontalPadding;
    const canvasHeight = diagramSize.height + verticalPadding;
    const margin = 12;
    const availableWidth = Math.max(1, viewportRect.width - margin * 2);
    const availableHeight = Math.max(1, viewportRect.height - margin * 2);
    const widthScale = availableWidth / canvasWidth;
    const heightScale = availableHeight / canvasHeight;
    const nextZoom = Math.min(
      6,
      Math.max(0.05, Math.min(widthScale, heightScale))
    );

    zoomLevel = Number(nextZoom.toFixed(3));
    panX = Math.round((viewportRect.width - canvasWidth * zoomLevel) / 2);
    panY = Math.round((viewportRect.height - canvasHeight * zoomLevel) / 2);
  }

  function handleWindowResize() {
    if (!zoomedDiagramHtml) {
      return;
    }

    requestAnimationFrame(fitDiagramToViewport);
  }

  function serializeDiagramSvg(svg: SVGSVGElement) {
    const clone = svg.cloneNode(true) as SVGSVGElement;
    clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    clone.setAttribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    inlineSvgComputedStyles(svg, clone);
    const bounds = getSvgContentBox(svg);
    if (bounds) {
      clone.setAttribute(
        "viewBox",
        `${bounds.x} ${bounds.y} ${bounds.width} ${bounds.height}`
      );
    }

    const size = bounds ?? getSvgSize(svg);
    if (size.width && size.height) {
      clone.setAttribute("width", String(Math.ceil(size.width)));
      clone.setAttribute("height", String(Math.ceil(size.height)));
    }
    clone.setAttribute("preserveAspectRatio", "xMidYMid meet");
    addSvgWhiteBackground(clone, size);
    clone.style.display = "block";
    clone.style.maxWidth = "none";
    clone.style.background = "#ffffff";
    return clone.outerHTML;
  }

  function inlineSvgComputedStyles(source: SVGElement, clone: SVGElement) {
    const sourceElements = [source, ...source.querySelectorAll<SVGElement>("*")];
    const cloneElements = [clone, ...clone.querySelectorAll<SVGElement>("*")];
    const properties = [
      "alignment-baseline",
      "color",
      "dominant-baseline",
      "fill",
      "fill-opacity",
      "font-family",
      "font-size",
      "font-style",
      "font-weight",
      "letter-spacing",
      "line-height",
      "marker-end",
      "marker-mid",
      "marker-start",
      "opacity",
      "paint-order",
      "stroke",
      "stroke-dasharray",
      "stroke-linecap",
      "stroke-linejoin",
      "stroke-opacity",
      "stroke-width",
      "text-anchor",
      "white-space"
    ];

    for (let index = 0; index < sourceElements.length; index += 1) {
      const sourceElement = sourceElements[index];
      const cloneElement = cloneElements[index];
      if (!sourceElement || !cloneElement) {
        continue;
      }

      const computed = getComputedStyle(sourceElement);
      for (const property of properties) {
        const value = computed.getPropertyValue(property);
        if (value) {
          cloneElement.style.setProperty(property, value);
        }
      }
    }
  }

  function addSvgWhiteBackground(svg: SVGSVGElement, size: SvgBox | { width: number; height: number }) {
    const viewBox = svg.viewBox?.baseVal;
    const x = "x" in size ? size.x : viewBox?.x ?? 0;
    const y = "y" in size ? size.y : viewBox?.y ?? 0;
    const width = size.width;
    const height = size.height;
    if (!width || !height) {
      return;
    }

    const background = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    background.setAttribute("x", String(x));
    background.setAttribute("y", String(y));
    background.setAttribute("width", String(width));
    background.setAttribute("height", String(height));
    background.setAttribute("fill", "#ffffff");
    svg.insertBefore(background, svg.firstChild);
  }

  function getSvgContentBox(svg: SVGSVGElement): SvgBox | null {
    try {
      const box = svg.getBBox();
      if (box.width <= 0 || box.height <= 0) {
        return null;
      }

      const padding = 10;
      return {
        x: Math.floor(box.x - padding),
        y: Math.floor(box.y - padding),
        width: Math.ceil(box.width + padding * 2),
        height: Math.ceil(box.height + padding * 2)
      };
    } catch {
      return null;
    }
  }

  function getSvgSize(svg: SVGSVGElement) {
    const viewBox = svg.viewBox?.baseVal;
    if (viewBox && viewBox.width > 0 && viewBox.height > 0) {
      return { width: viewBox.width, height: viewBox.height };
    }

    const attrWidth = parseFloat(svg.getAttribute("width") ?? "");
    const attrHeight = parseFloat(svg.getAttribute("height") ?? "");
    if (Number.isFinite(attrWidth) && Number.isFinite(attrHeight) && attrWidth > 0 && attrHeight > 0) {
      return { width: attrWidth, height: attrHeight };
    }

    try {
      const box = svg.getBBox();
      if (box.width > 0 && box.height > 0) {
        return { width: box.width, height: box.height };
      }
    } catch {
      // Some SVGs cannot report a box before layout; fall back to rendered bounds.
    }

    const rect = svg.getBoundingClientRect();
    return {
      width: rect.width / zoomLevel,
      height: rect.height / zoomLevel
    };
  }

  function handleDiagramWheel(event: WheelEvent) {
    event.preventDefault();
    if (shouldPanDiagramWheel(event)) {
      panDiagramByWheel(event);
      return;
    }

    const factor = getDiagramWheelZoomFactor(event);
    setZoom(zoomLevel * factor, event.clientX, event.clientY);
  }

  function getDiagramWheelZoomFactor(event: WheelEvent) {
    if ((event.ctrlKey || event.metaKey) && event.deltaMode === WheelEvent.DOM_DELTA_PIXEL) {
      const rawFactor = Math.exp(-event.deltaY * 0.008);
      return Math.min(1.15, Math.max(0.85, rawFactor));
    }

    return event.deltaY < 0 ? 1.12 : 0.88;
  }

  function shouldPanDiagramWheel(event: WheelEvent) {
    if (event.ctrlKey || event.metaKey) {
      return false;
    }

    if (event.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) {
      return false;
    }

    return Math.abs(event.deltaX) > 0 || Math.abs(event.deltaY) < 80;
  }

  function panDiagramByWheel(event: WheelEvent) {
    panX -= event.deltaX;
    panY -= event.deltaY;
  }

  function startPan(event: PointerEvent) {
    const target = event.target as HTMLElement;
    if (target.closest(".diagram-tools")) {
      return;
    }

    isPanning = true;
    panStartX = event.clientX;
    panStartY = event.clientY;
    panOriginX = panX;
    panOriginY = panY;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function movePan(event: PointerEvent) {
    if (!isPanning) {
      return;
    }

    panX = panOriginX + event.clientX - panStartX;
    panY = panOriginY + event.clientY - panStartY;
  }

  function endPan(event: PointerEvent) {
    if (!isPanning) {
      return;
    }

    isPanning = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<main
  class="app-shell"
  class:sidebar-closed={!showSidebar}
  class:context-closed={!contextOpen}
>
  <div
    class="app-toolbar"
    role="toolbar"
    aria-label="memView"
    tabindex="-1"
    data-tauri-drag-region="deep"
  >
    <div class="toolbar-left">
      <button
        class="ghost icon-button"
        type="button"
        disabled={activeViewIsFile}
        aria-expanded={showSidebar}
        aria-label={showSidebar ? t.hideSidebar : t.showSidebar}
        title={showSidebar ? t.hideSidebar : t.showSidebar}
        on:click={() => (sidebarOpen = !sidebarOpen)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M9 4v16" />
          {#if showSidebar}
            <path d="M14 12h-4" />
            <path d="m13 9-3 3 3 3" />
          {:else}
            <path d="M10 12h4" />
            <path d="m11 9 3 3-3 3" />
          {/if}
        </svg>
      </button>
    </div>
    <div class="toolbar-drag-region"></div>
    <div class="toolbar-actions">
      <button
        class="ghost icon-button update-check-button"
        class:checking={updateState === "checking"}
        type="button"
        disabled={updateBusy || updateDialogOpen}
        aria-busy={updateState === "checking"}
        aria-label={t.checkUpdate}
        title={updateState === "checking" ? t.checkingUpdate : t.checkUpdate}
        on:click={checkForUpdates}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20 11a8 8 0 0 0-14.5-4.7L3 9" />
          <path d="M3 5v4h4" />
          <path d="M4 13a8 8 0 0 0 8 7c2.2 0 4.2-.9 5.7-2.3" />
          <path d="m14 12 2 2 4-5" />
        </svg>
      </button>
      <button
        class="ghost icon-button"
        type="button"
        disabled={repoBusy}
        aria-label={t.openMarkdownFile}
        title={t.openMarkdownFile}
        on:click={browseMarkdownFile}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
          <path d="M14 3v5h5" />
          <path d="M8 13h8" />
          <path d="M8 17h5" />
        </svg>
      </button>
      <div class="language-toggle" role="group" aria-label={t.language}>
        <button
          class:active={locale === "zh-CN"}
          type="button"
          aria-pressed={locale === "zh-CN"}
          on:click={() => setLocale("zh-CN")}
        >
          中文
        </button>
        <button
          class:active={locale === "en"}
          type="button"
          aria-pressed={locale === "en"}
          on:click={() => setLocale("en")}
        >
          EN
        </button>
      </div>
      <button
        class="ghost icon-button"
        type="button"
        aria-expanded={contextOpen}
        aria-label={contextOpen ? t.hideDetails : t.showDetails}
        title={contextOpen ? t.hideDetails : t.showDetails}
        on:click={() => (contextOpen = !contextOpen)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M15 4v16" />
          {#if contextOpen}
            <path d="M10 12h4" />
            <path d="m13 9-3 3 3 3" />
          {:else}
            <path d="M14 12h-4" />
            <path d="m11 9 3 3-3 3" />
          {/if}
        </svg>
      </button>
    </div>
  </div>

  {#if isDragHovering}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-target">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
          <path d="M14 3v5h5" />
          <path d="M12 12v6" />
          <path d="m9 15 3 3 3-3" />
        </svg>
        <span>{t.dropMarkdownFile}</span>
      </div>
    </div>
  {/if}

  <div
    class="tabbar"
    class:hidden={openViews.length <= 1}
    role="tablist"
    aria-label={t.openViews}
  >
    {#if openViews.length > 1}
      {#each openViews as view (view.id)}
        <div class="tab-item" class:active={view.id === activeViewId}>
          <button
            class="tab-main"
            type="button"
            role="tab"
            aria-selected={view.id === activeViewId}
            title={view.path}
            on:click={() => activateView(view.id)}
          >
            <span class={`tab-dot ${view.type}`} aria-hidden="true"></span>
            <span>{view.title}</span>
          </button>
          <button
            class="tab-close"
            type="button"
            aria-label={`${t.closeView}: ${view.title}`}
            title={t.closeView}
            on:click={(event) => closeView(event, view.id)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M7 7l10 10" />
              <path d="M17 7L7 17" />
            </svg>
          </button>
        </div>
      {/each}
    {/if}
  </div>

  {#if showSidebar}
  <aside class="sidebar">
    <div class="brand">
      <div>
        <h1>{repoName(repoPath) || t.memoryRepo}</h1>
        <p>{snapshot?.counts.markdown ?? 0} {t.docs} · {snapshot?.counts.mermaid ?? 0} {t.diagrams}</p>
      </div>
    </div>

    <section class="repo-picker">
      <label for="recent-repo">{t.recentRepos}</label>
      <div class="repo-recent-row">
        <select
          id="recent-repo"
          bind:value={selectedRecentRepoPath}
          disabled={!recentRepoPaths.length || repoBusy}
          aria-label={t.recentRepos}
          title={selectedRecentRepoPath || t.noRecentRepos}
          on:change={(event) => handleRecentRepoChange(event)}
        >
          {#if recentRepoPaths.length}
            {#each recentRepoPaths as path (path)}
              <option value={path}>{repoName(path)}</option>
            {/each}
          {:else}
            <option value="">{t.noRecentRepos}</option>
          {/if}
        </select>
        {#if selectedRecentRepoPath}
          <button
            class="repo-info"
            type="button"
            title={selectedRecentRepoPath}
            data-path={selectedRecentRepoPath}
            aria-label={`${t.path}: ${selectedRecentRepoPath}`}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 11v5" />
              <path d="M12 8h.01" />
            </svg>
          </button>
        {/if}
      </div>
      <button
        class="repo-open"
        type="button"
        disabled={repoBusy}
        on:click={browseRepo}
      >
        {t.chooseNewRepo}
      </button>
    </section>

    <div class="search-row">
      <input class="search" bind:value={query} placeholder={t.searchPlaceholder} disabled={!snapshot} />
      <button
        class="ghost icon-button search-refresh"
        type="button"
        disabled={!repoPath || repoBusy}
        aria-label={t.refresh}
        title={t.refresh}
        on:click={() => loadRepo(repoPath, { preserveCurrentDocument: true })}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20 6v5h-5" />
          <path d="M4 18v-5h5" />
          <path d="M18.4 9A7 7 0 0 0 6.2 7.2L4 9.3" />
          <path d="M5.6 15A7 7 0 0 0 17.8 16.8L20 14.7" />
        </svg>
      </button>
    </div>

    <nav class="tree" aria-label={t.memoryFiles}>
      {#if visibleNodes.length}
        {#each visibleNodes as node (node.id)}
          <button
            class={nodeClass(node)}
            style={`--depth: ${node.depth}`}
            disabled={!node.path && !node.children.length}
            type="button"
            aria-expanded={node.path ? undefined : !collapsedFolderIds.has(node.id)}
            on:click={() => handleNodeClick(node)}
            title={node.path ?? displayNodeTitle(node)}
          >
            {#if !node.path}
              <span class="folder-chevron" aria-hidden="true"></span>
            {:else}
              <span class="folder-chevron placeholder" aria-hidden="true"></span>
            {/if}
            <span class="node-title">{displayNodeTitle(node)}</span>
            {#if node.path}
              <span class="node-meta">{formatKind(node.kind)}{node.title.endsWith(".md") ? "" : ""}</span>
            {/if}
          </button>
        {/each}
      {:else}
        <div class="empty">{snapshot ? t.noMatches : t.chooseRepoFirst}</div>
      {/if}
    </nav>
  </aside>
  {/if}

  <section class="content">
    <header class="reader-head">
      <div class="reader-head-main">
        <div class="reader-title">
          <div class="eyebrow">{formatKind(headerKind)}</div>
          <h2>{headerTitle}</h2>
          <p>{headerPath}</p>
        </div>
      </div>
      <div class="head-actions">
        <button
          class="ghost icon-button"
          type="button"
          disabled={!current}
          aria-label={t.findDocument}
          title={t.findDocument}
          on:click={openFind}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="11" cy="11" r="6" />
            <path d="m16 16 4 4" />
          </svg>
        </button>
        {#if activeViewIsFile || !snapshot}
          <button
            class="ghost icon-button"
            type="button"
            disabled={repoBusy}
            aria-label={t.chooseNewRepo}
            title={t.chooseNewRepo}
            on:click={browseRepo}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M3 6.5A2.5 2.5 0 0 1 5.5 4H10l2 2h6.5A2.5 2.5 0 0 1 21 8.5v8A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
              <path d="M3 9h18" />
            </svg>
          </button>
        {/if}
        <span class={`status ${status}`}>{t.status[status]}</span>
      </div>
    </header>

    <div class="reader-stage">
      {#if error}
        <div class="error">{error}</div>
      {/if}

      {#if findOpen && current}
        <div class="find-bar" role="search" aria-label={t.findDocument}>
          <input
            class="find-input"
            bind:this={findInput}
            value={findQuery}
            type="search"
            placeholder={t.findPlaceholder}
            aria-label={t.findDocument}
            on:input={handleFindInput}
          />
          <span class="find-status" class:empty={Boolean(findQuery.trim() && !findMatchCount)}>
            {findStatus}
          </span>
          <button
            class="ghost icon-button"
            type="button"
            disabled={!findMatchCount}
            aria-label={t.findPrevious}
            title={t.findPrevious}
            on:click={() => moveFindMatch(-1)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="m7 14 5-5 5 5" />
            </svg>
          </button>
          <button
            class="ghost icon-button"
            type="button"
            disabled={!findMatchCount}
            aria-label={t.findNext}
            title={t.findNext}
            on:click={() => moveFindMatch(1)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="m7 10 5 5 5-5" />
            </svg>
          </button>
          <button
            class="ghost icon-button"
            type="button"
            aria-label={t.closeFind}
            title={t.closeFind}
            on:click={closeFind}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M7 7l10 10" />
              <path d="M17 7L7 17" />
            </svg>
          </button>
        </div>
      {/if}

      {#if current}
        <article class="reader">
          {@html renderedHtml}
        </article>
      {:else}
        <section class="repo-empty">
          <div>
            <div class="eyebrow">{t.memoryRepo}</div>
            <h2>{t.noRepoTitle}</h2>
            <p>{t.noRepoBody}</p>
          </div>
          <div class="repo-empty-actions">
            <button class="repo-open" type="button" disabled={repoBusy} on:click={browseRepo}>
              {t.chooseNewRepo}
            </button>
            <button class="ghost" type="button" disabled={repoBusy} on:click={browseMarkdownFile}>
              {t.openMarkdownFile}
            </button>
          </div>
        </section>
      {/if}
    </div>
  </section>

  {#if contextOpen}
    <aside class="context">
      {#if docHeadings.length}
        <section>
          <h3>{t.outline}</h3>
          <nav class="outline" aria-label={t.outline}>
            {#each docHeadings as heading (heading.id)}
              <button
                class="outline-row"
                class:top-level={heading.level <= 2}
                type="button"
                style={`--indent: ${headingIndent(heading.level)}`}
                title={heading.title}
                on:click={() => scrollToReaderAnchor(heading.id)}
              >
                <span>{heading.title}</span>
              </button>
            {/each}
          </nav>
        </section>
      {/if}

      {#if !activeViewIsFile}
        <section>
          <h3>{t.readChain}</h3>
          {#if current?.read_chain.length}
            <div class="chain">
              {#each current.read_chain as item}
                <button type="button" on:click={() => openDocument(item.path)}>
                  <span>{formatChainLabel(item.label)}</span>
                  <strong>{item.title}</strong>
                </button>
              {/each}
            </div>
          {:else}
            <p class="muted">{t.noChain}</p>
          {/if}
        </section>
      {/if}

      <section>
        <h3>{t.file}</h3>
        <dl>
          <dt>{t.kind}</dt>
          <dd>{formatKind(activeViewIsFile ? "markdown_file" : current?.kind)}</dd>
          <dt>{t.path}</dt>
          <dd>{activeViewIsFile ? current?.path ?? "-" : current?.relative_path ?? "-"}</dd>
          <dt>{t.mermaid}</dt>
          <dd>{current?.has_mermaid ? t.yes : t.no}</dd>
        </dl>
      </section>
    </aside>
  {/if}

  {#if updateToastMessage}
    <div class="update-toast" class:error={updateToastTone === "error"} role="status">
      <span class="update-toast-icon" aria-hidden="true">
        {updateToastTone === "error" ? "!" : ""}
      </span>
      <span>{updateToastMessage}</span>
    </div>
  {/if}

  {#if updateDialogOpen}
    <div class="update-modal" role="dialog" aria-modal="true" aria-label={t.updateAvailable}>
      <section class="update-dialog">
        <div class="update-dialog-head">
          <div>
            <div class="eyebrow">{t.checkUpdate}</div>
            <h2>{t.updateAvailable}</h2>
            <p>{t.updateDialogIntro}</p>
          </div>
          <button
            class="ghost icon-button"
            type="button"
            disabled={updateInstalling}
            aria-label={t.closeUpdateDialog}
            title={t.closeUpdateDialog}
            on:click={closeUpdateDialog}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        <dl class="update-meta">
          <div>
            <dt>{t.updateVersion}</dt>
            <dd>{pendingUpdateVersion}</dd>
          </div>
          {#if pendingUpdateDate}
            <div>
              <dt>{t.updateDate}</dt>
              <dd>{formatUpdateDate(pendingUpdateDate)}</dd>
            </div>
          {/if}
        </dl>

        <section class="update-notes" aria-label={t.updateReleaseNotes}>
          <h3>{t.updateReleaseNotes}</h3>
          {#if renderedUpdateNotes}
            <div class="update-notes-content">
              {@html renderedUpdateNotes}
            </div>
          {:else}
            <p class="update-empty-notes">{t.updateNoReleaseNotes}</p>
          {/if}
        </section>

        {#if updateInstalling || updateError}
          <div class="update-progress-panel" class:error={updateError}>
            <div class="update-progress-line">
              <span>{updateMessage || t.updateProgress}</span>
              {#if updateProgress !== null && updateInstalling}
                <strong>{updateProgress}%</strong>
              {/if}
            </div>
            {#if updateInstalling}
              <div
                class="update-progress-track"
                class:indeterminate={updateProgress === null}
                role="progressbar"
                aria-label={t.updateProgress}
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={updateProgress ?? undefined}
              >
                {#if updateProgress !== null}
                  <div class="update-progress-fill" style={`width: ${updateProgress}%`}></div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}

        <div class="update-dialog-actions">
          <button
            class="ghost"
            type="button"
            disabled={updateInstalling}
            on:click={closeUpdateDialog}
          >
            {t.updateLater}
          </button>
          <button
            class="primary"
            type="button"
            disabled={updateInstalling}
            on:click={installPendingUpdate}
          >
            {updateInstalling ? t.installingUpdate : t.installUpdate}
          </button>
        </div>
      </section>
    </div>
  {/if}

  {#if zoomedDiagramHtml}
    <div class="diagram-modal" role="dialog" aria-modal="true" aria-label={t.diagramDetail}>
      <div class="diagram-modal-head">
        <div>
          <div class="eyebrow">{t.diagram}</div>
          <h2>{zoomedDiagramTitle}</h2>
        </div>
        <div class="diagram-tools">
          <button type="button" on:click={() => adjustZoom(-0.2)} aria-label={t.zoomOut} title={t.zoomOut}>
            -
          </button>
          <button type="button" on:click={resetDiagramView} aria-label={t.fitDiagram} title={`${t.fitDiagram} (${Math.round(zoomLevel * 100)}%)`}>
            {t.fitShort}
          </button>
          <button type="button" on:click={() => adjustZoom(0.2)} aria-label={t.zoomIn} title={t.zoomIn}>
            +
          </button>
          <button
            type="button"
            class:copied={copyDiagramState === "copied"}
            class:error={copyDiagramState === "error"}
            on:click={copyZoomedDiagram}
            disabled={copyDiagramState === "copying"}
            aria-label={copyDiagramButtonText}
            title={copyDiagramButtonTitle}
          >
            {copyDiagramButtonText}
          </button>
          <button type="button" on:click={closeDiagram} aria-label={t.closeDiagram} title={t.closeDiagram}>
            x
          </button>
        </div>
      </div>
      <div
        class:panning={isPanning}
        class="diagram-modal-body"
        role="application"
        aria-label={t.diagramViewer}
        bind:this={diagramViewport}
        on:wheel={handleDiagramWheel}
        on:pointerdown={startPan}
        on:pointermove={movePan}
        on:pointerup={endPan}
        on:pointercancel={endPan}
        on:pointerleave={endPan}
      >
        <div
          class="diagram-canvas"
          style={`transform: translate(${panX}px, ${panY}px) scale(${zoomLevel})`}
        >
          {@html zoomedDiagramHtml}
        </div>
      </div>
    </div>
  {/if}
</main>
