use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct RepoSnapshot {
    root_path: String,
    tree: Vec<TreeNode>,
    docs: Vec<DocMeta>,
    counts: RepoCounts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPullResult {
    root_path: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct RepoCounts {
    documents: usize,
    markdown: usize,
    html: usize,
    mermaid: usize,
    requirements: usize,
}

#[derive(Debug, Serialize, Clone)]
struct DocMeta {
    id: String,
    title: String,
    path: String,
    relative_path: String,
    kind: String,
    content_type: String,
    modified_at_unix_ms: u64,
    has_mermaid: bool,
}

#[derive(Debug, Serialize)]
struct TreeNode {
    id: String,
    title: String,
    path: Option<String>,
    kind: String,
    children: Vec<TreeNode>,
}

#[derive(Debug, Serialize)]
struct Document {
    id: String,
    title: String,
    path: String,
    relative_path: String,
    kind: String,
    content: String,
    content_type: String,
    modified_at_unix_ms: u64,
    has_mermaid: bool,
    read_chain: Vec<ChainItem>,
}

#[derive(Debug, Serialize)]
struct ChainItem {
    label: String,
    path: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardImage {
    png_base64: String,
}

#[derive(Debug, Deserialize)]
struct ClipboardSvg {
    svg: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationExportPayload {
    schema_version: String,
    created_at_unix_ms: u64,
    app: String,
    documents: Vec<AnnotationDocument>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationDocument {
    path: String,
    relative_path: String,
    repo_path: Option<String>,
    title: String,
    kind: String,
    annotations: Vec<AnnotationItem>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationItem {
    id: String,
    note: String,
    rect: AnnotationRect,
    covered_nodes: Vec<AnnotationCoveredNode>,
    visual_evidence: Option<AnnotationVisualEvidence>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    scroll_top: f64,
    scroll_left: f64,
    reader_width: f64,
    reader_height: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationCoveredNode {
    node_id: String,
    #[serde(rename = "type")]
    node_type: String,
    source_lines: Option<AnnotationSourceLines>,
    heading_path: Vec<String>,
    text_excerpt: String,
    intersection_ratio: f64,
    is_primary: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationSourceLines {
    start: usize,
    end: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AnnotationVisualEvidence {
    screenshot_path: Option<String>,
    capture_padding: f64,
    capture_rect: Option<AnnotationCaptureRect>,
    capture_status: AnnotationCaptureStatus,
    capture_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AnnotationCaptureRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
enum AnnotationCaptureStatus {
    Captured,
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationExportResult {
    annotation_directory_path: String,
    readme_path: String,
    prompt: String,
}

#[allow(dead_code)]
struct AnnotationBundlePaths {
    directory_path: PathBuf,
    annotations_path: PathBuf,
    readme_path: PathBuf,
}

const ANNOTATION_EXPORT_TASK: &str = "根据 memView 标注处理对应规格文档。";
const ANNOTATION_EXPORT_WORK_REQUIREMENTS: &[&str] = &[
    "先读取本 README.md，再理解 annotations.json 中的 documents[].path、annotations[].note、coveredNodes、rect 和 visualEvidence。",
    "再读取 documents[].path 指向的原文档文件。",
    "优先使用 coveredNodes[].sourceLines、headingPath 和 textExcerpt 定位标注对应的规格内容；rect 只作为视觉辅助，不要只凭坐标判断。",
    "对每条 annotations[].note 先判断意图：如果 note 明确要求修正、补充、删除、改写或同步规格内容，才按该要求做最小范围修改。",
    "如果 note 是问题、求解释、求确认、求分析，或修改意图不明确，只基于对应文档内容回答问题，不要改文件。",
    "如果 note 同时包含问题和明确修改要求，先回答问题，再只修改明确要求修改的内容。",
    "不要修改未被标注要求影响的内容；没有明确修改要求时不要为了回答问题而改文档。",
    "完成后说明修改了哪些文件；如果没有修改则说明无文件修改，并逐条说明每处标注的意图判断、定位依据和处理结果。",
];

#[tauri::command]
fn scan_repo(repo_path: String) -> Result<RepoSnapshot, String> {
    let root = normalize_repo_path(&repo_path)?;
    let mut docs = scan_documents(&root)?;
    docs.sort_by(|a, b| sort_key(&a.relative_path).cmp(&sort_key(&b.relative_path)));

    let counts = RepoCounts {
        documents: docs.len(),
        markdown: docs.iter().filter(|doc| doc.content_type == "markdown").count(),
        html: docs.iter().filter(|doc| doc.content_type == "html").count(),
        mermaid: docs.iter().filter(|doc| doc.has_mermaid).count(),
        requirements: count_requirements(&root),
    };

    Ok(RepoSnapshot {
        root_path: root.to_string_lossy().to_string(),
        tree: build_tree(&tree_docs(&docs)),
        docs,
        counts,
    })
}

#[tauri::command]
fn pull_repo(repo_path: String) -> Result<GitPullResult, String> {
    let root = normalize_repo_path(&repo_path)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("pull")
        .arg("--ff-only")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|err| format!("无法执行 git pull：{}", err))?;
    let message = command_output_text(&output.stdout, &output.stderr);

    if output.status.success() {
        Ok(GitPullResult {
            root_path: root.to_string_lossy().to_string(),
            message,
        })
    } else {
        let detail = if message.is_empty() {
            output.status.to_string()
        } else {
            message
        };
        Err(format!("git pull --ff-only 执行失败：\n{}", detail))
    }
}

#[tauri::command]
fn read_document(repo_path: String, path: String) -> Result<Document, String> {
    let root = normalize_repo_path(&repo_path)?;
    let requested = PathBuf::from(path);
    let requested = canonical_readable_child(&root, &requested)?;
    let relative_path = requested
        .strip_prefix(&root)
        .map_err(|_| "文档不在当前记忆库内".to_string())?
        .to_string_lossy()
        .to_string();
    let read_chain = build_read_chain(&root, &relative_path);

    read_document_file(&requested, relative_path, read_chain)
}

#[tauri::command]
fn read_standalone_document(path: String) -> Result<Document, String> {
    let requested = PathBuf::from(path.trim());
    let requested = canonical_readable_document_file(&requested)?;
    let relative_path = requested
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document")
        .to_string();

    read_document_file(&requested, relative_path, Vec::new())
}

#[tauri::command]
fn read_markdown_file(path: String) -> Result<Document, String> {
    read_standalone_document(path)
}

#[tauri::command]
fn copy_image_to_clipboard(image: ClipboardImage) -> Result<(), String> {
    let bytes = general_purpose::STANDARD
        .decode(image.png_base64.trim())
        .map_err(|err| format!("图片数据解析失败：{}", err))?;

    #[cfg(target_os = "macos")]
    if let Err(err) = copy_png_to_macos_clipboard(&bytes) {
        eprintln!("macOS clipboard fallback failed: {}", err);
    } else {
        return Ok(());
    }

    let decoded = image::load_from_memory(&bytes)
        .map_err(|err| format!("图片解码失败：{}", err))?
        .to_rgba8();
    let width = decoded.width() as usize;
    let height = decoded.height() as usize;
    if width == 0 || height == 0 {
        return Err("图片尺寸无效".to_string());
    }

    let mut clipboard =
        Clipboard::new().map_err(|err| format!("打开系统剪贴板失败：{}", err))?;
    clipboard
        .set_image(ImageData {
            width,
            height,
            bytes: Cow::Owned(decoded.into_raw()),
        })
        .map_err(|err| format!("写入系统剪贴板失败：{}", err))
}

#[tauri::command]
fn copy_svg_to_clipboard(image: ClipboardSvg) -> Result<(), String> {
    if image.svg.trim().is_empty() {
        return Err("SVG 内容为空".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let png = render_svg_to_png_with_sips(&image.svg)?;
        return copy_png_to_macos_clipboard(&png);
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("当前平台暂不支持直接复制 SVG 图".to_string())
    }
}

#[tauri::command]
fn finish_annotation_export(
    payload: AnnotationExportPayload,
) -> Result<AnnotationExportResult, String> {
    let bundle_paths = write_annotation_export(payload)?;
    let readme_path_string = bundle_paths.readme_path.to_string_lossy().to_string();
    let prompt = build_annotation_prompt(&readme_path_string);
    copy_text_to_clipboard(&prompt)?;

    Ok(AnnotationExportResult {
        annotation_directory_path: bundle_paths.directory_path.to_string_lossy().to_string(),
        readme_path: readme_path_string,
        prompt,
    })
}

fn write_annotation_export(
    mut payload: AnnotationExportPayload,
) -> Result<AnnotationBundlePaths, String> {
    validate_annotation_export(&payload)?;
    let directory_path = annotation_temp_dir_path(&std::env::temp_dir(), unix_timestamp_millis());
    let images_path = directory_path.join("images");
    fs::create_dir_all(&images_path)
        .map_err(|err| format!("创建标注临时目录失败：{} ({})", images_path.display(), err))?;

    finalize_annotation_visual_evidence(&mut payload, &images_path);

    let annotations_path = directory_path.join("annotations.json");
    write_annotation_export_to_path(&payload, &annotations_path)?;

    let readme_path = directory_path.join("README.md");
    let readme = build_annotation_readme(&payload, &annotations_path, &images_path);
    fs::write(&readme_path, readme)
        .map_err(|err| format!("写入标注说明文件失败：{} ({})", readme_path.display(), err))?;

    Ok(AnnotationBundlePaths {
        directory_path,
        annotations_path,
        readme_path,
    })
}

fn write_annotation_export_to_path(
    payload: &AnnotationExportPayload,
    path: &Path,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("序列化标注文件失败：{}", err))?;
    fs::write(path, json)
        .map_err(|err| format!("写入标注临时文件失败：{} ({})", path.display(), err))
}

fn finalize_annotation_visual_evidence(payload: &mut AnnotationExportPayload, images_path: &Path) {
    let mut index = 0usize;
    for annotation in payload
        .documents
        .iter_mut()
        .flat_map(|document| document.annotations.iter_mut())
    {
        let mut evidence = annotation
            .visual_evidence
            .take()
            .unwrap_or_else(|| unavailable_visual_evidence(None, 0.0, "前端未提供截图区域"));
        evidence.screenshot_path = None;
        evidence.capture_status = AnnotationCaptureStatus::Unavailable;

        if let Some(capture_rect) = evidence.capture_rect.clone() {
            let file_name = annotation_image_file_name(&annotation.id, index);
            let screenshot_path = images_path.join(file_name);
            match capture_screen_region_to_png(&capture_rect, &screenshot_path) {
                Ok(()) => {
                    evidence.screenshot_path = Some(screenshot_path.to_string_lossy().to_string());
                    evidence.capture_status = AnnotationCaptureStatus::Captured;
                    evidence.capture_error = None;
                }
                Err(err) => {
                    evidence.capture_error = Some(err);
                }
            }
        } else if evidence.capture_error.is_none() {
            evidence.capture_error =
                Some("标注区域不在当前可见 reader 视口内，未生成截图".to_string());
        }

        annotation.visual_evidence = Some(evidence);
        index += 1;
    }
}

fn unavailable_visual_evidence(
    capture_rect: Option<AnnotationCaptureRect>,
    capture_padding: f64,
    capture_error: impl Into<String>,
) -> AnnotationVisualEvidence {
    AnnotationVisualEvidence {
        screenshot_path: None,
        capture_padding,
        capture_rect,
        capture_status: AnnotationCaptureStatus::Unavailable,
        capture_error: Some(capture_error.into()),
    }
}

fn annotation_image_file_name(annotation_id: &str, index: usize) -> String {
    let sanitized: String = annotation_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let stem = if sanitized.trim_matches('_').is_empty() {
        format!("annotation-{}", index + 1)
    } else {
        sanitized
    };
    format!("{}.png", stem)
}

fn build_annotation_readme(
    payload: &AnnotationExportPayload,
    annotations_path: &Path,
    images_path: &Path,
) -> String {
    let annotation_count: usize = payload
        .documents
        .iter()
        .map(|document| document.annotations.len())
        .sum();
    let document_list = payload
        .documents
        .iter()
        .map(|document| {
            format!(
                "- `{}`：{} 条标注",
                document.path,
                document.annotations.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let work_requirements = ANNOTATION_EXPORT_WORK_REQUIREMENTS
        .iter()
        .enumerate()
        .map(|(index, requirement)| format!("{}. {}", index + 1, requirement))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# memView 标注任务

{task}

## 文件

- 结构化数据：`{annotations_path}`
- 局部截图目录：`{images_path}`
- 文档数量：{document_count}
- 标注数量：{annotation_count}

## 待处理文档

{document_list}

## 工作要求

{work_requirements}

## 字段说明

- `documents[].path`：被标注的原文档绝对路径，处理前必须读取。
- `annotations[].note`：用户写下的标注意图原文，它是判断要修改、回答还是确认的最高优先级输入。
- `annotations[].coveredNodes`：memView 根据框选区域推断出的候选文档节点，不是绝对真相。
- `coveredNodes[].sourceLines`、`textExcerpt`、`headingPath`：优先用于定位源文；HTML 文档可能没有 sourceLines，优先级高于截图。
- `annotations[].rect`：用户框选区域在 reader 内容坐标系里的位置，不是屏幕坐标。
- `annotations[].visualEvidence`：截图辅助信息。`screenshotPath` 指向局部 PNG；`captureRect` 是生成截图时使用的屏幕坐标；`captureStatus` 为 `captured` 才表示截图可用。

## 视觉证据使用原则

- 截图只用于确认视觉上下文，不能替代源文、`textExcerpt` 和可用的 `sourceLines`。
- Mermaid、表格、图片等复杂排版场景，可以用截图和图内文字判断用户实际框选的局部。
- Mermaid 的源码修改仍以 Markdown 中的 Mermaid 代码块为准；截图只是帮助判断具体节点、边或说明文字。
- 如果 `captureStatus` 是 `unavailable`，忽略截图并根据 `note`、`coveredNodes` 和原文处理。
"#,
        task = ANNOTATION_EXPORT_TASK,
        annotations_path = annotations_path.display(),
        images_path = images_path.display(),
        document_count = payload.documents.len(),
        annotation_count = annotation_count,
        document_list = if document_list.is_empty() {
            "- 无".to_string()
        } else {
            document_list
        },
        work_requirements = work_requirements
    )
}

fn validate_annotation_export(payload: &AnnotationExportPayload) -> Result<(), String> {
    if payload.schema_version.trim().is_empty() {
        return Err("标注 schemaVersion 为空".to_string());
    }
    if payload.documents.is_empty() {
        return Err("没有可导出的标注文档".to_string());
    }
    let annotation_count: usize = payload
        .documents
        .iter()
        .map(|document| document.annotations.len())
        .sum();
    if annotation_count == 0 {
        return Err("没有可导出的标注".to_string());
    }
    if payload.documents.iter().any(|document| document.path.trim().is_empty()) {
        return Err("标注文档路径不能为空".to_string());
    }
    if payload
        .documents
        .iter()
        .flat_map(|document| document.annotations.iter())
        .any(|annotation| annotation.note.trim().is_empty())
    {
        return Err("标注备注不能为空".to_string());
    }
    Ok(())
}

fn annotation_temp_dir_path(base: &Path, timestamp_millis: u128) -> PathBuf {
    base.join(format!(
        "mem-view-annotations-{}-{}",
        std::process::id(),
        timestamp_millis
    ))
}

fn build_annotation_prompt(readme_path: &str) -> String {
    format!(
        "请先读取并严格执行 memView 标注目录中的 README.md：\n{}\n\n再结合同目录 annotations.json 和 images/ 里的截图证据处理对应文档。",
        readme_path
    )
}

fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard =
        Clipboard::new().map_err(|err| format!("打开系统剪贴板失败：{}", err))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| format!("写入系统剪贴板失败：{}", err))
}

#[cfg(target_os = "macos")]
fn capture_screen_region_to_png(rect: &AnnotationCaptureRect, path: &Path) -> Result<(), String> {
    let region = screencapture_region_arg(rect)?;
    let output = Command::new("screencapture")
        .arg("-x")
        .arg("-R")
        .arg(region)
        .arg(path)
        .output()
        .map_err(|err| format!("调用系统截图失败：{}", err))?;

    if output.status.success() && path.exists() {
        return Ok(());
    }

    let _ = fs::remove_file(path);
    let detail = command_output_text(&output.stdout, &output.stderr);
    if detail.is_empty() {
        Err("系统截图失败，可能缺少屏幕录制权限".to_string())
    } else {
        Err(format!("系统截图失败：{}", detail))
    }
}

#[cfg(not(target_os = "macos"))]
fn capture_screen_region_to_png(_rect: &AnnotationCaptureRect, _path: &Path) -> Result<(), String> {
    Err("当前平台暂不支持自动截图".to_string())
}

fn screencapture_region_arg(rect: &AnnotationCaptureRect) -> Result<String, String> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width < 1.0
        || rect.height < 1.0
    {
        return Err("截图区域无效".to_string());
    }

    Ok(format!(
        "{},{},{},{}",
        rect.x.round() as i64,
        rect.y.round() as i64,
        rect.width.round().max(1.0) as u64,
        rect.height.round().max(1.0) as u64
    ))
}

#[cfg(target_os = "macos")]
fn copy_png_to_macos_clipboard(bytes: &[u8]) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "mem-view-diagram-{}-{}.png",
        std::process::id(),
        unix_timestamp_nanos()
    ));
    fs::write(&path, bytes).map_err(|err| format!("写入临时图片失败：{}", err))?;

    let script = format!(
        "set the clipboard to (read (POSIX file \"{}\") as TIFF picture)",
        escape_applescript_string(&path.to_string_lossy())
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| {
            let _ = fs::remove_file(&path);
            format!("调用系统剪贴板失败：{}", err)
        })?;
    let _ = fs::remove_file(&path);

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "系统剪贴板写入失败：{}{}",
        stderr.trim(),
        stdout.trim()
    ))
}

#[cfg(target_os = "macos")]
fn render_svg_to_png_with_sips(svg: &str) -> Result<Vec<u8>, String> {
    let stem = format!(
        "mem-view-diagram-{}-{}",
        std::process::id(),
        unix_timestamp_nanos()
    );
    let svg_path = std::env::temp_dir().join(format!("{}.svg", stem));
    let png_path = std::env::temp_dir().join(format!("{}.png", stem));
    fs::write(&svg_path, svg).map_err(|err| format!("写入临时 SVG 失败：{}", err))?;

    let output = Command::new("sips")
        .arg("-s")
        .arg("format")
        .arg("png")
        .arg(&svg_path)
        .arg("--out")
        .arg(&png_path)
        .output()
        .map_err(|err| {
            let _ = fs::remove_file(&svg_path);
            format!("调用系统 SVG 转图片失败：{}", err)
        })?;
    let _ = fs::remove_file(&svg_path);

    if !output.status.success() {
        let _ = fs::remove_file(&png_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "SVG 转图片失败：{}{}",
            stderr.trim(),
            stdout.trim()
        ));
    }

    let png = fs::read(&png_path).map_err(|err| format!("读取临时图片失败：{}", err))?;
    let _ = fs::remove_file(&png_path);
    Ok(png)
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn unix_timestamp_nanos() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn unix_timestamp_millis() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn read_document_file(
    requested: &Path,
    relative_path: String,
    read_chain: Vec<ChainItem>,
) -> Result<Document, String> {
    let content_type = document_content_type(requested)
        .ok_or_else(|| format!("不支持的文档类型：{}", requested.display()))?;
    let content = fs::read_to_string(&requested)
        .map_err(|err| format!("读取文档失败：{} ({})", requested.display(), err))?;
    let title = extract_document_title(&content, content_type).unwrap_or_else(|| fallback_title(&requested));
    let has_mermaid = content_type == "markdown" && content.contains("```mermaid");

    Ok(Document {
        id: relative_path.clone(),
        title,
        path: requested.to_string_lossy().to_string(),
        relative_path: relative_path.clone(),
        kind: classify_doc(&relative_path),
        content,
        content_type: content_type.to_string(),
        modified_at_unix_ms: file_modified_unix_ms(requested),
        has_mermaid,
        read_chain,
    })
}

fn normalize_repo_path(repo_path: &str) -> Result<PathBuf, String> {
    if repo_path.trim().is_empty() {
        return Err("请选择一个记忆库目录".to_string());
    }

    let path = PathBuf::from(repo_path);
    let canonical = path
        .canonicalize()
        .map_err(|err| format!("记忆库不存在或不可读：{} ({})", path.display(), err))?;
    if !canonical.is_dir() {
        return Err(format!("记忆库路径不是目录：{}", canonical.display()));
    }
    find_git_root(&canonical).ok_or_else(|| {
        format!(
            "记忆库必须是 Git 项目：{}（请选择仓库根目录或仓库内目录）",
            canonical.display()
        )
    })
}

fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        let dot_git = current.join(".git");
        if dot_git.is_dir() || dot_git.is_file() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{}\n{}", stdout, stderr),
    }
}

fn scan_documents(root: &Path) -> Result<Vec<DocMeta>, String> {
    let mut docs = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| entry.file_name().to_string_lossy() != ".git")
    {
        let entry = entry.map_err(|err| format!("扫描文件失败：{}", err))?;
        let path = entry.path();
        let Some(content_type) = document_content_type(path) else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .map_err(|_| "路径解析失败".to_string())?
            .to_string_lossy()
            .to_string();
        let content = fs::read_to_string(path).unwrap_or_default();
        let title = extract_document_title(&content, content_type).unwrap_or_else(|| fallback_title(path));

        docs.push(DocMeta {
            id: relative_path.clone(),
            title,
            path: path.to_string_lossy().to_string(),
            kind: classify_doc(&relative_path),
            content_type: content_type.to_string(),
            modified_at_unix_ms: file_modified_unix_ms(path),
            relative_path,
            has_mermaid: content_type == "markdown" && content.contains("```mermaid"),
        });
    }

    Ok(docs)
}

fn count_requirements(root: &Path) -> usize {
    let requirements = root.join("requirements");
    match fs::read_dir(requirements) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .count(),
        Err(_) => 0,
    }
}

