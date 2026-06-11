<script lang="ts">
  import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
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

  type DocumentContentType = "markdown" | "html";

  type DocMeta = {
    id: string;
    title: string;
    path: string;
    relative_path: string;
    kind: string;
    content_type: DocumentContentType;
    modified_at_unix_ms: number;
    has_mermaid: boolean;
  };

  type RepoSnapshot = {
    root_path: string;
    tree: TreeNode[];
    docs: DocMeta[];
    counts: {
      documents: number;
      markdown: number;
      html: number;
      mermaid: number;
      requirements: number;
    };
  };
  type GitPullResult = {
    rootPath: string;
    message: string;
  };

  type Document = {
    id: string;
    title: string;
    path: string;
    relative_path: string;
    kind: string;
    content: string;
    content_type: DocumentContentType;
    modified_at_unix_ms: number;
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
  type MarkdownRenderEnv = {
    headingCounts: Map<string, number>;
    nodeCounts: Map<string, number>;
    documentPath: string;
  };
  type DocumentRenderer = {
    id: DocumentContentType;
    render: (document: Document) => void;
    afterRender: (document: Document) => Promise<void>;
    getHeadings: (document: Document) => DocHeading[];
    getCoveredNodes: (rect: AnnotationRect, document: Document) => AnnotationCoveredNode[];
    clearFindHighlights: () => void;
    highlightFindMatches: (needle: string) => HTMLElement[];
    setActiveFindMatch: (matches?: HTMLElement[], scroll?: boolean) => void;
    findAnchor: (anchor: string) => HTMLElement | null;
  };
  type ViewType = "repo" | "file";
  type CopyDiagramState = "idle" | "copying" | "copied" | "error";
  const ANNOTATION_CAPTURE_PADDING = 24;
  type AnnotationSourceLines = { start: number; end: number };
  type AnnotationRect = {
    left: number;
    top: number;
    width: number;
    height: number;
    scrollTop: number;
    scrollLeft: number;
    readerWidth: number;
    readerHeight: number;
  };
  type AnnotationCoveredNode = {
    nodeId: string;
    type: string;
    sourceLines: AnnotationSourceLines | null;
    headingPath: string[];
    textExcerpt: string;
    intersectionRatio: number;
    isPrimary: boolean;
  };
  type AnnotationCaptureRect = {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  type AnnotationCaptureStatus = "captured" | "unavailable";
  type AnnotationVisualEvidence = {
    screenshotPath: string | null;
    capturePadding: number;
    captureRect: AnnotationCaptureRect | null;
    captureStatus: AnnotationCaptureStatus;
    captureError: string | null;
  };
  type AnnotationDocumentMeta = {
    path: string;
    relativePath: string;
    repoPath: string | null;
    title: string;
    kind: string;
  };
  type AnnotationItem = {
    id: string;
    note: string;
    rect: AnnotationRect;
    notePosition?: AnnotationNotePosition | null;
    noteCollapsed?: boolean;
    coveredNodes: AnnotationCoveredNode[];
    document: AnnotationDocumentMeta;
  };
  type AnnotationNotePosition = {
    left: number;
    top: number;
  };
  type AnnotationNoteDrag = {
    id: string;
    pointerId: number;
    startClientX: number;
    startClientY: number;
    startLeft: number;
    startTop: number;
    noteWidth: number;
    noteHeight: number;
  };
  type AnnotationDraft = {
    startX: number;
    startY: number;
    rect: AnnotationRect;
  };
  type AnnotationExportPayload = {
    schemaVersion: string;
    createdAtUnixMs: number;
    app: string;
    documents: Array<{
      path: string;
      relativePath: string;
      repoPath: string | null;
      title: string;
      kind: string;
      annotations: Array<{
        id: string;
        note: string;
        rect: AnnotationRect;
        coveredNodes: AnnotationCoveredNode[];
        visualEvidence: AnnotationVisualEvidence;
      }>;
    }>;
  };
  type AnnotationExportResult = {
    annotationDirectoryPath: string;
    readmePath: string;
    prompt: string;
  };
  type OpenView = {
    id: string;
    type: ViewType;
    title: string;
    path: string;
  };
  type LoadRepoOptions = {
    preserveCurrentDocument?: boolean;
    pullBeforeScan?: boolean;
  };
  type OpenDocumentOptions = {
    restoreScrollTop?: number;
  };
  type RepoViewState = {
    currentRelativePath: string;
    collapsedFolderIds: string[];
    scrollTop: number;
    updatedAtUnixMs: number;
  };
  type NavigationEntry = {
    type: ViewType;
    path: string;
    title: string;
    repoPath: string;
    scrollTop: number;
  };
  type LinkTarget = { type: ViewType; path: string; anchor: string };
  type Locale = "zh-CN" | "en";
  type StatusKey = "idle" | "loading" | "syncing" | "indexing" | "ready" | "opening" | "error";
  type UpdateState = "idle" | "checking" | "downloading" | "installing";
  type CheckUpdateOptions = {
    notifyNoUpdate?: boolean;
    notifyError?: boolean;
  };
  type ToastTone = "info" | "error";
  type MessagePack = {
    docs: string;
    diagrams: string;
    refresh: string;
    memoryRepo: string;
    recentRepos: string;
    noRecentRepos: string;
    chooseNewRepo: string;
    openDocumentFile: string;
    lastUpdated: string;
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
    dropDocumentFile: string;
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
    navigationHistory: string;
    navigateBack: string;
    navigateForward: string;
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
    mermaidRenderFailed: string;
    mermaidErrorBody: string;
    mermaidErrorDetails: string;
    copyMermaidFixPrompt: string;
    copiedMermaidFixPrompt: string;
    copyMermaidFixPromptFailed: string;
    copyDocumentPath: string;
    copiedDocumentPath: string;
    copyDocumentPathFailed: string;
    findDocument: string;
    findPlaceholder: string;
    findPrevious: string;
    findNext: string;
    closeFind: string;
    findNoMatches: string;
    annotationMode: string;
    startAnnotation: string;
    stopAnnotation: string;
    finishAnnotations: string;
    annotationNotePlaceholder: string;
    annotationEmptyNote: string;
    annotationNoCoveredNodes: string;
    annotationNoAnnotations: string;
    annotationExporting: string;
    annotationExported: string;
    annotationExportFailed: string;
    pullFailed: string;
    editAnnotation: string;
    moveAnnotationNote: string;
    collapseAnnotationNote: string;
    expandAnnotationNote: string;
    deleteAnnotation: string;
    status: Record<StatusKey, string>;
    kinds: Record<string, string>;
    chainLabels: Record<string, string>;
    folderTitles: Record<string, string>;
  };

  const localeStorageKey = "memView.locale";
  const repoPathStorageKey = "memView.repoPath";
  const recentRepoPathsStorageKey = "memView.recentRepoPaths";
  const repoViewStatesStorageKey = "memView.repoViewStates";
  const repoViewId = "repo";
  const fileViewPrefix = "file:";
  const recentRepoLimit = 8;
  const repoViewStateLimit = 24;
  const messages: Record<Locale, MessagePack> = {
    "zh-CN": {
      docs: "文档",
      diagrams: "图",
      refresh: "拉取并刷新",
      memoryRepo: "记忆库",
      recentRepos: "快捷切换",
      noRecentRepos: "暂无最近打开",
      chooseNewRepo: "打开新记忆库",
      openDocumentFile: "打开文档文件",
      lastUpdated: "最后更新",
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
      dropDocumentFile: "松开以打开文档文件",
      dropUnsupported: "请拖入 .md、.html 或 .htm 文件",
      closeView: "关闭视图",
      openViews: "打开的视图",
      chooseRepoTitle: "选择记忆库目录",
      chooseFileTitle: "打开文档文件",
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
      navigationHistory: "导航历史",
      navigateBack: "后退",
      navigateForward: "前进",
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
      mermaidRenderFailed: "Mermaid 格式错误",
      mermaidErrorBody: "这个图暂时无法渲染。复制修复提示词后，粘贴到 Codex App 或 Claude Code 里让 AI 修复。",
      mermaidErrorDetails: "错误信息",
      copyMermaidFixPrompt: "复制修复提示词",
      copiedMermaidFixPrompt: "修复提示词已复制",
      copyMermaidFixPromptFailed: "提示词复制失败",
      copyDocumentPath: "复制完整路径",
      copiedDocumentPath: "完整路径已复制",
      copyDocumentPathFailed: "路径复制失败",
      findDocument: "查找当前文档",
      findPlaceholder: "查找当前文档",
      findPrevious: "上一个匹配",
      findNext: "下一个匹配",
      closeFind: "关闭查找",
      findNoMatches: "没有匹配",
      annotationMode: "标注模式",
      startAnnotation: "开始标注",
      stopAnnotation: "退出标注",
      finishAnnotations: "完成标注",
      annotationNotePlaceholder: "点击填写备注",
      annotationEmptyNote: "请先填写所有标注备注",
      annotationNoCoveredNodes: "这个标注没有覆盖到可识别的文档内容",
      annotationNoAnnotations: "还没有可导出的标注",
      annotationExporting: "完成中",
      annotationExported: "标注提示词已复制",
      annotationExportFailed: "标注导出失败",
      pullFailed: "Git 拉取失败",
      editAnnotation: "编辑标注备注",
      moveAnnotationNote: "拖动标注备注",
      collapseAnnotationNote: "收起标注备注",
      expandAnnotationNote: "展开标注备注",
      deleteAnnotation: "删除标注",
      status: {
        idle: "待选择",
        loading: "加载中",
        syncing: "拉取中",
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
        document_file: "文档文件",
        markdown_file: "Markdown 文件",
        html_file: "HTML 文件",
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
      refresh: "Pull & Refresh",
      memoryRepo: "Memory Repo",
      recentRepos: "Quick switch",
      noRecentRepos: "No recent repos",
      chooseNewRepo: "Open New Repo",
      openDocumentFile: "Open Document File",
      lastUpdated: "Updated",
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
      dropDocumentFile: "Drop to open document file",
      dropUnsupported: "Drop .md, .html, or .htm files only",
      closeView: "Close view",
      openViews: "Open views",
      chooseRepoTitle: "Choose Memory Repo",
      chooseFileTitle: "Open Document File",
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
      navigationHistory: "Navigation history",
      navigateBack: "Back",
      navigateForward: "Forward",
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
      mermaidRenderFailed: "Mermaid syntax error",
      mermaidErrorBody: "This diagram cannot be rendered yet. Copy the repair prompt into Codex App or Claude Code to have AI fix it.",
      mermaidErrorDetails: "Error details",
      copyMermaidFixPrompt: "Copy repair prompt",
      copiedMermaidFixPrompt: "Repair prompt copied",
      copyMermaidFixPromptFailed: "Prompt copy failed",
      copyDocumentPath: "Copy full path",
      copiedDocumentPath: "Full path copied",
      copyDocumentPathFailed: "Path copy failed",
      findDocument: "Find in document",
      findPlaceholder: "Find in current document",
      findPrevious: "Previous match",
      findNext: "Next match",
      closeFind: "Close find",
      findNoMatches: "No matches",
      annotationMode: "Annotation mode",
      startAnnotation: "Start annotation",
      stopAnnotation: "Exit annotation",
      finishAnnotations: "Finish annotations",
      annotationNotePlaceholder: "Click to add note",
      annotationEmptyNote: "Fill every annotation note first",
      annotationNoCoveredNodes: "This annotation did not cover recognizable document content",
      annotationNoAnnotations: "No annotations to export",
      annotationExporting: "Finishing",
      annotationExported: "Annotation prompt copied",
      annotationExportFailed: "Annotation export failed",
      pullFailed: "Git pull failed",
      editAnnotation: "Edit annotation note",
      moveAnnotationNote: "Move annotation note",
      collapseAnnotationNote: "Collapse annotation note",
      expandAnnotationNote: "Expand annotation note",
      deleteAnnotation: "Delete annotation",
      status: {
        idle: "Choose Repo",
        loading: "Loading",
        syncing: "Pulling",
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
        document_file: "Document file",
        markdown_file: "Markdown file",
        html_file: "HTML file",
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
  const defaultParagraphOpenRenderer = markdown.renderer.rules.paragraph_open;
  const defaultListItemOpenRenderer = markdown.renderer.rules.list_item_open;
  const defaultBlockquoteOpenRenderer = markdown.renderer.rules.blockquote_open;
  const defaultTableOpenRenderer = markdown.renderer.rules.table_open;
  const defaultTableCloseRenderer = markdown.renderer.rules.table_close;
  const defaultTableRowOpenRenderer = markdown.renderer.rules.tr_open;
  const defaultImageRenderer = markdown.renderer.rules.image;
  const defaultTextRenderer = markdown.renderer.rules.text;

  markdown.renderer.rules.heading_open = (tokens, index, options, env, self) => {
    const token = tokens[index];
    const inlineToken = tokens[index + 1];
    if (inlineToken?.type === "inline") {
      token.attrSet("id", getUniqueHeadingId(inlineToken.content, env as MarkdownRenderEnv));
      token.attrJoin("class", "reader-heading");
      token.attrSet("tabindex", "-1");
    }
    annotateSourceToken(token, "heading", env as MarkdownRenderEnv);

    return defaultHeadingOpenRenderer
      ? defaultHeadingOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.paragraph_open = (tokens, index, options, env, self) => {
    annotateSourceToken(tokens[index], "paragraph", env as MarkdownRenderEnv);
    return defaultParagraphOpenRenderer
      ? defaultParagraphOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.list_item_open = (tokens, index, options, env, self) => {
    annotateSourceToken(tokens[index], "list_item", env as MarkdownRenderEnv);
    return defaultListItemOpenRenderer
      ? defaultListItemOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.blockquote_open = (tokens, index, options, env, self) => {
    annotateSourceToken(tokens[index], "blockquote", env as MarkdownRenderEnv);
    return defaultBlockquoteOpenRenderer
      ? defaultBlockquoteOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.table_open = (tokens, index, options, env, self) => {
    annotateSourceToken(tokens[index], "table", env as MarkdownRenderEnv);
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

  markdown.renderer.rules.tr_open = (tokens, index, options, env, self) => {
    annotateSourceToken(tokens[index], "table_row", env as MarkdownRenderEnv);
    return defaultTableRowOpenRenderer
      ? defaultTableRowOpenRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.image = (tokens, index, options, env, self) => {
    const token = tokens[index];
    const resolvedSrc = resolveMarkdownImageSrc(
      token.attrGet("src"),
      env as MarkdownRenderEnv
    );
    if (resolvedSrc) {
      token.attrSet("src", resolvedSrc);
    }
    token.attrJoin("class", "reader-image");
    token.attrSet("loading", "lazy");

    return defaultImageRenderer
      ? defaultImageRenderer(tokens, index, options, env, self)
      : self.renderToken(tokens, index, options);
  };

  markdown.renderer.rules.text = (tokens, index, options, env, self) => {
    const content = tokens[index].content;
    if (!/<br\s*\/?>/i.test(content)) {
      return defaultTextRenderer
        ? defaultTextRenderer(tokens, index, options, env, self)
        : escapeHtml(content);
    }

    return renderTextWithMarkdownBreakTags(content);
  };

  markdown.renderer.rules.fence = (tokens, index, options, env, self) => {
    const token = tokens[index];
    const info = token.info.trim();
    if (info.split(/\s+/)[0]?.toLowerCase() === "mermaid") {
      const attrs = sourceTokenAttrs(token, "mermaid", env as MarkdownRenderEnv);
      return `
        <figure class="diagram-frame"${attrs}>
          <div class="diagram-actions">
            <button class="diagram-copy" type="button" aria-label="${escapeHtml(t.copyDiagram)}" title="${escapeHtml(t.copyDiagram)}"></button>
            <button class="diagram-zoom" type="button" aria-label="${escapeHtml(t.enlargeDiagram)}" title="${escapeHtml(t.enlargeDiagram)}"></button>
          </div>
          <div class="mermaid">${escapeHtml(token.content)}</div>
          <code class="mermaid-source" hidden>${escapeHtml(token.content)}</code>
        </figure>
      `;
    }

    return renderCodeToken(token, info, env as MarkdownRenderEnv);
  };

  markdown.renderer.rules.code_block = (tokens, index, _options, env, _self) => {
    const token = tokens[index];
    return renderCodeToken(token, "", env as MarkdownRenderEnv);
  };

  mermaid.initialize({
    startOnLoad: false,
    theme: "neutral",
    securityLevel: "strict",
    flowchart: {
      htmlLabels: false
    }
  });

  const documentRenderers: Record<DocumentContentType, DocumentRenderer> = {
    markdown: {
      id: "markdown",
      render: renderMarkdownDocument,
      afterRender: completeMarkdownRenderedDocumentUpdate,
      getHeadings: getMarkdownDocumentHeadings,
      getCoveredNodes: getMarkdownCoveredNodes,
      clearFindHighlights: clearMarkdownFindHighlights,
      highlightFindMatches: highlightMarkdownReaderMatches,
      setActiveFindMatch: setActiveMarkdownFindMatch,
      findAnchor: findMarkdownReaderAnchor
    },
    html: {
      id: "html",
      render: renderHtmlDocument,
      afterRender: completeHtmlRenderedDocumentUpdate,
      getHeadings: getHtmlDocumentHeadings,
      getCoveredNodes: getHtmlCoveredNodes,
      clearFindHighlights: clearHtmlFindHighlights,
      highlightFindMatches: highlightHtmlReaderMatches,
      setActiveFindMatch: setActiveHtmlFindMatch,
      findAnchor: findHtmlReaderAnchor
    }
  };

  let snapshot: RepoSnapshot | null = null;
  let current: Document | null = null;
  let repoCurrent: Document | null = null;
  let renderedHtml = "";
  let localAssetCacheVersion = Date.now();
  let activeRendererId: DocumentContentType | "" = "";
  let htmlFrameSrcdoc = "";
  let htmlFrameElement: HTMLIFrameElement | null = null;
  let htmlFrameReadyPromise: Promise<void> | null = null;
  let resolveHtmlFrameReady: (() => void) | null = null;
  let htmlFrameResizeObserver: ResizeObserver | null = null;
  let htmlFrameMutationObserver: MutationObserver | null = null;
  let htmlFrameClickHandler: ((event: MouseEvent) => void) | null = null;
  let htmlDocHeadings: DocHeading[] = [];
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
  let navigationHistory: NavigationEntry[] = [];
  let navigationIndex = -1;
  let isRestoringNavigation = false;
  let collapsedFolderIds = new Set<string>();
  let recentRepoPaths = getInitialRecentRepoPaths(repoPath);
  let selectedRecentRepoPath = recentRepoPaths[0] ?? repoPath;
  let repoViewStates = getInitialRepoViewStates();
  let autoLoadingRecentRepoPath = "";
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
  let fileOpenUnlisten: UnlistenFn | null = null;
  let startupFileOpenChecked = !isTauri();
  let findOpen = false;
  let findQuery = "";
  let findMatchCount = 0;
  let activeFindIndex = 0;
  let findInput: HTMLInputElement | null = null;
  let readerElement: HTMLElement | null = null;
  let annotationMode = false;
  let annotationDraft: AnnotationDraft | null = null;
  let annotationPointerId: number | null = null;
  let annotationsByPath = new Map<string, AnnotationItem[]>();
  let editingAnnotationId = "";
  let annotationNoteDrag: AnnotationNoteDrag | null = null;
  let annotationExporting = false;
  let annotationCaptureHidden = false;

  $: t = messages[locale];
  $: activeView = getOpenView(activeViewId);
  $: activeViewIsFile = activeView?.type === "file";
  $: activeRepoDocumentPath = activeView?.type === "repo" ? current?.path ?? "" : "";
  $: canNavigateBack = navigationIndex > 0;
  $: canNavigateForward = navigationIndex >= 0 && navigationIndex < navigationHistory.length - 1;
  $: showSidebar = sidebarOpen && !activeViewIsFile;
  $: repoBusy = status === "syncing" || status === "indexing" || status === "opening";
  $: updateBusy = updateState !== "idle";
  $: updateInstalling = updateState === "downloading" || updateState === "installing";
  $: renderedUpdateNotes = renderUpdateNotes(pendingUpdateNotes);
  $: flatTree = snapshot ? flattenTree(snapshot.tree, 0, collapsedFolderIds) : [];
  $: visibleNodes = snapshot
    ? query.trim()
      ? flattenDocs(searchDocs(snapshot.docs, query))
      : flatTree
    : [];
  $: docHeadings = current
    ? current.content_type === "html"
      ? htmlDocHeadings
      : getDocumentRenderer(current).getHeadings(current)
    : [];
  $: headerKind = current ? getDocumentDisplayKind(current, activeViewIsFile) : "repo";
  $: headerTitle = current?.title ?? (repoPath ? t.status[status] : t.noRepoTitle);
  $: headerPath = activeViewIsFile
    ? current?.path ?? activeView?.path ?? t.noRepoSelected
    : current?.relative_path ?? snapshot?.root_path ?? (repoPath || t.noRepoSelected);
  $: headerUpdatedAt = current ? formatDocumentUpdatedAt(current.modified_at_unix_ms) : "";
  $: findStatus = formatFindStatus();
  $: copyDiagramButtonText = getCopyDiagramStateText(copyDiagramState);
  $: copyDiagramButtonTitle = getCopyDiagramButtonTitle();
  $: currentDocumentKey = current ? normalizePathname(current.path) : "";
  $: currentAnnotations = currentDocumentKey
    ? annotationsByPath.get(currentDocumentKey) ?? []
    : [];
  $: annotationModeButtonLabel = annotationMode ? t.stopAnnotation : t.annotationMode;
  $: finishAnnotationButtonLabel = currentAnnotations.length
    ? `${t.finishAnnotations} ${currentAnnotations.length}`
    : t.finishAnnotations;
  $: if (
    !repoPath &&
    !snapshot &&
    !current &&
    startupFileOpenChecked &&
    status === "idle" &&
    selectedRecentRepoPath &&
    autoLoadingRecentRepoPath !== selectedRecentRepoPath
  ) {
    autoLoadingRecentRepoPath = selectedRecentRepoPath;
    void loadRepo(selectedRecentRepoPath);
  }

  onMount(() => {
    document.documentElement.lang = locale;
    document.addEventListener("click", handleDocumentClick);
    window.addEventListener("resize", handleWindowResize);
    window.addEventListener("beforeunload", handleBeforeUnload);
    void setupDragDrop();
    void initializeNativeOpenFlow();
    void checkForUpdates({ notifyNoUpdate: false, notifyError: false });
  });

  onDestroy(() => {
    document.removeEventListener("click", handleDocumentClick);
    window.removeEventListener("resize", handleWindowResize);
    window.removeEventListener("beforeunload", handleBeforeUnload);
    dragDropUnlisten?.();
    fileOpenUnlisten?.();
    teardownHtmlFrameBridge();
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

  function handleBeforeUnload() {
    saveCurrentRepoViewState();
  }

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
        void openDroppedDocumentFiles(event.payload.paths);
      });
    } catch (err) {
      console.warn("File drag and drop setup failed", err);
    }
  }

  async function initializeNativeOpenFlow() {
    let openedSystemFiles = false;

    try {
      fileOpenUnlisten = await listen<string[]>("mem-view-open-files", () => {
        void openPendingSystemDocumentFiles();
      });
      openedSystemFiles = await openPendingSystemDocumentFiles();
    } catch (err) {
      console.warn("Native file open setup failed", err);
    } finally {
      startupFileOpenChecked = true;
    }

    const initialPath = repoPath || selectedRecentRepoPath;
    if (initialPath && !openedSystemFiles) {
      void loadRepo(initialPath);
    }
  }

  async function openPendingSystemDocumentFiles() {
    if (!isTauri()) {
      return false;
    }

    try {
      const paths = await invoke<string[]>("take_pending_open_files");
      return openSystemDocumentFiles(paths);
    } catch (err) {
      console.warn("Failed to read pending native file opens", err);
      return false;
    }
  }

  async function openSystemDocumentFiles(paths: string[]) {
    const documentPaths = paths.filter(isDocumentPath);
    for (const path of documentPaths) {
      await openStandaloneDocument(path);
    }
    return documentPaths.length > 0;
  }

  async function openDroppedDocumentFiles(paths: string[]) {
    const documentPaths = paths.filter(isDocumentPath);
    if (!documentPaths.length) {
      error = t.dropUnsupported;
      status = "error";
      return;
    }

    for (const path of documentPaths) {
      await openStandaloneDocument(path);
    }
  }

  function isDocumentPath(path: string) {
    return /\.(md|html?)$/i.test(path.trim());
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

    return localStorage.getItem(repoPathStorageKey) ?? parseStoredRecentRepoPaths()[0] ?? "";
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

  function getInitialRepoViewStates() {
    const states = new Map<string, RepoViewState>();
    if (typeof localStorage === "undefined") {
      return states;
    }

    try {
      const parsed = JSON.parse(localStorage.getItem(repoViewStatesStorageKey) ?? "[]");
      const entries = Array.isArray(parsed) ? parsed : Object.entries(parsed);
      for (const entry of entries) {
        if (!Array.isArray(entry) || typeof entry[0] !== "string") {
          continue;
        }

        const state = parseRepoViewState(entry[1]);
        const key = repoViewStateKey(entry[0]);
        if (key && state) {
          states.set(key, state);
        }
      }
    } catch {
      return new Map();
    }

    return states;
  }

  function parseRepoViewState(value: unknown): RepoViewState | null {
    if (!value || typeof value !== "object") {
      return null;
    }

    const record = value as Record<string, unknown>;
    const collapsedFolderIds = Array.isArray(record.collapsedFolderIds)
      ? record.collapsedFolderIds.filter((id): id is string => typeof id === "string")
      : [];

    return {
      currentRelativePath: typeof record.currentRelativePath === "string"
        ? record.currentRelativePath
        : "",
      collapsedFolderIds,
      scrollTop: typeof record.scrollTop === "number" && Number.isFinite(record.scrollTop)
        ? record.scrollTop
        : 0,
      updatedAtUnixMs: typeof record.updatedAtUnixMs === "number" && Number.isFinite(record.updatedAtUnixMs)
        ? record.updatedAtUnixMs
        : 0
    };
  }

  function repoViewStateKey(path: string) {
    return normalizePathname(path.trim());
  }

  function getRepoViewState(path: string) {
    return repoViewStates.get(repoViewStateKey(path)) ?? null;
  }

  function saveCurrentRepoViewState(scrollTop = getReaderScrollTop()) {
    if (!repoPath || !snapshot) {
      return;
    }

    const key = repoViewStateKey(repoPath);
    if (!key) {
      return;
    }

    const previous = repoViewStates.get(key);
    const next = new Map(repoViewStates);
    next.set(key, {
      currentRelativePath: repoCurrent?.relative_path ?? previous?.currentRelativePath ?? "",
      collapsedFolderIds: Array.from(collapsedFolderIds),
      scrollTop,
      updatedAtUnixMs: Date.now()
    });

    repoViewStates = trimRepoViewStates(next);
    persistRepoViewStates();
  }

  function trimRepoViewStates(states: Map<string, RepoViewState>) {
    return new Map(
      Array.from(states.entries())
        .sort((entryA, entryB) => entryB[1].updatedAtUnixMs - entryA[1].updatedAtUnixMs)
        .slice(0, repoViewStateLimit)
    );
  }

  function persistRepoViewStates() {
    if (typeof localStorage === "undefined") {
      return;
    }

    localStorage.setItem(repoViewStatesStorageKey, JSON.stringify(Array.from(repoViewStates.entries())));
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

    renderDocumentContent(current);
    await completeRenderedDocumentUpdate();
  }

  async function completeRenderedDocumentUpdate() {
    await tick();
    if (current) {
      await getDocumentRenderer(current).afterRender(current);
    }
    await refreshFindHighlights();
  }

  function formatKind(kind: string | null | undefined) {
    if (!kind) {
      return "-";
    }
    return t.kinds[kind] ?? kind;
  }

  function formatDocumentUpdatedAt(value: number | null | undefined) {
    if (!value) {
      return "";
    }
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return "";
    }

    const formatted = new Intl.DateTimeFormat(locale, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false
    }).format(date).replace(/\//g, "-");
    return `${t.lastUpdated} ${formatted}`;
  }

  function getDocumentDisplayKind(document: Document, isFileView: boolean) {
    if (!isFileView) {
      return document.kind;
    }
    return document.content_type === "html" ? "html_file" : "markdown_file";
  }

  function getDocumentRenderer(document: Document) {
    return documentRenderers[document.content_type] ?? documentRenderers.markdown;
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

    if (!isRestoringNavigation) {
      updateCurrentHistoryScroll();
    }
    activeViewId = id;
    error = "";
    await renderActiveView();
    recordNavigationEntry();
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

    if (activeViewId === id) {
      updateCurrentHistoryScroll();
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
    void renderActiveView().then(() => recordNavigationEntry());
  }

  async function renderActiveView() {
    const view = getOpenView(activeViewId);
    if (!view) {
      current = null;
      clearRenderedDocument();
      status = "idle";
      return;
    }

    current = view.type === "repo" ? repoCurrent : fileDocuments.get(view.id) ?? null;
    if (current) {
      renderDocumentContent(current);
    } else {
      clearRenderedDocument();
    }
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

  async function browseDocumentFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: t.chooseFileTitle,
      filters: [{ name: "Documents", extensions: ["md", "html", "htm"] }]
    });

    if (typeof selected !== "string") {
      return;
    }

    await openStandaloneDocument(selected);
  }

  async function checkForUpdates(options: CheckUpdateOptions = {}) {
    const { notifyNoUpdate = true, notifyError = true } = options;
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
        if (notifyNoUpdate) {
          showUpdateToast(t.noUpdate);
        }
        return;
      }

      openUpdateDialog(update);
    } catch (err) {
      if (notifyError) {
        showUpdateToast(`${t.updateFailed}: ${getErrorMessage(err)}`, "error");
      } else {
        console.warn("Startup update check failed", err);
      }
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

  async function copyCurrentDocumentPath() {
    if (!current?.path) {
      return;
    }

    try {
      await copyTextToClipboard(current.path);
      showUpdateToast(t.copiedDocumentPath);
    } catch (err) {
      console.warn("Copy document path failed", err);
      showUpdateToast(`${t.copyDocumentPathFailed}: ${getErrorMessage(err)}`, "error");
    }
  }

  async function copyTextToClipboard(text: string) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();

    try {
      if (!document.execCommand("copy")) {
        throw new Error("Browser copy command failed");
      }
    } finally {
      textarea.remove();
    }
  }

  function renderUpdateNotes(source: string) {
    if (!source.trim()) {
      return "";
    }

    return markdown.render(source, createRenderEnv());
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

  function refreshCurrentRepo() {
    if (!repoPath || repoBusy) {
      return;
    }

    void loadRepo(repoPath, { preserveCurrentDocument: true, pullBeforeScan: true });
  }

  async function loadRepo(path = repoPath, options: LoadRepoOptions = {}) {
    const nextRepoPath = path.trim();
    if (!nextRepoPath) {
      status = "idle";
      snapshot = null;
      current = null;
      clearRenderedDocument();
      error = "";
      return;
    }

    if (!isRestoringNavigation) {
      updateCurrentHistoryScroll();
    }
    saveCurrentRepoViewState();
    const previousRepoPath = repoPath;
    const previousSnapshot = snapshot;
    const previousRepoCurrent = repoCurrent;
    const previousCurrent = current;
    const previousRenderedHtml = renderedHtml;
    const previousActiveRendererId = activeRendererId;
    const previousHtmlFrameSrcdoc = htmlFrameSrcdoc;
    const previousHtmlDocHeadings = htmlDocHeadings;
    const previousCollapsedFolderIds = new Set(collapsedFolderIds);
    const previousActiveViewId = activeViewId;
    const preservedRelativePath = options.preserveCurrentDocument ? repoCurrent?.relative_path : "";
    const preservedScrollTop = options.preserveCurrentDocument ? getReaderScrollTop() : undefined;
    const preservedCollapsedFolderIds = options.preserveCurrentDocument
      ? new Set(collapsedFolderIds)
      : null;

    error = "";
    if (options.pullBeforeScan) {
      status = "syncing";
      try {
        await invoke<GitPullResult>("pull_repo", { repoPath: nextRepoPath });
      } catch (err) {
        error = `${t.pullFailed}: ${getErrorMessage(err)}`;
        status = "error";
        return;
      }
    }

    status = "indexing";
    try {
      snapshot = await invoke<RepoSnapshot>("scan_repo", { repoPath: nextRepoPath });
      const restoredState = getRepoViewState(snapshot.root_path);
      repoPath = snapshot.root_path;
      upsertOpenView(createRepoView(snapshot.root_path));
      activeViewId = repoViewId;
      collapsedFolderIds = preservedCollapsedFolderIds ??
        getRestoredCollapsedFolderIds(snapshot.tree, restoredState);
      rememberRepoPath(snapshot.root_path);
      status = "ready";
      const preservedEntry = preservedRelativePath
        ? snapshot.docs.find((doc) => doc.relative_path === preservedRelativePath)
        : null;
      const restoredEntry = restoredState?.currentRelativePath
        ? snapshot.docs.find((doc) => doc.relative_path === restoredState.currentRelativePath)
        : null;
      const entry = preservedEntry ?? restoredEntry ??
        snapshot.docs.find((doc) => doc.relative_path === "README.md") ??
        snapshot.docs.find((doc) => doc.relative_path === "baseline/README.md") ??
        snapshot.docs[0];
      if (entry) {
        await openDocument(entry.path, {
          restoreScrollTop: preservedEntry
            ? preservedScrollTop
            : restoredEntry
              ? restoredState?.scrollTop
              : undefined
        });
      }
    } catch (err) {
      if (previousSnapshot) {
        repoPath = previousRepoPath;
        snapshot = previousSnapshot;
        repoCurrent = previousRepoCurrent;
        current = previousCurrent;
        renderedHtml = previousRenderedHtml;
        activeRendererId = previousActiveRendererId;
        htmlFrameSrcdoc = previousHtmlFrameSrcdoc;
        htmlDocHeadings = previousHtmlDocHeadings;
        collapsedFolderIds = previousCollapsedFolderIds;
        activeViewId = previousActiveViewId;
      } else {
        snapshot = null;
        repoCurrent = null;
        current = null;
        clearRenderedDocument();
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

    if (!isRestoringNavigation) {
      updateCurrentHistoryScroll();
    }
    upsertOpenView(createRepoView(repoPath));
    activeViewId = repoViewId;
    status = "opening";
    error = "";
    try {
      repoCurrent = await invoke<Document>("read_document", { repoPath, path });
      current = repoCurrent;
      renderDocumentContent(repoCurrent);
      status = "ready";
      await completeRenderedDocumentUpdate();
      if (options.restoreScrollTop !== undefined) {
        const reader = document.querySelector<HTMLElement>(".reader");
        if (reader) {
          reader.scrollTop = options.restoreScrollTop;
        }
      }
      saveCurrentRepoViewState(options.restoreScrollTop ?? getReaderScrollTop());
      recordNavigationEntry();
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  async function openStandaloneDocument(path: string, anchor = "", options: OpenDocumentOptions = {}) {
    if (!isRestoringNavigation) {
      updateCurrentHistoryScroll();
    }
    const existing = findFileView(path);
    if (existing && fileDocuments.has(existing.id)) {
      await activateView(existing.id);
      if (anchor) {
        await scrollToReaderAnchor(anchor);
      } else if (options.restoreScrollTop !== undefined) {
        restoreReaderScrollTop(options.restoreScrollTop);
      }
      return;
    }

    status = "opening";
    error = "";
    try {
      const doc = await invoke<Document>("read_standalone_document", { path });
      const view = createFileView(doc);
      const nextFileDocuments = new Map(fileDocuments);
      nextFileDocuments.set(view.id, doc);
      fileDocuments = nextFileDocuments;
      upsertOpenView(view);
      activeViewId = view.id;
      current = doc;
      renderDocumentContent(doc);
      status = "ready";
      await completeRenderedDocumentUpdate();
      if (anchor) {
        await scrollToReaderAnchor(anchor);
      } else if (options.restoreScrollTop !== undefined) {
        restoreReaderScrollTop(options.restoreScrollTop);
      }
      recordNavigationEntry();
    } catch (err) {
      error = String(err);
      status = "error";
    }
  }

  function restoreReaderScrollTop(scrollTop: number) {
    const reader = document.querySelector<HTMLElement>(".reader");
    if (reader) {
      reader.scrollTop = scrollTop;
    }
  }

  function createNavigationEntry(scrollTop = getReaderScrollTop()): NavigationEntry | null {
    if (!current) {
      return null;
    }

    const view = getOpenView(activeViewId);
    const isFileView = view?.type === "file";
    return {
      type: isFileView ? "file" : "repo",
      path: current.path,
      title: current.title,
      repoPath: isFileView ? "" : repoPath,
      scrollTop
    };
  }

  function navigationEntryKey(entry: NavigationEntry) {
    return [
      entry.type,
      normalizePathname(entry.repoPath),
      normalizePathname(entry.path)
    ].join(":");
  }

  function updateCurrentHistoryScroll() {
    if (navigationIndex < 0 || navigationIndex >= navigationHistory.length) {
      return;
    }

    const entry = createNavigationEntry();
    if (!entry || navigationEntryKey(entry) !== navigationEntryKey(navigationHistory[navigationIndex])) {
      return;
    }

    navigationHistory = navigationHistory.map((item, index) =>
      index === navigationIndex ? { ...item, scrollTop: entry.scrollTop } : item
    );
  }

  function recordNavigationEntry() {
    if (isRestoringNavigation) {
      return;
    }

    const entry = createNavigationEntry();
    if (!entry) {
      return;
    }

    const entryKey = navigationEntryKey(entry);
    const currentEntry = navigationHistory[navigationIndex];
    if (currentEntry && navigationEntryKey(currentEntry) === entryKey) {
      navigationHistory = navigationHistory.map((item, index) =>
        index === navigationIndex ? entry : item
      );
      return;
    }

    const nextHistory = navigationHistory.slice(0, navigationIndex + 1);
    navigationHistory = [...nextHistory, entry];
    navigationIndex = navigationHistory.length - 1;
  }

  async function navigateHistory(delta: -1 | 1) {
    if ((delta < 0 && !canNavigateBack) || (delta > 0 && !canNavigateForward)) {
      return;
    }

    updateCurrentHistoryScroll();
    const targetIndex = navigationIndex + delta;
    const target = navigationHistory[targetIndex];
    if (!target) {
      return;
    }

    navigationIndex = targetIndex;
    isRestoringNavigation = true;
    try {
      await restoreNavigationEntry(target);
    } finally {
      isRestoringNavigation = false;
    }
  }

  async function restoreNavigationEntry(entry: NavigationEntry) {
    if (entry.type === "file") {
      await openStandaloneDocument(entry.path, "", { restoreScrollTop: entry.scrollTop });
      return;
    }

    if (entry.repoPath && normalizePathname(repoPath) !== normalizePathname(entry.repoPath)) {
      await loadRepo(entry.repoPath);
    }
    await openDocument(entry.path, { restoreScrollTop: entry.scrollTop });
  }

  function clearRenderedDocument() {
    renderedHtml = "";
    activeRendererId = "";
    htmlFrameSrcdoc = "";
    htmlDocHeadings = [];
    teardownHtmlFrameBridge();
  }

  function renderDocumentContent(document: Document) {
    bumpLocalAssetCacheVersion();
    getDocumentRenderer(document).render(document);
  }

  function bumpLocalAssetCacheVersion() {
    localAssetCacheVersion = Math.max(localAssetCacheVersion + 1, Date.now());
  }

  function renderMarkdownDocument(document: Document) {
    teardownHtmlFrameBridge();
    activeRendererId = "markdown";
    htmlFrameSrcdoc = "";
    htmlDocHeadings = [];
    renderedHtml = markdown.render(document.content, createRenderEnv(document));
  }

  async function completeMarkdownRenderedDocumentUpdate() {
    enhanceRenderedTables();
    await renderMermaid();
  }

  function getMarkdownDocumentHeadings(document: Document): DocHeading[] {
    const env = createRenderEnv();
    const tokens = markdown.parse(document.content, {});
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

  function renderHtmlDocument(document: Document) {
    teardownHtmlFrameBridge({ keepFrame: true });
    activeRendererId = "html";
    renderedHtml = "";
    htmlDocHeadings = [];
    prepareHtmlFrameReady();
    htmlFrameSrcdoc = buildHtmlFrameSrcdoc(document);
  }

  async function completeHtmlRenderedDocumentUpdate() {
    await waitForHtmlFrameReady();
    updateHtmlFrameHeight();
  }

  function getHtmlDocumentHeadings() {
    return htmlDocHeadings;
  }

  function buildHtmlFrameSrcdoc(document: Document) {
    const baseHref = getDocumentBaseHref(document.path);
    const bridge = `<script>window.__memViewHtmlReady = true;<\/script>`;
    const embeddedCss = [
      "html{background:#fff;}",
      "body{min-width:0;}",
      "img,video{max-width:100%;height:auto;}",
      "body.mem-view-embedded nav,body.mem-view-embedded aside,body.mem-view-embedded [role=\"navigation\"],body.mem-view-embedded .sidebar,body.mem-view-embedded .side-nav,body.mem-view-embedded .toc,body.mem-view-embedded .table-of-contents{display:none!important;}",
      "body.mem-view-embedded{margin-left:0!important;}",
      "mark.find-highlight{border-radius:3px;background:#ffe08a;color:inherit;padding:0 1px;}",
      "mark.find-highlight.active{background:#f59f00;color:#101820;}",
      ".mem-view-html-diagram-frame{position:relative;}",
      ".mem-view-html-diagram-frame>.mem-view-html-diagram-actions{position:absolute;top:8px;right:8px;z-index:2147483647;display:flex;gap:6px;opacity:.78;transition:opacity 120ms ease;}",
      ".mem-view-html-diagram-frame:hover>.mem-view-html-diagram-actions,.mem-view-html-diagram-actions:focus-within{opacity:1;}",
      ".mem-view-html-diagram-copy,.mem-view-html-diagram-zoom{--mem-view-html-diagram-icon:none;display:grid;width:30px;height:30px;place-items:center;padding:0;border:1px solid rgba(160,160,152,.7);border-radius:7px;appearance:none;background:rgba(255,255,255,.92);color:#24313d;font:16px/1 system-ui,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;cursor:pointer;}",
      ".mem-view-html-diagram-copy{--mem-view-html-diagram-icon:url(\"data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='none'%20stroke='black'%20stroke-linecap='round'%20stroke-linejoin='round'%20stroke-width='2'%20d='M8%208h11v11H8zM5%2015H4a1%201%200%200%201-1-1V4a1%201%200%200%201%201-1h10a1%201%200%200%201%201%201v1'/%3E%3C/svg%3E\");}",
      ".mem-view-html-diagram-zoom{--mem-view-html-diagram-icon:url(\"data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='none'%20stroke='black'%20stroke-linecap='round'%20stroke-linejoin='round'%20stroke-width='2'%20d='M15%203h6v6M9%2021H3v-6M21%203l-7%207M3%2021l7-7'/%3E%3C/svg%3E\");}",
      ".mem-view-html-diagram-copy:hover,.mem-view-html-diagram-zoom:hover{border-color:#9daaa5;background:#fff;}",
      ".mem-view-html-diagram-copy::before,.mem-view-html-diagram-zoom::before{content:\"\";width:16px;height:16px;background:currentColor;-webkit-mask-image:var(--mem-view-html-diagram-icon);-webkit-mask-position:center;-webkit-mask-repeat:no-repeat;-webkit-mask-size:contain;mask-image:var(--mem-view-html-diagram-icon);mask-position:center;mask-repeat:no-repeat;mask-size:contain;}",
      ".mem-view-html-diagram-copy.copied{border-color:#8fc2aa;background:#eef8f2;color:#0f6b47;}",
      ".mem-view-html-diagram-copy.copied{--mem-view-html-diagram-icon:url(\"data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='none'%20stroke='black'%20stroke-linecap='round'%20stroke-linejoin='round'%20stroke-width='2.4'%20d='m5%2012%205%205L20%207'/%3E%3C/svg%3E\");}",
      ".mem-view-html-diagram-copy.error{border-color:#d5a3a3;background:#fff1f1;color:#9b1c1c;}",
      ".mem-view-html-diagram-copy.error{--mem-view-html-diagram-icon:url(\"data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='none'%20stroke='black'%20stroke-linecap='round'%20stroke-linejoin='round'%20stroke-width='2'%20d='M12%208v5M12%2017h.01M21%2012a9%209%200%201%201-18%200%209%209%200%200%201%2018%200'/%3E%3C/svg%3E\");}"
    ].join("");
    const additions = `${baseHref ? `<base href="${escapeHtml(baseHref)}">` : ""}<style data-mem-view>${embeddedCss}</style>${bridge}`;
    const content = document.content;

    if (/<head[\s>]/i.test(content)) {
      return content.replace(/<head([^>]*)>/i, `<head$1>${additions}`);
    }
    if (/<html[\s>]/i.test(content)) {
      return content.replace(/<html([^>]*)>/i, `<html$1><head>${additions}</head>`);
    }
    return `<!doctype html><html><head>${additions}</head><body>${content}</body></html>`;
  }

  function getDocumentBaseHref(path: string) {
    const normalized = normalizePathname(path);
    const slashIndex = normalized.lastIndexOf("/");
    if (slashIndex === -1) {
      return "";
    }

    const directoryPath = normalized.slice(0, slashIndex + 1);
    if (isTauri()) {
      try {
        const converted = convertFileSrc(directoryPath);
        return converted.endsWith("/") ? converted : `${converted}/`;
      } catch {
        return "";
      }
    }

    return `file://${directoryPath}`;
  }

  function prepareHtmlFrameReady() {
    htmlFrameReadyPromise = new Promise((resolve) => {
      resolveHtmlFrameReady = resolve;
    });
  }

  async function waitForHtmlFrameReady() {
    if (!htmlFrameReadyPromise) {
      return;
    }
    await Promise.race([
      htmlFrameReadyPromise,
      new Promise<void>((resolve) => window.setTimeout(resolve, 1200))
    ]);
  }

  async function handleHtmlFrameLoad() {
    try {
      await setupHtmlFrameBridge();
    } finally {
      resolveHtmlFrameReady?.();
      resolveHtmlFrameReady = null;
    }
  }

  async function setupHtmlFrameBridge() {
    await tick();
    teardownHtmlFrameBridge({ keepFrame: true });
    const frameDocument = getHtmlFrameDocument();
    if (!frameDocument) {
      htmlDocHeadings = [];
      return;
    }

    prepareHtmlDocument(frameDocument);
    frameDocument.body?.classList.add("mem-view-embedded");
    enhanceHtmlMermaidDiagrams(frameDocument);
    htmlDocHeadings = collectHtmlHeadings(frameDocument);
    htmlFrameClickHandler = (event) => {
      void handleHtmlFrameClick(event);
    };
    frameDocument.addEventListener("click", htmlFrameClickHandler, true);

    htmlFrameMutationObserver = new MutationObserver(() => {
      enhanceHtmlMermaidDiagrams(frameDocument);
      updateHtmlFrameHeight();
    });
    if (frameDocument.body) {
      htmlFrameMutationObserver.observe(frameDocument.body, {
        childList: true,
        subtree: true
      });
    }

    htmlFrameResizeObserver = new ResizeObserver(() => updateHtmlFrameHeight());
    if (frameDocument.body) {
      htmlFrameResizeObserver.observe(frameDocument.body);
    }
    if (frameDocument.documentElement) {
      htmlFrameResizeObserver.observe(frameDocument.documentElement);
    }
    updateHtmlFrameHeight();
  }

  function teardownHtmlFrameBridge(options: { keepFrame?: boolean } = {}) {
    const frameDocument = getHtmlFrameDocument();
    if (frameDocument && htmlFrameClickHandler) {
      frameDocument.removeEventListener("click", htmlFrameClickHandler, true);
    }
    htmlFrameClickHandler = null;
    htmlFrameResizeObserver?.disconnect();
    htmlFrameResizeObserver = null;
    htmlFrameMutationObserver?.disconnect();
    htmlFrameMutationObserver = null;
    if (!options.keepFrame) {
      htmlFrameElement = null;
    }
  }

  function getHtmlFrameDocument() {
    try {
      return htmlFrameElement?.contentDocument ?? null;
    } catch {
      return null;
    }
  }

  function updateHtmlFrameHeight() {
    const frameDocument = getHtmlFrameDocument();
    if (!htmlFrameElement || !frameDocument) {
      return;
    }
    const body = frameDocument.body;
    const documentElement = frameDocument.documentElement;
    const height = Math.max(
      body?.scrollHeight ?? 0,
      body?.offsetHeight ?? 0,
      documentElement?.scrollHeight ?? 0,
      documentElement?.offsetHeight ?? 0,
      readerElement?.clientHeight ?? 0
    );
    htmlFrameElement.style.height = `${Math.max(240, height)}px`;
  }

  function enhanceHtmlMermaidDiagrams(frameDocument: globalThis.Document) {
    const diagramElements = new Set<Element>();
    frameDocument
      .querySelectorAll<SVGSVGElement>(".mermaid svg, svg[id^='mermaid-'], svg[aria-roledescription]")
      .forEach((svg) => {
        diagramElements.add(svg.closest(".mermaid") ?? svg);
      });

    diagramElements.forEach((diagramElement) => {
      const frame = getOrCreateHtmlDiagramFrame(diagramElement);
      if (!frame) {
        return;
      }

      ensureHtmlDiagramActions(frame);
    });
  }

  function getOrCreateHtmlDiagramFrame(diagramElement: Element) {
    const existingFrame = diagramElement.closest<HTMLElement>(".mem-view-html-diagram-frame");
    if (existingFrame) {
      return existingFrame;
    }

    const existingDiagramWrapper = diagramElement.closest<HTMLElement>(".diagram-wrap");
    if (existingDiagramWrapper) {
      existingDiagramWrapper.classList.add("mem-view-html-diagram-frame");
      return existingDiagramWrapper;
    }

    const parent = diagramElement.parentElement;
    if (!parent) {
      return null;
    }

    const frame = diagramElement.ownerDocument.createElement("figure");
    frame.className = "mem-view-html-diagram-frame";
    frame.style.margin = "0";
    parent.insertBefore(frame, diagramElement);
    frame.appendChild(diagramElement);
    return frame;
  }

  function ensureHtmlDiagramActions(frame: HTMLElement) {
    const hasActions = Array.from(frame.children).some((child) =>
      child.classList.contains("mem-view-html-diagram-actions")
    );
    if (hasActions) {
      return;
    }

    const actions = frame.ownerDocument.createElement("div");
    actions.className = "mem-view-html-diagram-actions";
    actions.appendChild(createHtmlDiagramButton("mem-view-html-diagram-copy", t.copyDiagram));
    actions.appendChild(createHtmlDiagramButton("mem-view-html-diagram-zoom", t.enlargeDiagram));
    frame.appendChild(actions);
  }

  function createHtmlDiagramButton(className: string, label: string) {
    const frameDocument = getHtmlFrameDocument();
    const button = (frameDocument ?? document).createElement("button");
    button.type = "button";
    button.className = className;
    button.setAttribute("aria-label", label);
    button.title = label;
    return button;
  }

  function prepareHtmlDocument(frameDocument: globalThis.Document) {
    const elements = Array.from(
      frameDocument.body?.querySelectorAll<HTMLElement>(
        "h1,h2,h3,h4,h5,h6,p,li,blockquote,pre,td,th,figure,img,section,article"
      ) ?? []
    );
    elements.forEach((element, index) => {
      if (!isVisibleHtmlElement(element) || !isMeaningfulHtmlNode(element)) {
        return;
      }
      if (!element.dataset.memNodeId) {
        element.dataset.memNodeId = `html-${element.tagName.toLowerCase()}-${index + 1}`;
      }
      element.dataset.nodeType = `html_${element.tagName.toLowerCase()}`;
    });
  }

  function collectHtmlHeadings(frameDocument: globalThis.Document): DocHeading[] {
    const env = createRenderEnv();
    return Array.from(frameDocument.querySelectorAll<HTMLElement>("h1,h2,h3,h4,h5,h6"))
      .map((heading) => {
        const title = normalizeExcerpt(heading.innerText || heading.textContent || "");
        const level = Number(heading.tagName.slice(1));
        if (!title || !Number.isFinite(level)) {
          return null;
        }
        if (!heading.id) {
          heading.id = getUniqueHeadingId(title, env);
        }
        return { id: heading.id, title, level };
      })
      .filter((heading): heading is DocHeading => Boolean(heading));
  }

  function isVisibleHtmlElement(element: HTMLElement) {
    const rect = element.getBoundingClientRect();
    const style = element.ownerDocument.defaultView?.getComputedStyle(element);
    return rect.width > 0 &&
      rect.height > 0 &&
      style?.display !== "none" &&
      style?.visibility !== "hidden";
  }

  function isMeaningfulHtmlNode(element: HTMLElement) {
    if (element.tagName.toLowerCase() === "img") {
      return true;
    }
    return Boolean(normalizeExcerpt(element.innerText || element.textContent || ""));
  }

  function scrollHtmlElementIntoReader(element: HTMLElement, block: "start" | "center" = "start") {
    if (!readerElement || !htmlFrameElement) {
      return;
    }
    const readerBounds = readerElement.getBoundingClientRect();
    const frameBounds = htmlFrameElement.getBoundingClientRect();
    const elementBounds = element.getBoundingClientRect();
    const elementTop = frameBounds.top + elementBounds.top - readerBounds.top + readerElement.scrollTop;
    const offset = block === "center"
      ? Math.max(0, (readerElement.clientHeight - elementBounds.height) / 2)
      : 0;
    readerElement.scrollTo({
      top: Math.max(0, elementTop - offset),
      behavior: "smooth"
    });
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
    const nodes = Array.from(document.querySelectorAll<HTMLElement>(".reader .mermaid"));
    if (!nodes.length) {
      return;
    }

    for (const node of nodes) {
      if (node.dataset.mermaidRendered === "true" && node.querySelector("svg")) {
        continue;
      }

      const definition = getMermaidSource(node);
      if (!definition.trim()) {
        continue;
      }

      try {
        node.textContent = definition;
        await mermaid.parse(definition);
        await mermaid.run({ nodes: [node] });
        if (!node.querySelector("svg")) {
          throw new Error("Mermaid rendered without producing an SVG");
        }
        node.dataset.mermaidRendered = "true";
      } catch (err) {
        console.warn("Mermaid render failed", err);
        renderMermaidErrorPlaceholder(node, definition, err);
      }
    }
  }

  function getMermaidSource(node: HTMLElement) {
    return node
      .closest<HTMLElement>(".diagram-frame")
      ?.querySelector<HTMLElement>(".mermaid-source")
      ?.textContent ?? node.textContent ?? "";
  }

  function renderMermaidErrorPlaceholder(node: HTMLElement, definition: string, err: unknown) {
    const frame = node.closest<HTMLElement>(".diagram-frame");
    if (!frame) {
      return;
    }

    const message = normalizeMermaidErrorMessage(err);
    frame.classList.add("mermaid-error-frame");
    frame.innerHTML = "";

    const placeholder = frame.ownerDocument.createElement("div");
    placeholder.className = "mermaid-error-placeholder";

    const art = frame.ownerDocument.createElement("div");
    art.className = "mermaid-error-art";
    art.setAttribute("aria-hidden", "true");
    art.innerHTML = "<span></span><span></span><span></span><strong>!</strong>";

    const content = frame.ownerDocument.createElement("div");
    content.className = "mermaid-error-content";

    const heading = frame.ownerDocument.createElement("div");
    heading.className = "mermaid-error-heading";

    const title = frame.ownerDocument.createElement("strong");
    title.textContent = t.mermaidRenderFailed;
    heading.appendChild(title);

    const lineRange = formatMermaidLineRange(frame);
    if (lineRange) {
      const line = frame.ownerDocument.createElement("span");
      line.textContent = lineRange;
      heading.appendChild(line);
    }

    const body = frame.ownerDocument.createElement("p");
    body.textContent = t.mermaidErrorBody;

    const detailsLabel = frame.ownerDocument.createElement("span");
    detailsLabel.className = "mermaid-error-details-label";
    detailsLabel.textContent = t.mermaidErrorDetails;

    const details = frame.ownerDocument.createElement("div");
    details.className = "mermaid-error-message";
    details.textContent = message;

    const source = frame.ownerDocument.createElement("code");
    source.className = "mermaid-error-source";
    source.hidden = true;
    source.textContent = definition;

    const actions = frame.ownerDocument.createElement("div");
    actions.className = "mermaid-error-actions";

    const promptButton = frame.ownerDocument.createElement("button");
    promptButton.className = "mermaid-error-copy-prompt";
    promptButton.type = "button";
    promptButton.textContent = t.copyMermaidFixPrompt;
    promptButton.setAttribute("aria-label", t.copyMermaidFixPrompt);
    promptButton.title = t.copyMermaidFixPrompt;
    actions.appendChild(promptButton);

    content.appendChild(heading);
    content.appendChild(body);
    content.appendChild(detailsLabel);
    content.appendChild(details);
    content.appendChild(actions);
    content.appendChild(source);
    placeholder.appendChild(art);
    placeholder.appendChild(content);
    frame.appendChild(placeholder);
  }

  function normalizeMermaidErrorMessage(err: unknown) {
    const message = limitExcerpt(normalizeExcerpt(getErrorMessage(err)));
    return message || (locale === "zh-CN" ? "未知 Mermaid 错误" : "Unknown Mermaid error");
  }

  function formatMermaidLineRange(frame: HTMLElement) {
    const start = Number(frame.dataset.sourceLineStart);
    const end = Number(frame.dataset.sourceLineEnd);
    if (!Number.isFinite(start) || start <= 0) {
      return "";
    }

    const range = Number.isFinite(end) && end > start ? `${start}-${end}` : `${start}`;
    return locale === "zh-CN" ? `第 ${range} 行` : `Line ${range}`;
  }

  function decodeHtml(value: string) {
    const textarea = document.createElement("textarea");
    textarea.innerHTML = value;
    return textarea.value;
  }

  function createRenderEnv(documentContext: Document | null = null): MarkdownRenderEnv {
    return {
      headingCounts: new Map<string, number>(),
      nodeCounts: new Map<string, number>(),
      documentPath: documentContext?.path ?? ""
    };
  }

  function annotateSourceToken(token: any, nodeType: string, env: MarkdownRenderEnv) {
    const attrs = sourceTokenData(token, nodeType, env);
    for (const [name, value] of Object.entries(attrs)) {
      token.attrSet(name, value);
    }
  }

  function sourceTokenAttrs(token: any, nodeType: string, env: MarkdownRenderEnv) {
    const attrs = sourceTokenData(token, nodeType, env);
    return Object.entries(attrs)
      .map(([name, value]) => ` ${name}="${escapeHtml(value)}"`)
      .join("");
  }

  function sourceTokenData(token: any, nodeType: string, env: MarkdownRenderEnv) {
    const data: Record<string, string> = {
      "data-mem-node-id": getUniqueNodeId(token, nodeType, env),
      "data-node-type": nodeType
    };
    if (Array.isArray(token.map) && token.map.length >= 2) {
      data["data-source-line-start"] = String(token.map[0] + 1);
      data["data-source-line-end"] = String(token.map[1]);
    }
    return data;
  }

  function getUniqueNodeId(token: any, nodeType: string, env: MarkdownRenderEnv) {
    const map = Array.isArray(token.map) ? token.map : null;
    const base = `${nodeType}-${map ? `${map[0] + 1}-${map[1]}` : "unknown"}`;
    const count = env.nodeCounts.get(base) ?? 0;
    env.nodeCounts.set(base, count + 1);
    return count === 0 ? base : `${base}-${count + 1}`;
  }

  function renderCodeToken(token: any, info: string, env: MarkdownRenderEnv) {
    const lang = info.split(/\s+/)[0]?.trim();
    const className = lang ? ` class="language-${escapeHtml(lang)}"` : "";
    const attrs = sourceTokenAttrs(token, "code_block", env);
    return `<pre${attrs}><code${className}>${escapeHtml(token.content)}</code></pre>\n`;
  }

  function escapeHtml(value: string) {
    return value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function renderTextWithMarkdownBreakTags(value: string) {
    const html: string[] = [];
    const breakTagPattern = /<br\s*\/?>/gi;
    let lastIndex = 0;

    value.replace(breakTagPattern, (match, offset: number) => {
      html.push(escapeHtml(value.slice(lastIndex, offset)));
      html.push("<br>");
      lastIndex = offset + match.length;
      return match;
    });

    html.push(escapeHtml(value.slice(lastIndex)));
    return html.join("");
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

  function getRestoredCollapsedFolderIds(nodes: TreeNode[], state: RepoViewState | null) {
    if (!state) {
      return getDefaultCollapsedFolderIds(nodes);
    }

    const folderIds = getFolderIds(nodes);
    return new Set(state.collapsedFolderIds.filter((id) => folderIds.has(id)));
  }

  function getFolderIds(nodes: TreeNode[]) {
    const ids = new Set<string>();
    const visit = (node: TreeNode) => {
      if (!node.path && node.children.length) {
        ids.add(node.id);
      }
      node.children.forEach(visit);
    };

    nodes.forEach(visit);
    return ids;
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

  function nodeClass(node: FlatNode, activePath: string) {
    return [
      "nav-row",
      node.path ? "doc" : "folder",
      isSamePath(activePath, node.path) ? "active" : "",
      !node.path && collapsedFolderIds.has(node.id) ? "collapsed" : "",
      node.kind
    ].join(" ");
  }

  function isSamePath(pathA: string | null | undefined, pathB: string | null | undefined) {
    return Boolean(pathA && pathB && normalizePathname(pathA) === normalizePathname(pathB));
  }

  function toggleFolder(node: FlatNode) {
    const next = new Set(collapsedFolderIds);
    if (next.has(node.id)) {
      next.delete(node.id);
    } else {
      next.add(node.id);
    }
    collapsedFolderIds = next;
    saveCurrentRepoViewState();
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

    const mermaidPromptButton = target.closest<HTMLButtonElement>(".mermaid-error-copy-prompt");
    if (mermaidPromptButton && mermaidPromptButton.closest(".reader")) {
      await copyMermaidFixPrompt(mermaidPromptButton);
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

  function handleReaderWheel(event: WheelEvent) {
    if (!readerElement || event.ctrlKey || event.metaKey) {
      return;
    }

    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const tableScroller = target.closest<HTMLElement>(".table-scroll");
    if (!tableScroller || !readerElement.contains(tableScroller)) {
      return;
    }

    const deltaY = wheelDeltaToPixels(event.deltaY, event.deltaMode, readerElement.clientHeight);
    const deltaX = wheelDeltaToPixels(event.deltaX, event.deltaMode, readerElement.clientWidth);
    if (Math.abs(deltaY) <= Math.abs(deltaX)) {
      return;
    }

    const maxReaderTop = Math.max(0, readerElement.scrollHeight - readerElement.clientHeight);
    const nextReaderTop = clampNumber(readerElement.scrollTop + deltaY, 0, maxReaderTop);
    if (nextReaderTop === readerElement.scrollTop) {
      return;
    }

    if (deltaX !== 0) {
      const maxTableLeft = Math.max(0, tableScroller.scrollWidth - tableScroller.clientWidth);
      tableScroller.scrollLeft = clampNumber(tableScroller.scrollLeft + deltaX, 0, maxTableLeft);
    }

    readerElement.scrollTop = nextReaderTop;
    event.preventDefault();
  }

  function wheelDeltaToPixels(delta: number, deltaMode: number, pageSize: number) {
    if (deltaMode === WheelEvent.DOM_DELTA_LINE) {
      return delta * 16;
    }

    if (deltaMode === WheelEvent.DOM_DELTA_PAGE) {
      return delta * pageSize;
    }

    return delta;
  }

  function getFrameDiagramSvg(button: HTMLButtonElement) {
    const frame = button.closest<HTMLElement>(".diagram-frame");
    return frame?.querySelector<SVGSVGElement>(".mermaid svg") ?? null;
  }

  async function copyMermaidFixPrompt(button: HTMLButtonElement) {
    if (button.disabled) {
      return;
    }

    const frame = button.closest<HTMLElement>(".diagram-frame");
    const definition = frame?.querySelector<HTMLElement>(".mermaid-error-source")?.textContent ?? "";
    const errorMessage = frame?.querySelector<HTMLElement>(".mermaid-error-message")?.textContent ?? "";
    const prompt = buildMermaidFixPrompt(frame, definition, errorMessage);

    button.disabled = true;
    try {
      await copyTextToClipboard(prompt);
      setMermaidFixPromptButtonState(button, "copied");
      showUpdateToast(t.copiedMermaidFixPrompt);
    } catch (err) {
      console.warn("Copy Mermaid fix prompt failed", err);
      const message = getErrorMessage(err);
      setMermaidFixPromptButtonState(button, "error", message);
      showUpdateToast(`${t.copyMermaidFixPromptFailed}: ${message}`, "error");
    } finally {
      button.disabled = false;
    }
  }

  function buildMermaidFixPrompt(
    frame: HTMLElement | null | undefined,
    definition: string,
    errorMessage: string
  ) {
    const documentPath = current?.path || "";
    const relativePath = current?.relative_path || "";
    const lineRange = frame ? formatMermaidLineRange(frame) : "";
    const source = definition.trimEnd();
    const errorText = errorMessage.trim() || (locale === "zh-CN" ? "未捕获到具体错误信息" : "No detailed error was captured");

    if (locale === "zh-CN") {
      return [
        "请修复下面 Markdown 文档中的 Mermaid 格式错误。",
        "",
        "如果你可以访问本地文件，请直接打开并修改该文件；如果不能访问，请返回可替换的 ```mermaid 代码块。",
        "只修复相关 Mermaid 代码块，保持图表达的业务含义、节点文案和文档其它内容不变。",
        "",
        `文档路径：${documentPath || "未知"}`,
        relativePath ? `相对路径：${relativePath}` : "",
        lineRange ? `错误位置：${lineRange} 附近` : "",
        "",
        "Mermaid 报错信息：",
        errorText,
        "",
        "当前 Mermaid 源码：",
        "```mermaid",
        source,
        "```",
        "",
        "请先说明错误原因，再给出修复后的 Mermaid 代码块；如果可以直接改文件，请完成修改并说明验证方式。"
      ].filter(Boolean).join("\n");
    }

    return [
      "Fix the Mermaid syntax error in this Markdown document.",
      "",
      "If you can access the local file, open and edit it directly. If not, return a replacement ```mermaid code block.",
      "Only fix the related Mermaid block. Preserve the diagram's meaning, node labels, and all other document content.",
      "",
      `Document path: ${documentPath || "Unknown"}`,
      relativePath ? `Relative path: ${relativePath}` : "",
      lineRange ? `Error location: near ${lineRange}` : "",
      "",
      "Mermaid error:",
      errorText,
      "",
      "Current Mermaid source:",
      "```mermaid",
      source,
      "```",
      "",
      "Briefly explain the cause, then provide the fixed Mermaid block. If you can edit the file directly, make the change and describe how you verified it."
    ].filter(Boolean).join("\n");
  }

  function setMermaidFixPromptButtonState(
    button: HTMLButtonElement,
    state: Extract<CopyDiagramState, "copied" | "error">,
    message = ""
  ) {
    button.classList.remove("copied", "error");
    button.classList.add(state);
    const label = state === "copied"
      ? t.copiedMermaidFixPrompt
      : `${t.copyMermaidFixPromptFailed}${message ? `: ${message}` : ""}`;
    button.textContent = label;
    button.setAttribute("aria-label", label);
    button.title = label;

    window.setTimeout(() => {
      if (!button.isConnected) {
        return;
      }
      button.classList.remove("copied", "error");
      button.textContent = t.copyMermaidFixPrompt;
      button.setAttribute("aria-label", t.copyMermaidFixPrompt);
      button.title = t.copyMermaidFixPrompt;
    }, 1800);
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
      await openStandaloneDocument(target.path, target.anchor);
      return true;
    }

    await openDocument(target.path);
    if (target.anchor) {
      await scrollToReaderAnchor(target.anchor);
    }
    return true;
  }

  async function handleHtmlFrameClick(event: MouseEvent) {
    const target = event.target as Element | null;
    if (!target || typeof target.closest !== "function") {
      return;
    }

    const copyButton = target.closest<HTMLButtonElement>(".mem-view-html-diagram-copy");
    if (copyButton) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      const svg = getHtmlFrameDiagramSvg(copyButton);
      if (svg) {
        await copyInlineDiagram(svg, copyButton);
      }
      return;
    }

    const zoomButton = target.closest<HTMLButtonElement>(".mem-view-html-diagram-zoom");
    if (zoomButton) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      const svg = getHtmlFrameDiagramSvg(zoomButton);
      if (svg) {
        zoomedDiagramHtml = serializeDiagramSvg(svg);
        zoomedDiagramTitle = current?.title ?? t.mermaidDiagram;
        setCopyDiagramState("idle");
        resetDiagramView();
        await tick();
        fitDiagramToViewport();
      }
      return;
    }

    await handleHtmlFrameLinkClick(event);
  }

  function getHtmlFrameDiagramSvg(button: HTMLButtonElement) {
    const frame = button.closest<HTMLElement>(".mem-view-html-diagram-frame");
    return frame?.querySelector<SVGSVGElement>(".mermaid svg, svg") ?? null;
  }

  async function handleHtmlFrameLinkClick(event: MouseEvent) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }

    const target = event.target as Element | null;
    const link = target && typeof target.closest === "function"
      ? target.closest("a")
      : null;
    if (!link) {
      return;
    }

    const href = link.getAttribute("href")?.trim() ?? "";
    if (!href) {
      return;
    }

    if (href.startsWith("#")) {
      event.preventDefault();
      await scrollToReaderAnchor(href.slice(1));
      return;
    }

    const resolved = resolveLinkedDocument(href);
    if (!resolved) {
      return;
    }

    event.preventDefault();
    if (resolved.type === "file") {
      await openStandaloneDocument(resolved.path, resolved.anchor);
      return;
    }

    await openDocument(resolved.path);
    if (resolved.anchor) {
      await scrollToReaderAnchor(resolved.anchor);
    }
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

  function resolveMarkdownImageSrc(src: string | null, env: MarkdownRenderEnv) {
    if (!src || !env.documentPath || isExternalImageSrc(src)) {
      return null;
    }

    const resolved = resolveLocalAssetPath(src, env.documentPath);
    if (!resolved || !isTauri()) {
      return null;
    }

    try {
      return appendLocalAssetCacheBuster(convertFileSrc(resolved.path), resolved.suffix);
    } catch {
      return null;
    }
  }

  function appendLocalAssetCacheBuster(assetUrl: string, suffix: string) {
    const hashIndex = suffix.indexOf("#");
    const suffixBeforeHash = hashIndex === -1 ? suffix : suffix.slice(0, hashIndex);
    const hash = hashIndex === -1 ? "" : suffix.slice(hashIndex);
    const hasQuery = assetUrl.includes("?") || suffixBeforeHash.includes("?");
    const separator = hasQuery
      ? suffixBeforeHash.endsWith("?") || suffixBeforeHash.endsWith("&") ? "" : "&"
      : "?";

    return `${assetUrl}${suffixBeforeHash}${separator}mem_view_asset_v=${encodeURIComponent(String(localAssetCacheVersion))}${hash}`;
  }

  function isExternalImageSrc(src: string) {
    const value = src.trim();
    if (!value || value.startsWith("#")) {
      return true;
    }
    if (/^(https?:|data:|blob:|asset:|tauri:)/i.test(value)) {
      return true;
    }
    return /^[a-z][a-z0-9+.-]*:/i.test(value) && !value.toLowerCase().startsWith("file:");
  }

  function resolveLocalAssetPath(src: string, documentPath: string) {
    const { pathPart, suffix } = splitAssetReference(src.trim());
    if (!pathPart) {
      return null;
    }

    let decodedPath = "";
    if (pathPart.toLowerCase().startsWith("file:")) {
      try {
        decodedPath = normalizePathname(safeDecodeURIComponent(new URL(pathPart).pathname));
      } catch {
        return null;
      }
    } else {
      decodedPath = normalizePathname(safeDecodeURIComponent(pathPart));
    }

    const normalizedPath = decodedPath.startsWith("/")
      ? normalizeAbsolutePathSegments(decodedPath)
      : normalizeAbsolutePathSegments(
          `${documentPath.includes("/") ? documentPath.slice(0, documentPath.lastIndexOf("/")) : ""}/${decodedPath}`
        );

    return normalizedPath ? { path: normalizedPath, suffix } : null;
  }

  function splitAssetReference(value: string) {
    const queryIndex = value.indexOf("?");
    const hashIndex = value.indexOf("#");
    const suffixIndex = [queryIndex, hashIndex]
      .filter((index) => index >= 0)
      .sort((left, right) => left - right)[0];

    if (suffixIndex === undefined) {
      return { pathPart: value, suffix: "" };
    }

    return {
      pathPart: value.slice(0, suffixIndex),
      suffix: value.slice(suffixIndex)
    };
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
      relativePath.endsWith("/") ? `${relativePath}index.html` : `${relativePath}/index.html`,
      isDocumentPath(relativePath) ? relativePath : `${relativePath}.md`,
      isDocumentPath(relativePath) ? relativePath : `${relativePath}.html`,
      isDocumentPath(relativePath) ? relativePath : `${relativePath}.htm`
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
      const target = current ? getDocumentRenderer(current).findAnchor(decodedAnchor) : null;
      if (!target) {
        return;
      }

      if (current?.content_type === "html") {
        scrollHtmlElementIntoReader(target, "start");
      } else {
        target.scrollIntoView({ behavior: "smooth", block: "start" });
      }
      target.focus({ preventScroll: true });
    });
  }

  function findMarkdownReaderAnchor(anchor: string) {
    const reader = document.querySelector<HTMLElement>(".reader");
    if (!reader) {
      return null;
    }

    const ids = [anchor, slugifyHeading(anchor)];
    return Array.from(reader.querySelectorAll<HTMLElement>("[id], a[name]")).find((element) =>
      ids.includes(element.id) || ids.includes(element.getAttribute("name") ?? "")
    ) ?? null;
  }

  function findHtmlReaderAnchor(anchor: string) {
    const frameDocument = getHtmlFrameDocument();
    if (!frameDocument) {
      return null;
    }

    const ids = [anchor, slugifyHeading(anchor)];
    return Array.from(frameDocument.querySelectorAll<HTMLElement>("[id], a[name]")).find((element) =>
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
    clearFindHighlights();

    const needle = findQuery.trim();
    if (!findOpen || !current || !needle) {
      findMatchCount = 0;
      activeFindIndex = 0;
      return;
    }

    const renderer = getDocumentRenderer(current);
    const matches = renderer.highlightFindMatches(needle);
    findMatchCount = matches.length;
    if (!matches.length) {
      activeFindIndex = 0;
      return;
    }

    activeFindIndex = Math.min(Math.max(activeFindIndex, 0), matches.length - 1);
    renderer.setActiveFindMatch(matches, scroll);
  }

  function highlightMarkdownReaderMatches(needle: string) {
    const reader = document.querySelector<HTMLElement>(".reader");
    return reader ? highlightTextMatches(reader, needle) : [];
  }

  function highlightHtmlReaderMatches(needle: string) {
    const frameDocument = getHtmlFrameDocument();
    return frameDocument?.body ? highlightTextMatches(frameDocument.body, needle) : [];
  }

  function highlightTextMatches(root: HTMLElement, needle: string) {
    const matches: HTMLElement[] = [];
    const textNodes: Text[] = [];
    const lowerNeedle = needle.toLowerCase();
    const ownerDocument = root.ownerDocument;
    const walker = ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement;
        if (
          !parent ||
          parent.closest(".mermaid, .diagram-actions, .mermaid-error-source, mark.find-highlight, script, style, noscript")
        ) {
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
      const fragment = ownerDocument.createDocumentFragment();
      let cursor = 0;
      let matchIndex = lowerText.indexOf(lowerNeedle);

      while (matchIndex !== -1) {
        if (matchIndex > cursor) {
          fragment.append(document.createTextNode(text.slice(cursor, matchIndex)));
        }

        const mark = ownerDocument.createElement("mark");
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

  function clearFindHighlights() {
    if (!current) {
      clearMarkdownFindHighlights();
      clearHtmlFindHighlights();
      return;
    }
    getDocumentRenderer(current).clearFindHighlights();
  }

  function clearMarkdownFindHighlights() {
    clearFindHighlightsInRoot(document.querySelector<HTMLElement>(".reader"));
  }

  function clearHtmlFindHighlights() {
    clearFindHighlightsInRoot(getHtmlFrameDocument()?.body ?? null);
  }

  function clearFindHighlightsInRoot(root: HTMLElement | null) {
    if (!root) {
      return;
    }

    const highlights = Array.from(root.querySelectorAll<HTMLElement>("mark.find-highlight"));
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

  function setActiveMarkdownFindMatch(matches = getMarkdownFindMatches(), scroll = false) {
    matches.forEach((match, index) => {
      match.classList.toggle("active", index === activeFindIndex);
    });

    if (scroll) {
      matches[activeFindIndex]?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }

  function setActiveHtmlFindMatch(matches = getHtmlFindMatches(), scroll = false) {
    matches.forEach((match, index) => {
      match.classList.toggle("active", index === activeFindIndex);
    });

    if (scroll && matches[activeFindIndex]) {
      scrollHtmlElementIntoReader(matches[activeFindIndex], "center");
    }
  }

  function getMarkdownFindMatches() {
    return Array.from(document.querySelectorAll<HTMLElement>(".reader mark.find-highlight"));
  }

  function getHtmlFindMatches() {
    return Array.from(getHtmlFrameDocument()?.querySelectorAll<HTMLElement>("mark.find-highlight") ?? []);
  }

  function moveFindMatch(delta: number) {
    if (!current) {
      return;
    }
    if (!findQuery.trim()) {
      findInput?.focus();
      return;
    }

    if (!findMatchCount) {
      void refreshFindHighlights({ scroll: true });
      return;
    }

    activeFindIndex = (activeFindIndex + delta + findMatchCount) % findMatchCount;
    getDocumentRenderer(current).setActiveFindMatch(undefined, true);
  }

  function toggleAnnotationMode() {
    if (!current || annotationExporting) {
      return;
    }

    annotationMode = !annotationMode;
    annotationDraft = null;
    annotationPointerId = null;
    if (!annotationMode) {
      editingAnnotationId = "";
    }
  }

  function handleReaderPointerDown(event: PointerEvent) {
    if (!annotationMode || !current || !readerElement || event.button !== 0) {
      return;
    }

    const target = event.target;
    if (
      target instanceof Element &&
      target.closest("a, button, input, textarea, select, .annotation-note, .diagram-actions")
    ) {
      return;
    }

    event.preventDefault();
    const point = getReaderContentPoint(event);
    annotationDraft = {
      startX: point.x,
      startY: point.y,
      rect: createAnnotationRect(point.x, point.y, point.x, point.y)
    };
    annotationPointerId = event.pointerId;
    readerElement.setPointerCapture(event.pointerId);
  }

  function handleReaderPointerMove(event: PointerEvent) {
    if (!annotationDraft || !readerElement || annotationPointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    const point = getReaderContentPoint(event);
    annotationDraft = {
      ...annotationDraft,
      rect: createAnnotationRect(annotationDraft.startX, annotationDraft.startY, point.x, point.y)
    };
  }

  function handleReaderPointerUp(event: PointerEvent) {
    if (!annotationDraft || !readerElement || annotationPointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    const draft = annotationDraft;
    releaseAnnotationPointer(event);
    annotationDraft = null;
    annotationPointerId = null;

    if (draft.rect.width < 8 || draft.rect.height < 8) {
      return;
    }

    addAnnotationFromRect(draft.rect);
  }

  function handleReaderPointerCancel(event: PointerEvent) {
    if (annotationPointerId !== event.pointerId) {
      return;
    }
    releaseAnnotationPointer(event);
    annotationDraft = null;
    annotationPointerId = null;
  }

  function releaseAnnotationPointer(event: PointerEvent) {
    if (!readerElement?.hasPointerCapture(event.pointerId)) {
      return;
    }
    readerElement.releasePointerCapture(event.pointerId);
  }

  function getReaderContentPoint(event: PointerEvent) {
    if (!readerElement) {
      return { x: 0, y: 0 };
    }
    const bounds = readerElement.getBoundingClientRect();
    return {
      x: event.clientX - bounds.left + readerElement.scrollLeft,
      y: event.clientY - bounds.top + readerElement.scrollTop
    };
  }

  function createAnnotationRect(x1: number, y1: number, x2: number, y2: number): AnnotationRect {
    const left = Math.min(x1, x2);
    const top = Math.min(y1, y2);
    return {
      left: roundNumber(left),
      top: roundNumber(top),
      width: roundNumber(Math.abs(x2 - x1)),
      height: roundNumber(Math.abs(y2 - y1)),
      scrollTop: roundNumber(readerElement?.scrollTop ?? 0),
      scrollLeft: roundNumber(readerElement?.scrollLeft ?? 0),
      readerWidth: roundNumber(readerElement?.clientWidth ?? 0),
      readerHeight: roundNumber(readerElement?.clientHeight ?? 0)
    };
  }

  function addAnnotationFromRect(rect: AnnotationRect) {
    if (!current || !currentDocumentKey) {
      return;
    }

    const coveredNodes = getCoveredNodes(rect, current);
    if (!coveredNodes.length) {
      showUpdateToast(t.annotationNoCoveredNodes, "error");
      return;
    }

    const annotation: AnnotationItem = {
      id: createAnnotationId(),
      note: "",
      rect,
      coveredNodes,
      document: getCurrentAnnotationDocumentMeta()
    };
    const next = [...currentAnnotations, annotation];
    setAnnotationsForPath(currentDocumentKey, next);
    editAnnotation(annotation.id);
  }

  function editAnnotation(id: string) {
    if (currentAnnotations.find((annotation) => annotation.id === id)?.noteCollapsed) {
      updateAnnotationNoteCollapsed(id, false);
    }
    editingAnnotationId = id;
    void focusEditingAnnotationNote(id);
  }

  async function focusEditingAnnotationNote(id: string) {
    await tick();
    if (editingAnnotationId !== id || !readerElement) {
      return;
    }

    const textarea = Array.from(
      readerElement.querySelectorAll<HTMLTextAreaElement>(".annotation-note textarea")
    ).find((element) => element.dataset.annotationId === id);
    if (!textarea) {
      return;
    }

    textarea.focus({ preventScroll: true });
    const cursorPosition = textarea.value.length;
    textarea.setSelectionRange(cursorPosition, cursorPosition);
  }

  function setAnnotationsForPath(path: string, items: AnnotationItem[]) {
    const next = new Map(annotationsByPath);
    if (items.length) {
      next.set(path, items);
    } else {
      next.delete(path);
    }
    annotationsByPath = next;
  }

  function updateAnnotationNote(id: string, note: string) {
    if (!currentDocumentKey) {
      return;
    }
    setAnnotationsForPath(
      currentDocumentKey,
      currentAnnotations.map((annotation) =>
        annotation.id === id ? { ...annotation, note } : annotation
      )
    );
  }

  function updateAnnotationNotePosition(id: string, notePosition: AnnotationNotePosition) {
    if (!currentDocumentKey) {
      return;
    }
    setAnnotationsForPath(
      currentDocumentKey,
      currentAnnotations.map((annotation) =>
        annotation.id === id ? { ...annotation, notePosition } : annotation
      )
    );
  }

  function updateAnnotationNoteCollapsed(id: string, noteCollapsed: boolean) {
    if (!currentDocumentKey) {
      return;
    }
    setAnnotationsForPath(
      currentDocumentKey,
      currentAnnotations.map((annotation) =>
        annotation.id === id ? { ...annotation, noteCollapsed } : annotation
      )
    );
    if (noteCollapsed && editingAnnotationId === id) {
      editingAnnotationId = "";
    }
  }

  function toggleAnnotationNoteCollapsed(id: string) {
    const annotation = currentAnnotations.find((item) => item.id === id);
    if (!annotation) {
      return;
    }

    const noteCollapsed = !annotation.noteCollapsed;
    updateAnnotationNoteCollapsed(id, noteCollapsed);
    if (!noteCollapsed) {
      editAnnotation(id);
    }
  }

  function removeAnnotation(id: string) {
    if (!currentDocumentKey) {
      return;
    }
    setAnnotationsForPath(
      currentDocumentKey,
      currentAnnotations.filter((annotation) => annotation.id !== id)
    );
    if (editingAnnotationId === id) {
      editingAnnotationId = "";
    }
  }

  function annotationNotePlacement(annotation: AnnotationItem) {
    if (annotation.notePosition) {
      return {
        className: "custom",
        left: annotation.notePosition.left,
        top: annotation.notePosition.top,
        style: `left: ${annotation.notePosition.left}px; top: ${annotation.notePosition.top}px`
      };
    }

    const gap = 10;
    const noteWidth = annotationNoteWidth(annotation);
    const viewportLeft = readerElement?.scrollLeft ?? 0;
    const viewportRight = viewportLeft + (readerElement?.clientWidth ?? 0);
    const preferredRight = annotation.rect.left + annotation.rect.width + gap;
    const canPlaceRight = preferredRight + noteWidth <= viewportRight - 8;
    const left = canPlaceRight
      ? preferredRight
      : Math.max(viewportLeft + 8, annotation.rect.left - noteWidth - gap);

    return {
      className: canPlaceRight ? "right" : "left",
      left: roundNumber(left),
      top: annotation.rect.top,
      style: `left: ${roundNumber(left)}px; top: ${annotation.rect.top}px`
    };
  }

  function annotationNoteWidth(annotation: AnnotationItem) {
    if (annotation.noteCollapsed) {
      return 52;
    }
    return 300;
  }

  function startAnnotationNoteDrag(annotation: AnnotationItem, event: PointerEvent) {
    event.preventDefault();
    event.stopPropagation();

    const handle = event.currentTarget as HTMLElement;
    const noteElement = handle.closest<HTMLElement>(".annotation-note");
    const placement = annotationNotePlacement(annotation);
    const noteWidth = annotationNoteWidth(annotation);
    const minNoteHeight = annotation.noteCollapsed ? 24 : 96;
    annotationNoteDrag = {
      id: annotation.id,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      startLeft: placement.left,
      startTop: placement.top,
      noteWidth: Math.max(noteElement?.offsetWidth ?? noteWidth, noteWidth),
      noteHeight: Math.max(noteElement?.offsetHeight ?? minNoteHeight, minNoteHeight)
    };
    handle.setPointerCapture(event.pointerId);
    if (annotation.noteCollapsed) {
      editingAnnotationId = "";
    } else {
      editAnnotation(annotation.id);
    }
  }

  function moveAnnotationNote(event: PointerEvent) {
    if (!annotationNoteDrag || annotationNoteDrag.pointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    const left = annotationNoteDrag.startLeft + event.clientX - annotationNoteDrag.startClientX;
    const top = annotationNoteDrag.startTop + event.clientY - annotationNoteDrag.startClientY;
    updateAnnotationNotePosition(
      annotationNoteDrag.id,
      clampAnnotationNotePosition(left, top, annotationNoteDrag.noteWidth, annotationNoteDrag.noteHeight)
    );
  }

  function endAnnotationNoteDrag(event: PointerEvent) {
    if (!annotationNoteDrag || annotationNoteDrag.pointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    if ((event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) {
      (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    }
    annotationNoteDrag = null;
  }

  function clampAnnotationNotePosition(
    left: number,
    top: number,
    noteWidth: number,
    noteHeight: number
  ): AnnotationNotePosition {
    const minLeft = (readerElement?.scrollLeft ?? 0) + 8;
    const minTop = (readerElement?.scrollTop ?? 0) + 8;
    const maxLeft = Math.max(minLeft, minLeft + (readerElement?.clientWidth ?? noteWidth) - noteWidth - 16);
    const maxTop = Math.max(minTop, minTop + (readerElement?.clientHeight ?? noteHeight) - noteHeight - 16);

    return {
      left: roundNumber(clampNumber(left, minLeft, maxLeft)),
      top: roundNumber(clampNumber(top, minTop, maxTop))
    };
  }

  function annotationDeleteStyle(annotation: AnnotationItem) {
    return `left: ${roundNumber(annotation.rect.left + annotation.rect.width - 12)}px; top: ${roundNumber(annotation.rect.top - 12)}px`;
  }

  async function finishCurrentPageAnnotations() {
    if (!currentAnnotations.length) {
      showUpdateToast(t.annotationNoAnnotations, "error");
      return;
    }
    const incomplete = currentAnnotations.find((annotation) => !annotation.note.trim());
    if (incomplete) {
      editAnnotation(incomplete.id);
      showUpdateToast(t.annotationEmptyNote, "error");
      return;
    }

    annotationExporting = true;
    annotationCaptureHidden = true;
    const previousEditingAnnotationId = editingAnnotationId;
    editingAnnotationId = "";
    try {
      await tick();
      await nextAnimationFrame();
      const payload = await buildAnnotationExportPayload(currentAnnotations);
      const result = await invoke<AnnotationExportResult>("finish_annotation_export", { payload });
      if (currentDocumentKey) {
        setAnnotationsForPath(currentDocumentKey, []);
      }
      annotationMode = false;
      editingAnnotationId = "";
      showUpdateToast(`${t.annotationExported}: ${result.readmePath}`);
    } catch (err) {
      editingAnnotationId = previousEditingAnnotationId;
      showUpdateToast(`${t.annotationExportFailed}: ${getErrorMessage(err)}`, "error");
    } finally {
      annotationCaptureHidden = false;
      annotationExporting = false;
    }
  }

  async function buildAnnotationExportPayload(
    annotations: AnnotationItem[]
  ): Promise<AnnotationExportPayload> {
    const document = annotations[0].document;
    const visualEvidenceById = await buildAnnotationVisualEvidenceById(annotations);

    return {
      schemaVersion: "memView.annotation.v1",
      createdAtUnixMs: Date.now(),
      app: "memView",
      documents: [
        {
          path: document.path,
          relativePath: document.relativePath,
          repoPath: document.repoPath,
          title: document.title,
          kind: document.kind,
          annotations: annotations.map((annotation) => ({
            id: annotation.id,
            note: annotation.note.trim(),
            rect: annotation.rect,
            coveredNodes: annotation.coveredNodes,
            visualEvidence:
              visualEvidenceById.get(annotation.id) ??
              unavailableAnnotationVisualEvidence("未能计算截图区域")
          }))
        }
      ]
    };
  }

  async function buildAnnotationVisualEvidenceById(annotations: AnnotationItem[]) {
    const evidenceById = new Map<string, AnnotationVisualEvidence>();
    if (!isTauri()) {
      for (const annotation of annotations) {
        evidenceById.set(
          annotation.id,
          unavailableAnnotationVisualEvidence("当前不是桌面应用环境，未生成截图")
        );
      }
      return evidenceById;
    }
    if (!readerElement) {
      for (const annotation of annotations) {
        evidenceById.set(annotation.id, unavailableAnnotationVisualEvidence("reader 不可用"));
      }
      return evidenceById;
    }

    try {
      const [webviewPosition, scaleFactor] = await Promise.all([
        getCurrentWebview().position(),
        getCurrentWindow().scaleFactor()
      ]);
      const originalScrollTop = readerElement.scrollTop;
      const originalScrollLeft = readerElement.scrollLeft;
      try {
        for (const annotation of annotations) {
          await scrollAnnotationIntoCaptureView(annotation.rect);
          const captureRect = getAnnotationScreenCaptureRect(
            annotation.rect,
            webviewPosition.x,
            webviewPosition.y,
            scaleFactor
          );
          evidenceById.set(
            annotation.id,
            captureRect
              ? await captureAnnotationVisualEvidence(captureRect)
              : unavailableAnnotationVisualEvidence(
                  "标注区域无法滚动到当前 reader 视口内，未生成截图"
                )
          );
        }
      } finally {
        readerElement.scrollTop = originalScrollTop;
        readerElement.scrollLeft = originalScrollLeft;
        await nextAnimationFrame();
      }
    } catch (err) {
      const message = `计算截图区域失败：${getErrorMessage(err)}`;
      for (const annotation of annotations) {
        evidenceById.set(annotation.id, unavailableAnnotationVisualEvidence(message));
      }
    }

    return evidenceById;
  }

  async function captureAnnotationVisualEvidence(
    captureRect: AnnotationCaptureRect
  ): Promise<AnnotationVisualEvidence> {
    try {
      const screenshotPath = await invoke<string>("capture_annotation_screenshot", {
        captureRect
      });
      return {
        screenshotPath,
        capturePadding: ANNOTATION_CAPTURE_PADDING,
        captureRect,
        captureStatus: "captured",
        captureError: null
      };
    } catch (err) {
      return {
        screenshotPath: null,
        capturePadding: ANNOTATION_CAPTURE_PADDING,
        captureRect,
        captureStatus: "unavailable",
        captureError: `生成截图失败：${getErrorMessage(err)}`
      };
    }
  }

  async function scrollAnnotationIntoCaptureView(rect: AnnotationRect) {
    if (!readerElement) {
      return;
    }

    const target = getAnnotationCaptureScrollPosition(rect);
    readerElement.scrollTop = target.top;
    readerElement.scrollLeft = target.left;
    await nextAnimationFrame();
    await nextAnimationFrame();
  }

  function getAnnotationCaptureScrollPosition(rect: AnnotationRect) {
    if (!readerElement) {
      return { top: 0, left: 0 };
    }

    const maxTop = Math.max(0, readerElement.scrollHeight - readerElement.clientHeight);
    const maxLeft = Math.max(0, readerElement.scrollWidth - readerElement.clientWidth);
    const desiredTop = rect.height + ANNOTATION_CAPTURE_PADDING * 2 <= readerElement.clientHeight
      ? rect.top - Math.max(0, (readerElement.clientHeight - rect.height) / 2)
      : rect.top - ANNOTATION_CAPTURE_PADDING;
    const desiredLeft = rect.width + ANNOTATION_CAPTURE_PADDING * 2 <= readerElement.clientWidth
      ? rect.left - Math.max(0, (readerElement.clientWidth - rect.width) / 2)
      : rect.left - ANNOTATION_CAPTURE_PADDING;

    return {
      top: clampNumber(desiredTop, 0, maxTop),
      left: clampNumber(desiredLeft, 0, maxLeft)
    };
  }

  function unavailableAnnotationVisualEvidence(message: string): AnnotationVisualEvidence {
    return {
      screenshotPath: null,
      capturePadding: ANNOTATION_CAPTURE_PADDING,
      captureRect: null,
      captureStatus: "unavailable",
      captureError: message
    };
  }

  function getAnnotationScreenCaptureRect(
    rect: AnnotationRect,
    webviewX: number,
    webviewY: number,
    scaleFactor: number
  ): AnnotationCaptureRect | null {
    if (!readerElement) {
      return null;
    }

    const readerBounds = readerElement.getBoundingClientRect();
    const left = readerBounds.left + rect.left - readerElement.scrollLeft;
    const top = readerBounds.top + rect.top - readerElement.scrollTop;
    const paddedLeft = left - ANNOTATION_CAPTURE_PADDING;
    const paddedTop = top - ANNOTATION_CAPTURE_PADDING;
    const paddedRight = left + rect.width + ANNOTATION_CAPTURE_PADDING;
    const paddedBottom = top + rect.height + ANNOTATION_CAPTURE_PADDING;
    const clipLeft = Math.max(paddedLeft, readerBounds.left, 0);
    const clipTop = Math.max(paddedTop, readerBounds.top, 0);
    const clipRight = Math.min(paddedRight, readerBounds.right, window.innerWidth);
    const clipBottom = Math.min(paddedBottom, readerBounds.bottom, window.innerHeight);
    const width = clipRight - clipLeft;
    const height = clipBottom - clipTop;

    if (width < 1 || height < 1) {
      return null;
    }

    return {
      x: Math.round(webviewX + clipLeft * scaleFactor),
      y: Math.round(webviewY + clipTop * scaleFactor),
      width: Math.max(1, Math.round(width * scaleFactor)),
      height: Math.max(1, Math.round(height * scaleFactor))
    };
  }

  function nextAnimationFrame() {
    return new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => resolve());
    });
  }

  function clampNumber(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function getCurrentAnnotationDocumentMeta(): AnnotationDocumentMeta {
    return {
      path: current?.path ?? "",
      relativePath: current?.relative_path ?? "",
      repoPath: activeViewIsFile ? null : repoPath || null,
      title: current?.title ?? "",
      kind: current ? getDocumentDisplayKind(current, activeViewIsFile) : "document"
    };
  }

  function getCoveredNodes(rect: AnnotationRect, document: Document): AnnotationCoveredNode[] {
    return getDocumentRenderer(document).getCoveredNodes(rect, document);
  }

  function getMarkdownCoveredNodes(rect: AnnotationRect, document: Document): AnnotationCoveredNode[] {
    if (!readerElement) {
      return [];
    }

    const sourceLines = document.content.split(/\r?\n/);
    const candidates = Array.from(
      readerElement.querySelectorAll<HTMLElement>("[data-mem-node-id]")
    )
      .map((element) => {
        const nodeRect = getElementContentRect(element);
        const intersectionArea = getIntersectionArea(rect, nodeRect);
        const nodeArea = nodeRect.width * nodeRect.height;
        return {
          element,
          intersectionArea,
          intersectionRatio: nodeArea > 0 ? intersectionArea / nodeArea : 0
        };
      })
      .filter((item) => item.intersectionArea > 1)
      .sort((a, b) => {
        const aRect = getElementContentRect(a.element);
        const bRect = getElementContentRect(b.element);
        return aRect.top - bRect.top || aRect.left - bRect.left;
      });

    const primary = candidates.reduce<(typeof candidates)[number] | null>(
      (best, item) => !best || item.intersectionArea > best.intersectionArea ? item : best,
      null
    );

    return candidates.map((item) => {
      const lines = getElementSourceLines(item.element);
      return {
        nodeId: item.element.dataset.memNodeId ?? "",
        type: item.element.dataset.nodeType ?? item.element.tagName.toLowerCase(),
        sourceLines: lines,
        headingPath: getHeadingPathForElement(item.element),
        textExcerpt: getElementTextExcerpt(item.element, lines, sourceLines),
        intersectionRatio: roundNumber(item.intersectionRatio),
        isPrimary: item === primary
      };
    });
  }

  function getHtmlCoveredNodes(rect: AnnotationRect): AnnotationCoveredNode[] {
    const frameDocument = getHtmlFrameDocument();
    if (!readerElement || !htmlFrameElement || !frameDocument) {
      return [];
    }

    const candidates = Array.from(
      frameDocument.querySelectorAll<HTMLElement>("[data-mem-node-id]")
    )
      .map((element) => {
        const nodeRect = getHtmlElementContentRect(element);
        const intersectionArea = getIntersectionArea(rect, nodeRect);
        const nodeArea = nodeRect.width * nodeRect.height;
        return {
          element,
          intersectionArea,
          intersectionRatio: nodeArea > 0 ? intersectionArea / nodeArea : 0
        };
      })
      .filter((item) => item.intersectionArea > 1)
      .sort((a, b) => {
        const aRect = getHtmlElementContentRect(a.element);
        const bRect = getHtmlElementContentRect(b.element);
        return aRect.top - bRect.top || aRect.left - bRect.left;
      });

    const primary = candidates.reduce<(typeof candidates)[number] | null>(
      (best, item) => !best || item.intersectionArea > best.intersectionArea ? item : best,
      null
    );

    return candidates.map((item) => ({
      nodeId: item.element.dataset.memNodeId ?? "",
      type: item.element.dataset.nodeType ?? item.element.tagName.toLowerCase(),
      sourceLines: null,
      headingPath: getHtmlHeadingPathForElement(item.element),
      textExcerpt: getHtmlElementTextExcerpt(item.element),
      intersectionRatio: roundNumber(item.intersectionRatio),
      isPrimary: item === primary
    }));
  }

  function getElementContentRect(element: HTMLElement) {
    const readerBounds = readerElement?.getBoundingClientRect();
    const bounds = element.getBoundingClientRect();
    return {
      left: roundNumber(bounds.left - (readerBounds?.left ?? 0) + (readerElement?.scrollLeft ?? 0)),
      top: roundNumber(bounds.top - (readerBounds?.top ?? 0) + (readerElement?.scrollTop ?? 0)),
      width: roundNumber(bounds.width),
      height: roundNumber(bounds.height)
    };
  }

  function getHtmlElementContentRect(element: HTMLElement) {
    const readerBounds = readerElement?.getBoundingClientRect();
    const frameBounds = htmlFrameElement?.getBoundingClientRect();
    const bounds = element.getBoundingClientRect();
    return {
      left: roundNumber(
        (frameBounds?.left ?? 0) + bounds.left - (readerBounds?.left ?? 0) + (readerElement?.scrollLeft ?? 0)
      ),
      top: roundNumber(
        (frameBounds?.top ?? 0) + bounds.top - (readerBounds?.top ?? 0) + (readerElement?.scrollTop ?? 0)
      ),
      width: roundNumber(bounds.width),
      height: roundNumber(bounds.height)
    };
  }

  function getIntersectionArea(
    a: Pick<AnnotationRect, "left" | "top" | "width" | "height">,
    b: Pick<AnnotationRect, "left" | "top" | "width" | "height">
  ) {
    const left = Math.max(a.left, b.left);
    const top = Math.max(a.top, b.top);
    const right = Math.min(a.left + a.width, b.left + b.width);
    const bottom = Math.min(a.top + a.height, b.top + b.height);
    return Math.max(0, right - left) * Math.max(0, bottom - top);
  }

  function getElementSourceLines(element: HTMLElement): AnnotationSourceLines | null {
    const start = Number(element.dataset.sourceLineStart);
    const end = Number(element.dataset.sourceLineEnd);
    if (!Number.isFinite(start) || !Number.isFinite(end) || start <= 0 || end < start) {
      return null;
    }
    return { start, end };
  }

  function getElementTextExcerpt(
    element: HTMLElement,
    sourceLines: AnnotationSourceLines | null,
    markdownLines: string[]
  ) {
    const sourceText = sourceLines
      ? markdownLines.slice(sourceLines.start - 1, sourceLines.end).join("\n")
      : "";
    const renderedText = element.innerText || element.textContent || "";
    return limitExcerpt(normalizeExcerpt(sourceText || renderedText));
  }

  function getHtmlElementTextExcerpt(element: HTMLElement) {
    const text = element.tagName.toLowerCase() === "img"
      ? element.getAttribute("alt") || element.getAttribute("title") || element.getAttribute("src") || ""
      : element.innerText || element.textContent || "";
    return limitExcerpt(normalizeExcerpt(text));
  }

  function normalizeExcerpt(value: string) {
    return value.replace(/\s+/g, " ").trim();
  }

  function limitExcerpt(value: string) {
    return value.length > 800 ? `${value.slice(0, 797)}...` : value;
  }

  function getHeadingPathForElement(element: HTMLElement) {
    if (!readerElement) {
      return [];
    }

    const path: string[] = [];
    const headings = Array.from(
      readerElement.querySelectorAll<HTMLElement>(".reader-heading[data-mem-node-id]")
    );
    for (const heading of headings) {
      const relation = heading.compareDocumentPosition(element);
      const isBeforeOrSame = heading === element || Boolean(relation & Node.DOCUMENT_POSITION_FOLLOWING);
      if (!isBeforeOrSame) {
        continue;
      }

      const level = Number(heading.tagName.slice(1));
      if (!Number.isFinite(level) || level < 1) {
        continue;
      }
      path.length = Math.max(0, level - 1);
      path[level - 1] = normalizeExcerpt(heading.textContent ?? "");
    }

    return path.filter(Boolean);
  }

  function getHtmlHeadingPathForElement(element: HTMLElement) {
    const frameDocument = getHtmlFrameDocument();
    if (!frameDocument) {
      return [];
    }

    const path: string[] = [];
    const headings = Array.from(frameDocument.querySelectorAll<HTMLElement>("h1,h2,h3,h4,h5,h6"));
    for (const heading of headings) {
      const relation = heading.compareDocumentPosition(element);
      const isBeforeOrSame = heading === element || Boolean(relation & Node.DOCUMENT_POSITION_FOLLOWING);
      if (!isBeforeOrSame) {
        continue;
      }

      const level = Number(heading.tagName.slice(1));
      if (!Number.isFinite(level) || level < 1) {
        continue;
      }
      path.length = Math.max(0, level - 1);
      path[level - 1] = normalizeExcerpt(heading.innerText || heading.textContent || "");
    }

    return path.filter(Boolean);
  }

  function createAnnotationId() {
    return `ann-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  }

  function roundNumber(value: number) {
    return Math.round(value * 100) / 100;
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
      const blob = new Blob([svgMarkup], { type: "image/svg+xml;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const revokeUrl = () => URL.revokeObjectURL(url);
      image.onload = () => {
        revokeUrl();
        resolve(image);
      };
      image.onerror = () => {
        revokeUrl();
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
        return;
      }

      if (annotationDraft) {
        annotationDraft = null;
        annotationPointerId = null;
        return;
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
    replaceSvgForeignObjectsWithText(clone);
    clone.querySelectorAll("style").forEach((style) => style.remove());
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
    return new XMLSerializer().serializeToString(clone);
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

      const computed = sourceElement.ownerDocument.defaultView?.getComputedStyle(sourceElement) ??
        getComputedStyle(sourceElement);
      for (const property of properties) {
        const value = normalizeSvgStyleValue(computed.getPropertyValue(property));
        if (value) {
          cloneElement.style.setProperty(property, value);
        }
      }
    }
  }

  function normalizeSvgStyleValue(value: string) {
    return value.replace(/url\((['"]?)[^'")#]*#([^'")]+)\1\)/g, "url(#$2)");
  }

  function replaceSvgForeignObjectsWithText(svg: SVGSVGElement) {
    svg.querySelectorAll<SVGForeignObjectElement>("foreignObject").forEach((foreignObject) => {
      const lines = getForeignObjectTextLines(foreignObject);
      if (!lines.length) {
        return;
      }

      const width = parseFloat(foreignObject.getAttribute("width") ?? "") || 0;
      const height = parseFloat(foreignObject.getAttribute("height") ?? "") || 0;
      const labelElement = foreignObject.querySelector<HTMLElement>(".nodeLabel, p, div, span");
      const fontSize = parseFloat(labelElement?.style.getPropertyValue("font-size") ?? "") || 16;
      const lineHeight = Math.max(fontSize * 1.35, fontSize + 4);
      const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
      text.setAttribute("x", String(width / 2));
      text.setAttribute("y", String(height / 2 - ((lines.length - 1) * lineHeight) / 2));
      text.setAttribute("text-anchor", "middle");
      text.setAttribute("dominant-baseline", "middle");
      text.setAttribute("font-family", labelElement?.style.getPropertyValue("font-family") || "sans-serif");
      text.setAttribute("font-size", String(fontSize));
      text.setAttribute("font-weight", labelElement?.style.getPropertyValue("font-weight") || "400");
      text.setAttribute("fill", labelElement?.style.getPropertyValue("color") || "#000000");

      lines.forEach((line, index) => {
        const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
        tspan.setAttribute("x", String(width / 2));
        tspan.setAttribute("dy", index === 0 ? "0" : String(lineHeight));
        tspan.textContent = line;
        text.appendChild(tspan);
      });

      foreignObject.replaceWith(text);
    });
  }

  function getForeignObjectTextLines(foreignObject: SVGForeignObjectElement) {
    const lines: string[] = [];
    let currentLine = "";
    const flushLine = () => {
      const line = currentLine.replace(/\s+/g, " ").trim();
      if (line) {
        lines.push(line);
      }
      currentLine = "";
    };
    const visit = (node: Node) => {
      if (node.nodeType === Node.TEXT_NODE) {
        currentLine += node.textContent ?? "";
        return;
      }
      if (!(node instanceof Element)) {
        return;
      }

      const tagName = node.tagName.toLowerCase();
      const isBlock = ["div", "p", "li", "section", "article"].includes(tagName);
      if (tagName === "br") {
        flushLine();
        return;
      }
      if (isBlock && currentLine.trim()) {
        flushLine();
      }
      node.childNodes.forEach((child) => visit(child));
      if (isBlock) {
        flushLine();
      }
    };

    foreignObject.childNodes.forEach((child) => visit(child));
    flushLine();
    return lines;
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
      <div class="toolbar-history" role="group" aria-label={t.navigationHistory}>
        <button
          class="ghost icon-button toolbar-history-button"
          type="button"
          disabled={!canNavigateBack}
          aria-label={t.navigateBack}
          title={t.navigateBack}
          on:click={() => navigateHistory(-1)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m15 18-6-6 6-6" />
          </svg>
        </button>
        <button
          class="ghost icon-button toolbar-history-button"
          type="button"
          disabled={!canNavigateForward}
          aria-label={t.navigateForward}
          title={t.navigateForward}
          on:click={() => navigateHistory(1)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m9 18 6-6-6-6" />
          </svg>
        </button>
      </div>
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
        on:click={() => checkForUpdates()}
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
        aria-label={t.openDocumentFile}
        title={t.openDocumentFile}
        on:click={browseDocumentFile}
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
        <span>{t.dropDocumentFile}</span>
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
        <p>{snapshot?.counts.documents ?? snapshot?.counts.markdown ?? 0} {t.docs} · {snapshot?.counts.mermaid ?? 0} {t.diagrams}</p>
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
        on:click={refreshCurrentRepo}
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
            class={nodeClass(node, activeRepoDocumentPath)}
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
          <div class="reader-title-line">
            <h2>{headerTitle}</h2>
            {#if headerUpdatedAt}
              <span class="reader-updated-at">{headerUpdatedAt}</span>
            {/if}
          </div>
          <div class="reader-path-row">
            <p class="reader-path">{headerPath}</p>
            {#if current?.path}
              <button
                class="ghost icon-button reader-path-copy"
                type="button"
                aria-label={t.copyDocumentPath}
                title={t.copyDocumentPath}
                on:click={copyCurrentDocumentPath}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <rect x="9" y="9" width="10" height="10" rx="2" />
                  <path d="M5 15V7a2 2 0 0 1 2-2h8" />
                </svg>
              </button>
            {/if}
          </div>
        </div>
      </div>
      <div class="head-actions">
        {#if !activeViewIsFile && repoPath}
          <button
            class="ghost icon-button reader-refresh"
            type="button"
            disabled={repoBusy}
            aria-label={t.refresh}
            title={t.refresh}
            on:click={refreshCurrentRepo}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M20 6v5h-5" />
              <path d="M4 18v-5h5" />
              <path d="M18.4 9A7 7 0 0 0 6.2 7.2L4 9.3" />
              <path d="M5.6 15A7 7 0 0 0 17.8 16.8L20 14.7" />
            </svg>
          </button>
        {/if}
        <button
          class="ghost annotation-toggle"
          class:active={annotationMode}
          type="button"
          disabled={!current || annotationExporting}
          aria-pressed={annotationMode}
          aria-label={annotationModeButtonLabel}
          title={annotationModeButtonLabel}
          on:click={toggleAnnotationMode}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              class="annotation-bubble-outline"
              d="M5.5 5.5h13a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H12l-4.6 3.2v-3.2H5.5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z"
            />
            <path class="annotation-bubble-line" d="M7.5 9.2h8.6" />
            <path class="annotation-bubble-line" d="M7.5 12.6h6.3" />
          </svg>
          <span>{annotationModeButtonLabel}</span>
        </button>
        <button
          class="primary annotation-finish"
          type="button"
          disabled={!current || !currentAnnotations.length || annotationExporting}
          aria-label={finishAnnotationButtonLabel}
          title={finishAnnotationButtonLabel}
          on:click={finishCurrentPageAnnotations}
        >
          <span>{annotationExporting ? t.annotationExporting : t.finishAnnotations}</span>
          {#if currentAnnotations.length}
            <strong>{currentAnnotations.length}</strong>
          {/if}
        </button>
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
        <article
          class="reader"
          class:annotating={annotationMode}
          class:capturing={annotationCaptureHidden}
          class:html-document={activeRendererId === "html"}
          bind:this={readerElement}
          on:wheel|nonpassive={handleReaderWheel}
          on:pointerdown={handleReaderPointerDown}
          on:pointermove={handleReaderPointerMove}
          on:pointerup={handleReaderPointerUp}
          on:pointercancel={handleReaderPointerCancel}
        >
          {#if activeRendererId === "html"}
            <iframe
              class="html-reader-frame"
              title={current.title}
              srcdoc={htmlFrameSrcdoc}
              sandbox="allow-same-origin allow-scripts allow-forms allow-modals allow-popups"
              scrolling="no"
              bind:this={htmlFrameElement}
              on:load={handleHtmlFrameLoad}
            ></iframe>
          {:else}
            {@html renderedHtml}
          {/if}
          {#each currentAnnotations as annotation (annotation.id)}
            {@const notePlacement = annotationNotePlacement(annotation)}
            <div
              class="annotation-box"
              class:active={editingAnnotationId === annotation.id}
              style={`left: ${annotation.rect.left}px; top: ${annotation.rect.top}px; width: ${annotation.rect.width}px; height: ${annotation.rect.height}px`}
              role="button"
              tabindex="0"
              aria-label={t.editAnnotation}
              on:pointerdown={(event) => event.stopPropagation()}
              on:click={() => editAnnotation(annotation.id)}
              on:keydown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  editAnnotation(annotation.id);
                }
              }}
            ></div>
            <button
              class="annotation-box-delete"
              class:active={editingAnnotationId === annotation.id}
              type="button"
              style={annotationDeleteStyle(annotation)}
              aria-label={t.deleteAnnotation}
              title={t.deleteAnnotation}
              on:pointerdown={(event) => event.stopPropagation()}
              on:click={() => removeAnnotation(annotation.id)}
            >
              x
            </button>
            <div
              class={`annotation-note ${notePlacement.className}`}
              class:active={editingAnnotationId === annotation.id}
              class:collapsed={annotation.noteCollapsed}
              class:dragging={annotationNoteDrag?.id === annotation.id}
              style={notePlacement.style}
              role="group"
              aria-label={t.editAnnotation}
              on:pointerdown={(event) => {
                event.stopPropagation();
                if (event.target instanceof HTMLTextAreaElement) {
                  return;
                }
                editAnnotation(annotation.id);
              }}
            >
              <div class="annotation-note-toolbar">
                <button
                  class="annotation-note-drag-handle"
                  type="button"
                  aria-label={t.moveAnnotationNote}
                  title={t.moveAnnotationNote}
                  on:pointerdown={(event) => startAnnotationNoteDrag(annotation, event)}
                  on:pointermove={moveAnnotationNote}
                  on:pointerup={endAnnotationNoteDrag}
                  on:pointercancel={endAnnotationNoteDrag}
                >
                  <span class="annotation-note-grip" aria-hidden="true"></span>
                </button>
                <button
                  class="annotation-note-toggle"
                  type="button"
                  aria-label={annotation.noteCollapsed ? t.expandAnnotationNote : t.collapseAnnotationNote}
                  title={annotation.noteCollapsed ? t.expandAnnotationNote : t.collapseAnnotationNote}
                  aria-expanded={!annotation.noteCollapsed}
                  on:pointerdown={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    toggleAnnotationNoteCollapsed(annotation.id);
                  }}
                >
                  <svg viewBox="0 0 16 16" aria-hidden="true">
                    {#if annotation.noteCollapsed}
                      <path d="M4 6l4 4 4-4" />
                    {:else}
                      <path d="M4 10l4-4 4 4" />
                    {/if}
                  </svg>
                </button>
              </div>
              {#if !annotation.noteCollapsed}
                <textarea
                  data-annotation-id={annotation.id}
                  value={annotation.note}
                  placeholder={t.annotationNotePlaceholder}
                  aria-label={t.editAnnotation}
                  on:focus={() => (editingAnnotationId = annotation.id)}
                  on:input={(event) => updateAnnotationNote(annotation.id, event.currentTarget.value)}
                  on:blur={() => {
                    if (!annotationNoteDrag) {
                      editingAnnotationId = "";
                    }
                  }}
                ></textarea>
              {/if}
            </div>
          {/each}
          {#if annotationDraft}
            <div
              class="annotation-box draft"
              style={`left: ${annotationDraft.rect.left}px; top: ${annotationDraft.rect.top}px; width: ${annotationDraft.rect.width}px; height: ${annotationDraft.rect.height}px`}
              aria-hidden="true"
            ></div>
          {/if}
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
            <button class="ghost" type="button" disabled={repoBusy} on:click={browseDocumentFile}>
              {t.openDocumentFile}
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
          <dd>{current ? formatKind(getDocumentDisplayKind(current, activeViewIsFile)) : "-"}</dd>
          <dt>{t.path}</dt>
          <dd>{activeViewIsFile ? current?.path ?? "-" : current?.relative_path ?? "-"}</dd>
          {#if current?.content_type === "markdown"}
            <dt>{t.mermaid}</dt>
            <dd>{current.has_mermaid ? t.yes : t.no}</dd>
          {/if}
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
