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

  const repoPath = "/Users/god/project/easy-kid-mem";
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
  let status = "Loading";
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

  $: flatTree = snapshot ? flattenTree(snapshot.tree) : [];
  $: visibleNodes = query.trim() ? flattenDocs(searchDocs(snapshot?.docs ?? [], query)) : flatTree;

  loadRepo();

  onMount(() => {
    document.addEventListener("click", handleDocumentClick);
    window.addEventListener("resize", handleWindowResize);
  });

  onDestroy(() => {
    document.removeEventListener("click", handleDocumentClick);
    window.removeEventListener("resize", handleWindowResize);
  });

  async function loadRepo() {
    status = "Indexing";
    error = "";
    try {
      snapshot = await invoke<RepoSnapshot>("scan_repo", { repoPath });
      status = "Ready";
      const entry =
        snapshot.docs.find((doc) => doc.relative_path === "README.md") ??
        snapshot.docs.find((doc) => doc.relative_path === "baseline/README.md") ??
        snapshot.docs[0];
      if (entry) {
        await openDocument(entry.path);
      }
    } catch (err) {
      error = String(err);
      status = "Error";
    }
  }

  async function openDocument(path: string) {
    status = "Opening";
    error = "";
    try {
      current = await invoke<Document>("read_document", { path });
      renderedHtml = renderMarkdown(current.markdown);
      status = "Ready";
      await tick();
      await renderMermaid();
    } catch (err) {
      error = String(err);
      status = "Error";
    }
  }

  function renderMarkdown(source: string) {
    return markdown.render(source).replace(
      /<pre><code class="language-mermaid">([\s\S]*?)<\/code><\/pre>/g,
      (_, encoded: string) => `
        <figure class="diagram-frame">
          <button class="diagram-zoom" type="button" aria-label="Enlarge diagram" title="Enlarge diagram"></button>
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
    zoomedDiagramTitle = current?.title ?? "Mermaid diagram";
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
        <p>{snapshot?.counts.markdown ?? 0} docs · {snapshot?.counts.mermaid ?? 0} diagrams</p>
      </div>
      <button class="ghost" type="button" on:click={loadRepo}>Refresh</button>
    </div>

    <input class="search" bind:value={query} placeholder="Search title or path" />

    <nav class="tree" aria-label="Memory files">
      {#if visibleNodes.length}
        {#each visibleNodes as node (node.id)}
          <button
            class={nodeClass(node)}
            style={`--depth: ${node.depth}`}
            disabled={!node.path}
            type="button"
            on:click={() => node.path && openDocument(node.path)}
            title={node.path ?? node.title}
          >
            <span class="node-title">{node.title}</span>
            {#if node.path}
              <span class="node-meta">{node.kind}{node.title.endsWith(".md") ? "" : ""}</span>
            {/if}
          </button>
        {/each}
      {:else}
        <div class="empty">No matches</div>
      {/if}
    </nav>
  </aside>

  <section class="content">
    <header class="reader-head">
      <div>
        <div class="eyebrow">{current?.kind ?? "repo"}</div>
        <h2>{current?.title ?? "Loading"}</h2>
        <p>{current?.relative_path ?? repoPath}</p>
      </div>
      <div class="head-actions">
        <span class={`status ${status.toLowerCase()}`}>{status}</span>
        <button class="ghost iconish" type="button" on:click={() => (contextOpen = !contextOpen)}>
          {contextOpen ? "Hide" : "Info"}
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
        <h3>Read Chain</h3>
        {#if current?.read_chain.length}
          <div class="chain">
            {#each current.read_chain as item}
              <button type="button" on:click={() => openDocument(item.path)}>
                <span>{item.label}</span>
                <strong>{item.title}</strong>
              </button>
            {/each}
          </div>
        {:else}
          <p class="muted">No chain for this file.</p>
        {/if}
      </section>

      <section>
        <h3>File</h3>
        <dl>
          <dt>Kind</dt>
          <dd>{current?.kind ?? "-"}</dd>
          <dt>Path</dt>
          <dd>{current?.relative_path ?? "-"}</dd>
          <dt>Mermaid</dt>
          <dd>{current?.has_mermaid ? "Yes" : "No"}</dd>
        </dl>
      </section>
    </aside>
  {/if}

  {#if zoomedDiagramHtml}
    <div class="diagram-modal" role="dialog" aria-modal="true" aria-label="Diagram detail">
      <div class="diagram-modal-head">
        <div>
          <div class="eyebrow">Diagram</div>
          <h2>{zoomedDiagramTitle}</h2>
        </div>
        <div class="diagram-tools">
          <button type="button" on:click={() => adjustZoom(-0.2)} aria-label="Zoom out" title="Zoom out">
            -
          </button>
          <button type="button" on:click={resetDiagramView} aria-label="Fit diagram" title={`Fit diagram (${Math.round(zoomLevel * 100)}%)`}>
            Fit
          </button>
          <button type="button" on:click={() => adjustZoom(0.2)} aria-label="Zoom in" title="Zoom in">
            +
          </button>
          <button type="button" on:click={closeDiagram} aria-label="Close diagram" title="Close diagram">
            x
          </button>
        </div>
      </div>
      <div
        class:panning={isPanning}
        class="diagram-modal-body"
        role="application"
        aria-label="Diagram viewer"
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