fn build_tree(docs: &[DocMeta]) -> Vec<TreeNode> {
    let mut root = MutableNode::new("root", "mem", "root", None);
    for doc in docs {
        let parts: Vec<&str> = doc.relative_path.split('/').collect();
        root.insert(&parts, doc);
    }
    root.children
        .into_values()
        .map(MutableNode::into_tree_node)
        .collect()
}

#[derive(Debug)]
struct MutableNode {
    id: String,
    title: String,
    kind: String,
    path: Option<String>,
    children: BTreeMap<String, MutableNode>,
}

impl MutableNode {
    fn new(id: &str, title: &str, kind: &str, path: Option<String>) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            kind: kind.to_string(),
            path,
            children: BTreeMap::new(),
        }
    }

    fn insert(&mut self, parts: &[&str], doc: &DocMeta) {
        self.insert_with_prefix(parts, doc, "");
    }

    fn insert_with_prefix(&mut self, parts: &[&str], doc: &DocMeta, prefix: &str) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            self.children.insert(
                sort_key(&doc.relative_path),
                MutableNode::new(&doc.id, &tree_doc_title(doc), &doc.kind, Some(doc.path.clone())),
            );
            return;
        }

        let segment = parts[0];
        let node_id = if prefix.is_empty() {
            segment.to_string()
        } else {
            format!("{}/{}", prefix, segment)
        };
        let child = self.children.entry(format!("0-{}", segment)).or_insert_with(|| {
            MutableNode::new(&node_id, &pretty_segment(segment), "folder", None)
        });
        child.insert_with_prefix(&parts[1..], doc, &node_id);
    }

    fn into_tree_node(self) -> TreeNode {
        TreeNode {
            id: self.id,
            title: self.title,
            path: self.path,
            kind: self.kind,
            children: self
                .children
                .into_values()
                .map(MutableNode::into_tree_node)
                .collect(),
        }
    }
}

