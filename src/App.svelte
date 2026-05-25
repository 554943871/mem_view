<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
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
  type LinkTarget = { path: string; anchor: string };
  type Locale = "zh-CN" | "en";
  type StatusKey = "idle" | "loading" | "indexing" | "ready" | "opening" | "error";
  type MessagePack = {
    docs: string;
    diagrams: string;
    refresh: string;
    memoryRepo: string;
    recentRepos: string;
    noRecentRepos: string;
    chooseNewRepo: string;
    chooseRepoTitle: string;
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
    status: Record<StatusKey, string>;
    kinds: Record<string, string>;
    chainLabels: Record<string, string>;
    folderTitles: Record<string, string>;
  };

  const localeStorageKey = "memView.locale";
  const repoPathStorageKey = "memView.repoPath";
  const recentRepoPathsStorageKey = "memView.recentRepoPaths";
  const recentRepoLimit = 8;
  const messages: Record<Locale, MessagePack> = {
    "zh-CN": {
      docs: "文档",
      diagrams: "图",
      refresh: "刷新",
      memoryRepo: "记忆库",
      recentRepos: "最近打开",
      noRecentRepos: "暂无最近记忆库",
      chooseNewRepo: "选择新记忆库",
      chooseRepoTitle: "选择记忆库目录",
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
      recentRepos: "Recent repos",
      noRecentRepos: "No recent repos",
      chooseNewRepo: "Choose New Repo",
      chooseRepoTitle: "Choose Memory Repo",
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
    securityLevel: "strict"
  });

  let snapshot: RepoSnapshot | null = null;
  let current: Document | null = null;
  let renderedHtml = "";
  let query = "";
  let status: StatusKey = "idle";
  let error = "";
  let repoPath = getInitialRepoPath();
  let collapsedFolderIds = new Set<string>();
  let recentRepoPaths = getInitialRecentRepoPaths(repoPath);
  let selectedRecentRepoPath = recentRepoPaths[0] ?? repoPath;
  let sidebarOpen = false;
  let contextOpen = true;
  let zoomedDiagramHtml = "";
  let zoomedDiagramTitle = "";
  let zoomLevel = 1;
  let diagramViewport: HTMLDivElement | null = null;
  let panX = 32;
  let panY = 32;
  let isPanning = false;
  let panStartX = 0;
  let panStartY = 0;
  let panOriginX = 0;
  let panOriginY = 0;
  let locale: Locale = getInitialLocale();

  $: t = messages[locale];
  $: repoBusy = status === "indexing" || status === "opening";
  $: flatTree = snapshot ? flattenTree(snapshot.tree, 0, collapsedFolderIds) : [];
  $: visibleNodes = snapshot
    ? query.trim()
      ? flattenDocs(searchDocs(snapshot.docs, query))
      : flatTree
    : [];
  $: docHeadings = current ? getDocumentHeadings(current.markdown) : [];

  onMount(() => {
    document.documentElement.lang = locale;
    document.addEventListener("click", handleDocumentClick);
    window.addEventListener("resize", handleWindowResize);
    if (repoPath) {
      void loadRepo(repoPath);
    }
  });

  onDestroy(() => {
    document.removeEventListener("click", handleDocumentClick);
    window.removeEventListener("resize", handleWindowResize);
  });

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
    await tick();
    enhanceRenderedTables();
    await renderMermaid();
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

  function handleRecentRepoChange(event: Event) {
    const nextPath = event.currentTarget instanceof HTMLSelectElement
      ? event.currentTarget.value
      : selectedRecentRepoPath;
    if (!nextPath || nextPath === repoPath || repoBusy) {
      return;
    }

    void loadRepo(nextPath);
  }

  async function loadRepo(path = repoPath) {
    const nextRepoPath = path.trim();
    if (!nextRepoPath) {
      status = "idle";
      snapshot = null;
      current = null;
      renderedHtml = "";
      error = "";
      return;
    }

    status = "indexing";
    error = "";
    try {
      snapshot = await invoke<RepoSnapshot>("scan_repo", { repoPath: nextRepoPath });
      repoPath = snapshot.root_path;
      collapsedFolderIds = getDefaultCollapsedFolderIds(snapshot.tree);
      rememberRepoPath(snapshot.root_path);
      status = "ready";
      const entry =
        snapshot.docs.find((doc) => doc.relative_path === "README.md") ??
        snapshot.docs.find((doc) => doc.relative_path === "baseline/README.md") ??
        snapshot.docs[0];
      if (entry) {
        await openDocument(entry.path);
      }
    } catch (err) {
      snapshot = null;
      current = null;
      renderedHtml = "";
      error = String(err);
      status = "error";
    }
  }

  async function openDocument(path: string) {
    if (!repoPath) {
      status = "idle";
      return;
    }

    status = "opening";
    error = "";
    try {
      current = await invoke<Document>("read_document", { repoPath, path });
      renderedHtml = renderMarkdown(current.markdown);
      status = "ready";
      await tick();
      enhanceRenderedTables();
      await renderMermaid();
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
          <button class="diagram-zoom" type="button" aria-label="${t.enlargeDiagram}" title="${t.enlargeDiagram}"></button>
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
      current?.path === node.path ? "active" : "",
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

    const zoomButton = target.closest<HTMLButtonElement>(".diagram-zoom");
    if (!zoomButton || !zoomButton.closest(".reader")) {
      return;
    }

    const frame = zoomButton.closest<HTMLElement>(".diagram-frame");
    const svg = frame?.querySelector<SVGElement>(".mermaid svg");
    if (!svg) {
      return;
    }

    zoomedDiagramHtml = serializeDiagramSvg(svg);
    zoomedDiagramTitle = current?.title ?? t.mermaidDiagram;
    resetDiagramView();
    await tick();
    fitDiagramToViewport();
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
    await openDocument(target.path);
    if (target.anchor) {
      await scrollToReaderAnchor(target.anchor);
    }
    return true;
  }

  function resolveLinkedDocument(href: string): LinkTarget | null {
    if (!snapshot || !current) {
      return null;
    }

    const { pathPart, anchor } = splitHref(href);
    if (isExternalHref(pathPart)) {
      return null;
    }

    const normalizedPath = resolveRepoRelativePath(pathPart);
    if (normalizedPath === null) {
      return null;
    }

    const doc = findDocByLinkPath(normalizedPath);
    return doc ? { path: doc.path, anchor } : null;
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

  function normalizePathname(value: string) {
    return value.replace(/\\/g, "/");
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
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && zoomedDiagramHtml) {
      closeDiagram();
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
    clone.style.display = "block";
    clone.style.maxWidth = "none";
    clone.style.background = "#ffffff";
    return clone.outerHTML;
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
  class:sidebar-closed={!sidebarOpen}
  class:context-closed={!contextOpen}
>
  {#if sidebarOpen}
  <aside class="sidebar">
    <div class="brand">
      <div>
        <h1>memView</h1>
        <p>{snapshot?.counts.markdown ?? 0} {t.docs} · {snapshot?.counts.mermaid ?? 0} {t.diagrams}</p>
      </div>
      <div class="brand-actions">
        <button
          class="ghost icon-button"
          type="button"
          aria-expanded={sidebarOpen}
          aria-label={t.hideSidebar}
          title={t.hideSidebar}
          on:click={() => (sidebarOpen = false)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <path d="M9 4v16" />
            <path d="M14 12h-4" />
            <path d="m13 9-3 3 3 3" />
          </svg>
        </button>
      </div>
    </div>

    <section class="repo-picker">
      <label for="recent-repo">{t.memoryRepo}</label>
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
        on:click={() => loadRepo(repoPath)}
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
        {#if !sidebarOpen}
          <button
            class="ghost icon-button"
            type="button"
            aria-expanded={sidebarOpen}
            aria-label={t.showSidebar}
            title={t.showSidebar}
            on:click={() => (sidebarOpen = true)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="3" y="4" width="18" height="16" rx="2" />
              <path d="M9 4v16" />
              <path d="M10 12h4" />
              <path d="m11 9 3 3-3 3" />
            </svg>
          </button>
        {/if}
        <div class="reader-title">
          <div class="eyebrow">{formatKind(current?.kind ?? "repo")}</div>
          <h2>{current?.title ?? (repoPath ? t.status[status] : t.noRepoTitle)}</h2>
          <p>{current?.relative_path ?? snapshot?.root_path ?? (repoPath || t.noRepoSelected)}</p>
        </div>
      </div>
      <div class="head-actions">
        <span class={`status ${status}`}>{t.status[status]}</span>
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
    </header>

    {#if error}
      <div class="error">{error}</div>
    {/if}

    {#if snapshot}
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
        </div>
      </section>
    {/if}
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

      <section>
        <h3>{t.file}</h3>
        <dl>
          <dt>{t.kind}</dt>
          <dd>{formatKind(current?.kind)}</dd>
          <dt>{t.path}</dt>
          <dd>{current?.relative_path ?? "-"}</dd>
          <dt>{t.mermaid}</dt>
          <dd>{current?.has_mermaid ? t.yes : t.no}</dd>
        </dl>
      </section>
    </aside>
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
