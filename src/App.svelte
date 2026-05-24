<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
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
  type SvgBox = { x: number; y: number; width: number; height: number };
  type Locale = "zh-CN" | "en";
  type StatusKey = "loading" | "indexing" | "ready" | "opening" | "error";
  type MessagePack = {
    docs: string;
    diagrams: string;
    refresh: string;
    searchPlaceholder: string;
    memoryFiles: string;
    noMatches: string;
    language: string;
    hide: string;
    info: string;
    readChain: string;
    noChain: string;
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

  const repoPath = "/Users/god/project/easy-kid-mem";
  const localeStorageKey = "memView.locale";
  const messages: Record<Locale, MessagePack> = {
    "zh-CN": {
      docs: "文档",
      diagrams: "图",
      refresh: "刷新",
      searchPlaceholder: "搜索标题或路径",
      memoryFiles: "记忆文件",
      noMatches: "没有匹配",
      language: "语言",
      hide: "隐藏",
      info: "信息",
      readChain: "阅读链",
      noChain: "这个文件没有阅读链。",
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
      searchPlaceholder: "Search title or path",
      memoryFiles: "Memory files",
      noMatches: "No matches",
      language: "Language",
      hide: "Hide",
      info: "Info",
      readChain: "Read Chain",
      noChain: "No chain for this file.",
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

  mermaid.initialize({
    startOnLoad: false,
    theme: "neutral",
    securityLevel: "strict"
  });

  let snapshot: RepoSnapshot | null = null;
  let current: Document | null = null;
  let renderedHtml = "";
  let query = "";
  let status: StatusKey = "loading";
  let error = "";
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
  $: flatTree = snapshot ? flattenTree(snapshot.tree) : [];
  $: visibleNodes = query.trim() ? flattenDocs(searchDocs(snapshot?.docs ?? [], query)) : flatTree;

  loadRepo();

  onMount(() => {
    document.documentElement.lang = locale;
    document.addEventListener("click", handleDocumentClick);
    window.addEventListener("resize", handleWindowResize);
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

  async function loadRepo() {
    status = "indexing";
    error = "";
    try {
      snapshot = await invoke<RepoSnapshot>("scan_repo", { repoPath });
      status = "ready";
      const entry =
        snapshot.docs.find((doc) => doc.relative_path === "README.md") ??
        snapshot.docs.find((doc) => doc.relative_path === "baseline/README.md") ??
        snapshot.docs[0];
      if (entry) {
        await openDocument(entry.path);
      }
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  async function openDocument(path: string) {
    status = "opening";
    error = "";
    try {
      current = await invoke<Document>("read_document", { path });
      renderedHtml = renderMarkdown(current.markdown);
      status = "ready";
      await tick();
      await renderMermaid();
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  function renderMarkdown(source: string) {
    return markdown.render(source).replace(
      /<pre><code class="language-mermaid">([\s\S]*?)<\/code><\/pre>/g,
      (_, encoded: string) => `
        <figure class="diagram-frame">
          <button class="diagram-zoom" type="button" aria-label="${t.enlargeDiagram}" title="${t.enlargeDiagram}"></button>
          <div class="mermaid">${decodeHtml(encoded)}</div>
        </figure>
      `
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

  function flattenTree(nodes: TreeNode[], depth = 0): FlatNode[] {
    return nodes.flatMap((node) => [
      { ...node, depth },
      ...flattenTree(node.children, depth + 1)
    ]);
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
      node.kind
    ].join(" ");
  }

  async function handleDocumentClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
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
    const factor = event.deltaY < 0 ? 1.12 : 0.88;
    setZoom(zoomLevel * factor, event.clientX, event.clientY);
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

<main class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div>
        <h1>memView</h1>
        <p>{snapshot?.counts.markdown ?? 0} {t.docs} · {snapshot?.counts.mermaid ?? 0} {t.diagrams}</p>
      </div>
      <button class="ghost" type="button" on:click={loadRepo}>{t.refresh}</button>
    </div>

    <input class="search" bind:value={query} placeholder={t.searchPlaceholder} />

    <nav class="tree" aria-label={t.memoryFiles}>
      {#if visibleNodes.length}
        {#each visibleNodes as node (node.id)}
          <button
            class={nodeClass(node)}
            style={`--depth: ${node.depth}`}
            disabled={!node.path}
            type="button"
            on:click={() => node.path && openDocument(node.path)}
            title={node.path ?? displayNodeTitle(node)}
          >
            <span class="node-title">{displayNodeTitle(node)}</span>
            {#if node.path}
              <span class="node-meta">{formatKind(node.kind)}{node.title.endsWith(".md") ? "" : ""}</span>
            {/if}
          </button>
        {/each}
      {:else}
        <div class="empty">{t.noMatches}</div>
      {/if}
    </nav>
  </aside>

  <section class="content">
    <header class="reader-head">
      <div>
        <div class="eyebrow">{formatKind(current?.kind ?? "repo")}</div>
        <h2>{current?.title ?? t.status.loading}</h2>
        <p>{current?.relative_path ?? repoPath}</p>
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
        <button class="ghost iconish" type="button" on:click={() => (contextOpen = !contextOpen)}>
          {contextOpen ? t.hide : t.info}
        </button>
      </div>
    </header>

    {#if error}
      <div class="error">{error}</div>
    {/if}

    <article class="reader">
      {@html renderedHtml}
    </article>
  </section>

  {#if contextOpen}
    <aside class="context">
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