fn tree_docs(docs: &[DocMeta]) -> Vec<DocMeta> {
    docs.iter()
        .filter(|doc| doc.relative_path != "README.md")
        .cloned()
        .collect()
}

fn tree_doc_title(doc: &DocMeta) -> String {
    Path::new(&doc.relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&doc.relative_path)
        .to_string()
}

fn build_read_chain(root: &Path, relative_path: &str) -> Vec<ChainItem> {
    let mut chain = Vec::new();
    let parts: Vec<&str> = relative_path.split('/').collect();

    if relative_path.starts_with("baseline/") {
        push_chain(&mut chain, "baseline", root.join("baseline/README.md"));
        return chain;
    }

    if parts.first() == Some(&"requirements") && parts.len() >= 2 {
        let requirement_root = root.join("requirements").join(parts[1]);
        if parts.contains(&"missions") || parts.contains(&"tasks") {
            if let Some(position) = parts.iter().position(|part| *part == "missions" || *part == "tasks") {
                if let Some(child_id) = parts.get(position + 1) {
                    push_chain(
                        &mut chain,
                        parts[position],
                        requirement_root.join(parts[position]).join(child_id).join("README.md"),
                    );
                }
            }
        }
        push_chain(&mut chain, "requirement", requirement_root.join("README.md"));
        push_chain(&mut chain, "baseline", root.join("baseline/README.md"));
    }

    chain
}

fn push_chain(chain: &mut Vec<ChainItem>, label: &str, path: PathBuf) {
    if !path.exists() {
        return;
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    let content_type = document_content_type(&path).unwrap_or("markdown");
    chain.push(ChainItem {
        label: label.to_string(),
        title: extract_document_title(&content, content_type).unwrap_or_else(|| fallback_title(&path)),
        path: path.to_string_lossy().to_string(),
    });
}

fn canonical_readable_child(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let canonical = requested
        .canonicalize()
        .map_err(|err| format!("文档不存在或不可读：{} ({})", requested.display(), err))?;
    if !canonical.starts_with(root) {
        return Err("只能读取当前记忆库内的文件".to_string());
    }
    Ok(canonical)
}

fn canonical_readable_document_file(requested: &Path) -> Result<PathBuf, String> {
    let canonical = requested
        .canonicalize()
        .map_err(|err| format!("文档文件不存在或不可读：{} ({})", requested.display(), err))?;
    if !canonical.is_file() {
        return Err(format!("文档路径不是文件：{}", canonical.display()));
    }
    if document_content_type(&canonical).is_none() {
        return Err(format!("只能打开 Markdown 或 HTML 文件：{}", canonical.display()));
    }
    Ok(canonical)
}

fn document_content_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" => Some("markdown"),
        "html" | "htm" => Some("html"),
        _ => None,
    }
}

fn file_modified_unix_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn extract_document_title(content: &str, content_type: &str) -> Option<String> {
    match content_type {
        "html" => extract_html_title(content),
        _ => extract_markdown_title(content),
    }
}

fn extract_markdown_title(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("title:") {
            return Some(title.trim().trim_matches('"').to_string()).filter(|value| !value.is_empty());
        }
        if let Some(title) = trimmed.strip_prefix("# ") {
            return Some(title.trim().to_string()).filter(|value| !value.is_empty());
        }
    }
    None
}

fn extract_html_title(html: &str) -> Option<String> {
    extract_html_tag_text(html, "title")
        .or_else(|| extract_html_tag_text(html, "h1"))
}

fn extract_html_tag_text(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open_pattern = format!("<{}", tag);
    let mut search_from = 0;

    while let Some(open_start) = lower[search_from..].find(&open_pattern) {
        let open_start = search_from + open_start;
        let after_tag = lower.as_bytes().get(open_start + open_pattern.len()).copied();
        if !matches!(after_tag, Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')) {
            search_from = open_start + open_pattern.len();
            continue;
        }
        let Some(open_end_offset) = lower[open_start..].find('>') else {
            return None;
        };
        let content_start = open_start + open_end_offset + 1;
        let close_pattern = format!("</{}>", tag);
        let Some(close_offset) = lower[content_start..].find(&close_pattern) else {
            return None;
        };
        let content_end = content_start + close_offset;
        return clean_html_text(&html[content_start..content_end]);
    }

    None
}

fn clean_html_text(value: &str) -> Option<String> {
    let mut text = String::new();
    let mut in_tag = false;

    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }

    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let normalized = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(normalized).filter(|value| !value.is_empty())
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .replace('-', " ")
}

fn classify_doc(relative_path: &str) -> String {
    if relative_path == "README.md" {
        "repo".to_string()
    } else if relative_path.starts_with("baseline/") {
        "baseline".to_string()
    } else if relative_path.contains("/missions/") {
        "mission".to_string()
    } else if relative_path.contains("/tasks/") {
        "task".to_string()
    } else if relative_path.starts_with("requirements/") {
        "requirement".to_string()
    } else {
        "document".to_string()
    }
}

fn sort_key(relative_path: &str) -> String {
    let readme_rank = if relative_path.ends_with("README.md") { "0" } else { "1" };
    format!("{}-{}", readme_rank, relative_path.to_lowercase())
}

fn pretty_segment(segment: &str) -> String {
    match segment {
        "baseline" => "Baseline".to_string(),
        "requirements" => "Requirements".to_string(),
        "missions" => "Missions".to_string(),
        "tasks" => "Tasks".to_string(),
        _ => segment.replace('-', " "),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_repo,
            pull_repo,
            read_document,
            read_standalone_document,
            read_markdown_file,
            finish_annotation_export,
            copy_svg_to_clipboard,
            copy_image_to_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running memView");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scans_memory_repo() {
        let root = build_test_repo();
        let snapshot =
            scan_repo(root.to_string_lossy().to_string()).expect("test mem repo should scan");

        assert_eq!(snapshot.root_path, root.to_string_lossy().to_string());
        assert_eq!(snapshot.counts.documents, 6);
        assert_eq!(snapshot.counts.markdown, 4);
        assert_eq!(snapshot.counts.html, 2);
        assert_eq!(snapshot.counts.mermaid, 1);
        assert!(snapshot
            .docs
            .iter()
            .any(|doc| doc.relative_path == "baseline/README.md"));
        assert!(snapshot.docs.iter().any(|doc| doc.relative_path == "README.md"));
        assert!(!snapshot.tree.iter().any(|node| node.id == "README.md"));
        assert_eq!(
            find_tree_node(&snapshot.tree, "baseline/README.md").map(|node| node.title.as_str()),
            Some("README.md")
        );
        assert_eq!(
            find_tree_node(&snapshot.tree, "baseline/10-standards/rules.md")
                .map(|node| node.title.as_str()),
            Some("rules.md")
        );
        assert_eq!(
            find_tree_node(&snapshot.tree, "baseline/overview.html").map(|node| node.title.as_str()),
            Some("overview.html")
        );
        assert_eq!(
            snapshot
                .docs
                .iter()
                .find(|doc| doc.relative_path == "baseline/overview.html")
                .map(|doc| (doc.title.as_str(), doc.content_type.as_str(), doc.modified_at_unix_ms > 0)),
            Some(("Baseline Overview", "html", true))
        );
    }

    #[test]
    fn scans_from_child_directory_at_git_root() {
        let root = build_test_repo();
        let child = root.join("requirements/R001/tasks");
        let snapshot = scan_repo(child.to_string_lossy().to_string())
            .expect("child dir should resolve to git root");

        assert_eq!(snapshot.root_path, root.to_string_lossy().to_string());
        assert!(snapshot.docs.iter().any(|doc| doc.relative_path == "README.md"));
    }

    #[test]
    fn rejects_non_git_directory() {
        let root = build_temp_dir("non-git");
        fs::create_dir_all(&root).expect("non-git test dir should create");
        fs::write(root.join("README.md"), "# Not A Memory Repo\n").expect("README should write");

        let err = scan_repo(root.to_string_lossy().to_string())
            .expect_err("non-git dir should be rejected");

        assert!(err.contains("Git"));
    }

    #[test]
    fn pull_repo_rejects_non_git_directory() {
        let root = build_temp_dir("pull-non-git");
        fs::create_dir_all(&root).expect("non-git test dir should create");

        let err = pull_repo(root.to_string_lossy().to_string())
            .expect_err("non-git dir should be rejected before shelling out");

        assert!(err.contains("Git"));
    }

    #[test]
    fn combines_command_output_without_extra_blank_lines() {
        assert_eq!(
            command_output_text(b"Already up to date.\n", b""),
            "Already up to date."
        );
        assert_eq!(
            command_output_text(b"", b"fatal: no upstream\n"),
            "fatal: no upstream"
        );
        assert_eq!(
            command_output_text(b"stdout\n", b"stderr\n"),
            "stdout\nstderr"
        );
    }

    #[test]
    fn reads_baseline_readme_with_chain() {
        let root = build_test_repo();
        let repo_path = root.to_string_lossy().to_string();
        let doc = read_document(
            repo_path,
            root.join("baseline/README.md").to_string_lossy().to_string(),
        )
        .expect("baseline README should read");

        assert_eq!(doc.kind, "baseline");
        assert_eq!(doc.content_type, "markdown");
        assert!(doc.modified_at_unix_ms > 0);
        assert!(doc.content.contains("# "));
        assert_eq!(doc.read_chain.len(), 1);
    }

    #[test]
    fn reads_standalone_document_file() {
        let root = build_temp_dir("standalone");
        fs::create_dir_all(&root).expect("standalone test dir should create");
        let path = root.join("loose-note.md");
        fs::write(&path, "# Loose Note\n\nStandalone markdown.\n")
            .expect("standalone markdown should write");

        let doc = read_standalone_document(path.to_string_lossy().to_string())
            .expect("standalone document should read");

        assert_eq!(doc.title, "Loose Note");
        assert_eq!(doc.content_type, "markdown");
        assert!(doc.modified_at_unix_ms > 0);
        assert_eq!(doc.relative_path, "loose-note.md");
        assert!(doc.read_chain.is_empty());
    }

    #[test]
    fn reads_standalone_html_file() {
        let root = build_temp_dir("standalone-html");
        fs::create_dir_all(&root).expect("standalone test dir should create");
        let path = root.join("preview.html");
        fs::write(&path, "<!doctype html><title>Preview Doc</title><h1>Fallback</h1>\n")
            .expect("html file should write");

        let doc = read_standalone_document(path.to_string_lossy().to_string())
            .expect("standalone html should read");

        assert_eq!(doc.title, "Preview Doc");
        assert_eq!(doc.content_type, "html");
        assert!(doc.modified_at_unix_ms > 0);
        assert_eq!(doc.relative_path, "preview.html");
        assert!(doc.read_chain.is_empty());
    }

    #[test]
    fn extracts_html_h1_title_when_title_is_missing() {
        assert_eq!(
            extract_document_title("<html><body><h1>HTML Heading</h1></body></html>", "html")
                .as_deref(),
            Some("HTML Heading")
        );
    }

    #[test]
    fn rejects_standalone_unsupported_file() {
        let root = build_temp_dir("standalone-txt");
        fs::create_dir_all(&root).expect("standalone test dir should create");
        let path = root.join("note.txt");
        fs::write(&path, "not markdown").expect("text file should write");

        let err = read_standalone_document(path.to_string_lossy().to_string())
            .expect_err("unsupported file should be rejected");

        assert!(err.contains("Markdown 或 HTML"));
    }

    #[test]
    fn serializes_annotation_export_payload_with_camel_case_keys() {
        let payload = build_annotation_payload();
        let json = serde_json::to_string_pretty(&payload).expect("payload should serialize");

        assert!(json.contains("\"schemaVersion\""));
        assert!(json.contains("\"createdAtUnixMs\""));
        assert!(json.contains("\"repoPath\""));
        assert!(json.contains("\"coveredNodes\""));
        assert!(json.contains("\"sourceLines\""));
        assert!(json.contains("\"intersectionRatio\""));
    }

    #[test]
    fn builds_annotation_temp_dir_path_with_expected_name() {
        let base = PathBuf::from("/tmp");
        let path = annotation_temp_dir_path(&base, 12345);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("temp path should have file name");

        assert_eq!(path.parent(), Some(base.as_path()));
        assert!(file_name.starts_with("mem-view-annotations-"));
        assert!(file_name.ends_with("-12345"));
    }

    #[test]
    fn writes_annotation_bundle_with_readme_json_and_visual_evidence() {
        let payload = build_annotation_payload();

        let bundle = write_annotation_export(payload).expect("annotation bundle should write");
        let prompt = build_annotation_prompt(&bundle.readme_path.to_string_lossy());
        let readme = fs::read_to_string(&bundle.readme_path).expect("README should be readable");
        let json = fs::read_to_string(&bundle.annotations_path)
            .expect("annotation JSON should be readable");

        assert!(bundle.directory_path.starts_with(std::env::temp_dir()));
        assert_eq!(
            bundle
                .annotations_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("annotations.json")
        );
        assert_eq!(
            bundle.readme_path.file_name().and_then(|name| name.to_str()),
            Some("README.md")
        );
        assert!(bundle.directory_path.join("images").is_dir());
        assert!(readme.contains("note`：用户写下的标注意图原文"));
        assert!(readme.contains("coveredNodes`：memView 根据框选区域推断出的候选文档节点"));
        assert!(readme.contains("sourceLines`、`textExcerpt`、`headingPath`：优先用于定位"));
        assert!(readme.contains("截图只用于确认视觉上下文"));
        assert!(readme.contains("Mermaid 的源码修改仍以 Markdown 中的 Mermaid 代码块为准"));
        assert!(json.contains("需要补充边界条件"));
        assert!(json.contains("\"visualEvidence\""));
        assert!(json.contains("\"captureStatus\": \"unavailable\""));
        assert!(json.contains("\"captureError\""));
        assert!(json.contains("sourceLines"));
        assert!(json.contains("documents"));
        assert!(prompt.contains(&bundle.readme_path.to_string_lossy().to_string()));
        assert!(prompt.contains("annotations.json"));
        assert!(!prompt.contains("coveredNodes"));
        assert!(!prompt.contains("sourceLines"));
    }

    #[test]
    fn rejects_empty_annotation_note() {
        let mut payload = build_annotation_payload();
        payload.documents[0].annotations[0].note.clear();

        let err = validate_annotation_export(&payload)
            .expect_err("empty annotation note should be rejected");

        assert!(err.contains("备注"));
    }

    fn build_annotation_payload() -> AnnotationExportPayload {
        AnnotationExportPayload {
            schema_version: "memView.annotation.v1".to_string(),
            created_at_unix_ms: 123,
            app: "memView".to_string(),
            documents: vec![AnnotationDocument {
                path: "/tmp/spec.md".to_string(),
                relative_path: "spec.md".to_string(),
                repo_path: Some("/tmp".to_string()),
                title: "Spec".to_string(),
                kind: "requirement".to_string(),
                annotations: vec![AnnotationItem {
                    id: "ann-1".to_string(),
                    note: "需要补充边界条件".to_string(),
                    rect: AnnotationRect {
                        left: 10.0,
                        top: 20.0,
                        width: 120.0,
                        height: 48.0,
                        scroll_top: 0.0,
                        scroll_left: 0.0,
                        reader_width: 800.0,
                        reader_height: 600.0,
                    },
                    covered_nodes: vec![AnnotationCoveredNode {
                        node_id: "paragraph-1-2".to_string(),
                        node_type: "paragraph".to_string(),
                        source_lines: Some(AnnotationSourceLines { start: 3, end: 4 }),
                        heading_path: vec!["Spec".to_string()],
                        text_excerpt: "原始规格内容".to_string(),
                        intersection_ratio: 0.7,
                        is_primary: true,
                    }],
                    visual_evidence: Some(unavailable_visual_evidence(
                        None,
                        24.0,
                        "测试环境不生成截图",
                    )),
                }],
            }],
        }
    }

    fn build_test_repo() -> PathBuf {
        let root = build_temp_dir("test");

        fs::create_dir_all(root.join(".git")).expect("git metadata directory should create");
        fs::create_dir_all(root.join("baseline/10-standards"))
            .expect("test directories should create");
        fs::create_dir_all(root.join("requirements/R001/tasks/T001"))
            .expect("test directories should create");

        fs::write(root.join("README.md"), "# Test Memory\n").expect("root README should write");
        fs::write(
            root.join("baseline/README.md"),
            "# Baseline\n\n```mermaid\ngraph TD\n  A --> B\n```\n",
        )
        .expect("baseline README should write");
        fs::write(root.join("baseline/10-standards/rules.md"), "# Rules\n")
            .expect("rules doc should write");
        fs::write(
            root.join("baseline/overview.html"),
            "<!doctype html><title>Baseline Overview</title><h1>Baseline HTML</h1>\n",
        )
        .expect("baseline html should write");
        fs::write(
            root.join("requirements/R001/brief.htm"),
            "<!doctype html><h1>Requirement Brief</h1>\n",
        )
        .expect("requirement html should write");
        fs::write(root.join("requirements/R001/tasks/T001/README.md"), "# Task\n")
            .expect("task README should write");

        root.canonicalize()
            .expect("test repo should canonicalize after creation")
    }

    fn build_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mem-view-{}-{}-{}",
            label,
            std::process::id(),
            nonce
        ))
    }

    fn find_tree_node<'a>(nodes: &'a [TreeNode], id: &str) -> Option<&'a TreeNode> {
        for node in nodes {
            if node.id == id {
                return Some(node);
            }
            if let Some(child) = find_tree_node(&node.children, id) {
                return Some(child);
            }
        }
        None
    }
}
