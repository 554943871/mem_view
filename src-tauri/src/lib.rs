use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::time::Duration;
use std::time::UNIX_EPOCH;
use tauri::{Emitter, Manager};
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

#[derive(Default)]
struct PendingOpenFiles(Mutex<Vec<String>>);

#[derive(Debug, Serialize)]
struct RepoCounts {
    documents: usize,
    markdown: usize,
    html: usize,
    assets: usize,
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
    content_type: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note_position: Option<AnnotationNotePosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note_collapsed: Option<bool>,
    covered_nodes: Vec<AnnotationCoveredNode>,
    visual_evidence: Option<AnnotationVisualEvidence>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationNotePosition {
    left: f64,
    top: f64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture_method: Option<String>,
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
    archive_id: String,
    annotation_directory_path: String,
    readme_path: String,
    prompt: String,
    annotation_count: usize,
    screenshot_unavailable_count: usize,
    prompt_copied: bool,
    prompt_copy_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationArchiveSummary {
    archive_id: String,
    created_at_unix_ms: u64,
    annotation_directory_path: String,
    readme_path: String,
    document_path: String,
    document_title: String,
    document_relative_path: String,
    repo_path: Option<String>,
    annotation_count: usize,
    screenshot_unavailable_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationArchiveRecord {
    summary: AnnotationArchiveSummary,
    payload: AnnotationExportPayload,
    prompt: String,
}

#[allow(dead_code)]
struct AnnotationBundlePaths {
    archive_id: String,
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
#[cfg(target_os = "macos")]
const MACOS_SCREEN_CAPTURE_PERMISSION_ERROR: &str =
    "当前运行的 memView 进程没有屏幕录制权限；如果已授权 memView.app，请重启应用后重试，开发模式还需要授权启动它的 Terminal/Codex 进程";

#[tauri::command]
fn scan_repo(repo_path: String) -> Result<RepoSnapshot, String> {
    let root = normalize_repo_path(&repo_path)?;
    let mut docs = scan_documents(&root)?;
    docs.sort_by(|a, b| sort_key(&a.relative_path).cmp(&sort_key(&b.relative_path)));

    let counts = RepoCounts {
        documents: docs
            .iter()
            .filter(|doc| is_renderable_document_content_type(&doc.content_type))
            .count(),
        markdown: docs.iter().filter(|doc| doc.content_type == "markdown").count(),
        html: docs.iter().filter(|doc| doc.content_type == "html").count(),
        assets: docs.iter().filter(|doc| doc.content_type == "asset").count(),
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
    } else if is_missing_pull_upstream_message(&message) {
        Ok(GitPullResult {
            root_path: root.to_string_lossy().to_string(),
            message: "当前 Git 分支没有 upstream，已跳过 git pull --ff-only。".to_string(),
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

fn is_missing_pull_upstream_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("there is no tracking information for the current branch")
        || normalized.contains("no upstream configured for branch")
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
fn take_pending_open_files(
    state: tauri::State<'_, PendingOpenFiles>,
) -> Result<Vec<String>, String> {
    let mut pending = state
        .0
        .lock()
        .map_err(|_| "待打开文件队列已损坏".to_string())?;

    Ok(std::mem::take(&mut *pending))
}

#[tauri::command]
fn copy_image_to_clipboard(image: ClipboardImage) -> Result<(), String> {
    let bytes = general_purpose::STANDARD
        .decode(image.png_base64.trim())
        .map_err(|err| format!("图片数据解析失败：{}", err))?;

    copy_png_bytes_to_clipboard(&bytes)
}

fn copy_png_bytes_to_clipboard(bytes: &[u8]) -> Result<(), String> {
    let decoded = image::load_from_memory(&bytes)
        .map_err(|err| format!("图片解码失败：{}", err))?
        .to_rgba8();
    let width = decoded.width() as usize;
    let height = decoded.height() as usize;
    if width == 0 || height == 0 {
        return Err("图片尺寸无效".to_string());
    }

    match copy_rgba_image_to_clipboard(width, height, decoded.into_raw()) {
        Ok(()) => Ok(()),
        Err(err) => {
            #[cfg(target_os = "macos")]
            {
                eprintln!("Native image clipboard path failed: {}", err);
                copy_png_to_macos_clipboard(bytes)
                    .map_err(|fallback_err| format!("{}；macOS 兜底失败：{}", err, fallback_err))
            }

            #[cfg(not(target_os = "macos"))]
            {
                Err(err)
            }
        }
    }
}

fn copy_rgba_image_to_clipboard(width: usize, height: usize, bytes: Vec<u8>) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|err| format!("打开系统剪贴板失败：{}", err))?;
    clipboard
        .set_image(ImageData {
            width,
            height,
            bytes: Cow::Owned(bytes),
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
        return copy_png_bytes_to_clipboard(&png);
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("当前平台暂不支持直接复制 SVG 图".to_string())
    }
}

#[tauri::command]
fn finish_annotation_export(
    app: tauri::AppHandle,
    payload: AnnotationExportPayload,
) -> Result<AnnotationExportResult, String> {
    let archive_root = annotation_archive_root(&app)?;
    let bundle_paths = write_annotation_export(payload, &archive_root)?;
    let readme_path_string = bundle_paths.readme_path.to_string_lossy().to_string();
    let archived_payload = read_annotation_payload_from_path(&bundle_paths.annotations_path)?;
    let stats = annotation_export_stats(&archived_payload);
    let prompt = build_annotation_prompt(&readme_path_string);
    let prompt_copy_error = copy_text_to_clipboard(&prompt).err();

    Ok(AnnotationExportResult {
        archive_id: bundle_paths.archive_id,
        annotation_directory_path: bundle_paths.directory_path.to_string_lossy().to_string(),
        readme_path: readme_path_string,
        prompt,
        annotation_count: stats.annotation_count,
        screenshot_unavailable_count: stats.screenshot_unavailable_count,
        prompt_copied: prompt_copy_error.is_none(),
        prompt_copy_error,
    })
}

#[tauri::command]
fn list_annotation_archives(
    app: tauri::AppHandle,
) -> Result<Vec<AnnotationArchiveSummary>, String> {
    list_annotation_archives_from_root(&annotation_archive_root(&app)?)
}

#[tauri::command]
fn read_annotation_archive(
    app: tauri::AppHandle,
    archive_id: String,
) -> Result<AnnotationArchiveRecord, String> {
    read_annotation_archive_from_root(&annotation_archive_root(&app)?, &archive_id)
}

#[tauri::command]
fn copy_annotation_archive_prompt(
    app: tauri::AppHandle,
    archive_id: String,
) -> Result<String, String> {
    let record = read_annotation_archive_from_root(&annotation_archive_root(&app)?, &archive_id)?;
    copy_text_to_clipboard(&record.prompt)?;
    Ok(record.prompt)
}

#[tauri::command]
fn capture_annotation_screenshot(capture_rect: AnnotationCaptureRect) -> Result<String, String> {
    let screenshot_path = annotation_capture_temp_file_path(&std::env::temp_dir());
    capture_screen_region_to_png(&capture_rect, &screenshot_path)?;
    Ok(screenshot_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn capture_annotation_webview_snapshot(
    window: tauri::WebviewWindow,
    capture_rect: AnnotationCaptureRect,
) -> Result<String, String> {
    let screenshot_path = annotation_capture_temp_file_path(&std::env::temp_dir());
    capture_webview_region_to_png(&window, &capture_rect, &screenshot_path).await?;
    Ok(screenshot_path.to_string_lossy().to_string())
}

fn write_annotation_export(
    mut payload: AnnotationExportPayload,
    archive_root: &Path,
) -> Result<AnnotationBundlePaths, String> {
    validate_annotation_export(&payload)?;
    let directory_path = annotation_archive_dir_path(archive_root, unix_timestamp_millis());
    let archive_id = directory_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "无法生成标注归档 ID".to_string())?
        .to_string();
    let images_path = directory_path.join("images");
    fs::create_dir_all(&images_path)
        .map_err(|err| format!("创建标注归档目录失败：{} ({})", images_path.display(), err))?;

    finalize_annotation_visual_evidence(&mut payload, &images_path);

    let annotations_path = directory_path.join("annotations.json");
    write_annotation_export_to_path(&payload, &annotations_path)?;

    let readme_path = directory_path.join("README.md");
    let readme = build_annotation_readme(&payload, &annotations_path, &images_path);
    fs::write(&readme_path, readme)
        .map_err(|err| format!("写入标注说明文件失败：{} ({})", readme_path.display(), err))?;

    Ok(AnnotationBundlePaths {
        archive_id,
        directory_path,
        annotations_path,
        readme_path,
    })
}

fn annotation_archive_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("annotation-archives"))
        .map_err(|err| format!("获取标注归档目录失败：{}", err))
}

fn list_annotation_archives_from_root(root: &Path) -> Result<Vec<AnnotationArchiveSummary>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(root)
        .map_err(|err| format!("读取标注归档目录失败：{} ({})", root.display(), err))?;
    let mut summaries = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("读取标注归档项失败：{}", err))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(summary) = annotation_archive_summary_from_dir(&path) {
            summaries.push(summary);
        }
    }
    summaries.sort_by(|a, b| b.created_at_unix_ms.cmp(&a.created_at_unix_ms));
    Ok(summaries)
}

fn read_annotation_archive_from_root(
    root: &Path,
    archive_id: &str,
) -> Result<AnnotationArchiveRecord, String> {
    validate_annotation_archive_id(archive_id)?;
    let directory_path = root.join(archive_id);
    if !directory_path.is_dir() {
        return Err(format!("标注归档不存在：{}", archive_id));
    }
    let payload = read_annotation_payload_from_path(&directory_path.join("annotations.json"))?;
    let summary = annotation_archive_summary_from_payload(&directory_path, &payload)?;
    let prompt = build_annotation_prompt(&summary.readme_path);

    Ok(AnnotationArchiveRecord {
        summary,
        payload,
        prompt,
    })
}

fn annotation_archive_summary_from_dir(
    directory_path: &Path,
) -> Result<AnnotationArchiveSummary, String> {
    let payload = read_annotation_payload_from_path(&directory_path.join("annotations.json"))?;
    annotation_archive_summary_from_payload(directory_path, &payload)
}

fn annotation_archive_summary_from_payload(
    directory_path: &Path,
    payload: &AnnotationExportPayload,
) -> Result<AnnotationArchiveSummary, String> {
    let archive_id = directory_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("标注归档目录无效：{}", directory_path.display()))?
        .to_string();
    let document = payload
        .documents
        .first()
        .ok_or_else(|| "标注归档没有文档".to_string())?;
    let stats = annotation_export_stats(&payload);

    Ok(AnnotationArchiveSummary {
        archive_id,
        created_at_unix_ms: payload.created_at_unix_ms,
        annotation_directory_path: directory_path.to_string_lossy().to_string(),
        readme_path: directory_path.join("README.md").to_string_lossy().to_string(),
        document_path: document.path.clone(),
        document_title: document.title.clone(),
        document_relative_path: document.relative_path.clone(),
        repo_path: document.repo_path.clone(),
        annotation_count: stats.annotation_count,
        screenshot_unavailable_count: stats.screenshot_unavailable_count,
    })
}

fn read_annotation_payload_from_path(path: &Path) -> Result<AnnotationExportPayload, String> {
    let json = fs::read_to_string(path)
        .map_err(|err| format!("读取标注归档 JSON 失败：{} ({})", path.display(), err))?;
    serde_json::from_str(&json)
        .map_err(|err| format!("解析标注归档 JSON 失败：{} ({})", path.display(), err))
}

struct AnnotationExportStats {
    annotation_count: usize,
    screenshot_unavailable_count: usize,
}

fn annotation_export_stats(payload: &AnnotationExportPayload) -> AnnotationExportStats {
    let mut annotation_count = 0usize;
    let mut screenshot_unavailable_count = 0usize;
    for annotation in payload
        .documents
        .iter()
        .flat_map(|document| document.annotations.iter())
    {
        annotation_count += 1;
        if !matches!(
            annotation.visual_evidence.as_ref().map(|evidence| &evidence.capture_status),
            Some(AnnotationCaptureStatus::Captured)
        ) {
            screenshot_unavailable_count += 1;
        }
    }

    AnnotationExportStats {
        annotation_count,
        screenshot_unavailable_count,
    }
}

fn validate_annotation_archive_id(archive_id: &str) -> Result<(), String> {
    if archive_id.is_empty()
        || archive_id
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
    {
        return Err("标注归档 ID 无效".to_string());
    }
    Ok(())
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
        let supplied_screenshot_path = evidence
            .screenshot_path
            .clone()
            .filter(|_| matches!(&evidence.capture_status, AnnotationCaptureStatus::Captured));
        evidence.screenshot_path = None;
        evidence.capture_status = AnnotationCaptureStatus::Unavailable;
        let file_name = annotation_image_file_name(&annotation.id, index);
        let screenshot_path = images_path.join(file_name);

        if let Some(source_path) = supplied_screenshot_path {
            match copy_annotation_screenshot_to_bundle(&source_path, &screenshot_path) {
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
            if let Some(capture_rect) = evidence.capture_rect.clone() {
                match capture_screen_region_to_png(&capture_rect, &screenshot_path) {
                    Ok(()) => {
                        evidence.screenshot_path =
                            Some(screenshot_path.to_string_lossy().to_string());
                        evidence.capture_status = AnnotationCaptureStatus::Captured;
                        evidence.capture_error = None;
                    }
                    Err(err) => {
                        evidence.capture_error = Some(err);
                    }
                }
            } else {
                evidence.capture_error =
                    Some("标注区域不在当前可见 reader 视口内，未生成截图".to_string());
            }
        }

        annotation.visual_evidence = Some(evidence);
        index += 1;
    }
}

fn copy_annotation_screenshot_to_bundle(
    source_path: &str,
    destination_path: &Path,
) -> Result<(), String> {
    let source_path = PathBuf::from(source_path);
    if !source_path.is_file() {
        return Err(format!("截图临时文件不存在：{}", source_path.display()));
    }

    fs::copy(&source_path, destination_path)
        .map(|_| ())
        .map_err(|err| {
            format!(
                "复制标注截图失败：{} -> {} ({})",
                source_path.display(),
                destination_path.display(),
                err
            )
        })
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
        capture_method: None,
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

fn annotation_capture_temp_file_path(base: &Path) -> PathBuf {
    let process_id = std::process::id();
    for attempt in 0..1000 {
        let file_name = format!(
            "mem-view-annotation-shot-{}-{}-{}.png",
            process_id,
            unix_timestamp_millis(),
            attempt
        );
        let candidate = base.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    base.join(format!(
        "mem-view-annotation-shot-{}-{}.png",
        process_id,
        unix_timestamp_millis()
    ))
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
- `annotations[].visualEvidence`：截图辅助信息。`screenshotPath` 指向局部 PNG；`captureMethod` 标识截图来源；`captureRect` 是生成截图时使用的 WebView 坐标；`captureStatus` 为 `captured` 才表示截图可用。

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

fn annotation_archive_dir_path(base: &Path, timestamp_millis: u128) -> PathBuf {
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
async fn capture_webview_region_to_png(
    window: &tauri::WebviewWindow,
    rect: &AnnotationCaptureRect,
    path: &Path,
) -> Result<(), String> {
    let (x, y, width, height) = normalized_capture_rect(rect)?;
    let capture_rect = AnnotationCaptureRect {
        x,
        y,
        width,
        height,
    };
    let (sender, mut receiver) = tauri::async_runtime::channel(1);

    window
        .with_webview(move |webview| unsafe {
            request_webview_snapshot_png(webview.inner(), capture_rect, sender);
        })
        .map_err(|err| format!("获取 WebView 失败：{}", err))?;

    let bytes = tokio::time::timeout(Duration::from_secs(8), receiver.recv())
        .await
        .map_err(|_| "WebView 标注截图超时".to_string())?
        .ok_or_else(|| "WebView 标注截图中断".to_string())??;
    fs::write(path, bytes).map_err(|err| format!("写入截图失败：{} ({})", path.display(), err))
}

#[cfg(not(target_os = "macos"))]
async fn capture_webview_region_to_png(
    _window: &tauri::WebviewWindow,
    _rect: &AnnotationCaptureRect,
    _path: &Path,
) -> Result<(), String> {
    Err("当前平台暂不支持 WebView 标注截图".to_string())
}

#[cfg(target_os = "macos")]
unsafe fn request_webview_snapshot_png(
    webview: *mut std::ffi::c_void,
    rect: AnnotationCaptureRect,
    sender: tauri::async_runtime::Sender<Result<Vec<u8>, String>>,
) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSNumber;
    use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};

    let view = match webview.cast::<WKWebView>().as_ref() {
        Some(view) => view,
        None => {
            let _ = sender.try_send(Err("WebView 标注截图失败：WKWebView 句柄为空".to_string()));
            return;
        }
    };
    let mtm = MainThreadMarker::new_unchecked();
    let configuration = WKSnapshotConfiguration::new(mtm);
    let snapshot_rect = CGRect::new(
        CGPoint::new(rect.x, rect.y),
        CGSize::new(rect.width, rect.height),
    );
    configuration.setRect(snapshot_rect);
    configuration.setSnapshotWidth(Some(&NSNumber::new_f64(rect.width)));
    configuration.setAfterScreenUpdates(true);

    let completion = RcBlock::new(
        move |image: *mut objc2_app_kit::NSImage, error: *mut objc2_foundation::NSError| {
            let result = unsafe { webview_snapshot_png_bytes(image, error) };
            let _ = sender.try_send(result);
        },
    );
    view.takeSnapshotWithConfiguration_completionHandler(Some(&configuration), &completion);
}

#[cfg(target_os = "macos")]
unsafe fn webview_snapshot_png_bytes(
    image: *mut objc2_app_kit::NSImage,
    error: *mut objc2_foundation::NSError,
) -> Result<Vec<u8>, String> {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey};
    use objc2_foundation::NSDictionary;

    if !error.is_null() {
        return Err("WebKit 生成标注截图失败".to_string());
    }
    let image = image
        .as_ref()
        .ok_or_else(|| "WebKit 生成标注截图失败：没有返回图像".to_string())?;
    let properties = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::dictionary();
    let tiff_data = image
        .TIFFRepresentation()
        .ok_or_else(|| "WebKit 生成标注截图失败：无法读取图像数据".to_string())?;
    let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff_data)
        .ok_or_else(|| "WebKit 生成标注截图失败：无法读取 bitmap 图像".to_string())?;
    let data = bitmap_rep
        .representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
        .ok_or_else(|| "WebKit 生成标注截图失败：无法编码 PNG".to_string())?;
    let bytes = data.to_vec();
    if bytes.is_empty() {
        return Err("WebKit 生成标注截图失败：PNG 数据为空".to_string());
    }
    Ok(bytes)
}

#[cfg(target_os = "macos")]
fn capture_screen_region_to_png(rect: &AnnotationCaptureRect, path: &Path) -> Result<(), String> {
    let preflight_granted = macos_screen_capture_access_granted();
    let image = capture_screen_region_with_core_graphics(rect).map_err(|err| {
        if preflight_granted {
            err
        } else {
            format!("{}；{}", MACOS_SCREEN_CAPTURE_PERMISSION_ERROR, err)
        }
    })?;
    match write_cg_image_to_png(&image, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(path);
            Err(err)
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn capture_screen_region_to_png(_rect: &AnnotationCaptureRect, _path: &Path) -> Result<(), String> {
    Err("当前平台暂不支持自动截图".to_string())
}

#[cfg(target_os = "macos")]
fn normalized_capture_rect(rect: &AnnotationCaptureRect) -> Result<(f64, f64, f64, f64), String> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width < 1.0
        || rect.height < 1.0
    {
        return Err("截图区域无效".to_string());
    }

    Ok((
        rect.x.round(),
        rect.y.round(),
        rect.width.round().max(1.0),
        rect.height.round().max(1.0),
    ))
}

#[cfg(target_os = "macos")]
fn capture_screen_region_with_core_graphics(
    rect: &AnnotationCaptureRect,
) -> Result<core_graphics::image::CGImage, String> {
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        create_image, kCGNullWindowID, kCGWindowImageBestResolution,
        kCGWindowListOptionOnScreenOnly,
    };

    let (x, y, width, height) = normalized_capture_rect(rect)?;
    let bounds = CGRect::new(&CGPoint::new(x, y), &CGSize::new(width, height));

    create_image(
        bounds,
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageBestResolution,
    )
    .ok_or_else(|| "系统截图失败：CoreGraphics 没有返回图像，可能缺少屏幕录制权限".to_string())
}

#[cfg(target_os = "macos")]
fn write_cg_image_to_png(image: &core_graphics::image::CGImage, path: &Path) -> Result<(), String> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err("系统截图失败：返回了空图像".to_string());
    }
    if image.bits_per_component() != 8 || image.bits_per_pixel() != 32 {
        return Err(format!(
            "系统截图失败：不支持的图像格式（{} bits/component, {} bits/pixel）",
            image.bits_per_component(),
            image.bits_per_pixel()
        ));
    }

    let bytes_per_row = image.bytes_per_row();
    let expected_len = bytes_per_row
        .checked_mul(height)
        .ok_or_else(|| "系统截图失败：图像数据尺寸溢出".to_string())?;
    let data = image.data();
    let bytes = data.bytes();
    if bytes.len() < expected_len {
        return Err("系统截图失败：图像数据不完整".to_string());
    }

    let mut rgba = vec![0; width * height * 4];
    for y in 0..height {
        let source_row = y * bytes_per_row;
        let target_row = y * width * 4;
        for x in 0..width {
            let source = source_row + x * 4;
            let target = target_row + x * 4;
            rgba[target] = bytes[source + 2];
            rgba[target + 1] = bytes[source + 1];
            rgba[target + 2] = bytes[source];
            rgba[target + 3] = bytes[source + 3];
        }
    }

    let buffer = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
        width as u32,
        height as u32,
        rgba,
    )
    .ok_or_else(|| "系统截图失败：无法组装 PNG 图像".to_string())?;
    buffer
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|err| format!("写入截图失败：{} ({})", path.display(), err))
}

#[cfg(target_os = "macos")]
fn macos_screen_capture_access_granted() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
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
        .filter_entry(|entry| !is_hidden_directory_entry(entry))
    {
        let entry = entry.map_err(|err| format!("扫描文件失败：{}", err))?;
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(content_type) = indexed_file_content_type(path) else {
            continue;
        };

        let relative_path = path
            .strip_prefix(root)
            .map_err(|_| "路径解析失败".to_string())?
            .to_string_lossy()
            .to_string();
        let is_renderable = is_renderable_document_content_type(content_type);
        let content = if is_renderable {
            fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };
        let title = if is_renderable {
            extract_document_title(&content, content_type).unwrap_or_else(|| fallback_title(path))
        } else {
            file_name_title(path)
        };

        docs.push(DocMeta {
            id: relative_path.clone(),
            title,
            path: path.to_string_lossy().to_string(),
            kind: classify_indexed_file(&relative_path, content_type),
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
            .filter(|entry| entry.path().is_dir() && !is_dot_prefixed_name(&entry.file_name()))
            .count(),
        Err(_) => 0,
    }
}

fn is_hidden_directory_entry(entry: &walkdir::DirEntry) -> bool {
    entry.depth() > 0 && entry.file_type().is_dir() && is_dot_prefixed_name(entry.file_name())
}

fn is_dot_prefixed_name(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .map(|value| value.starts_with('.'))
        .unwrap_or(false)
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
    content_type: Option<String>,
    children: BTreeMap<String, MutableNode>,
}

impl MutableNode {
    fn new(id: &str, title: &str, kind: &str, path: Option<String>) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            kind: kind.to_string(),
            path,
            content_type: None,
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
                MutableNode {
                    id: doc.id.clone(),
                    title: tree_doc_title(doc),
                    kind: doc.kind.clone(),
                    path: Some(doc.path.clone()),
                    content_type: Some(doc.content_type.clone()),
                    children: BTreeMap::new(),
                },
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
            content_type: self.content_type,
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

fn system_open_document_paths_from_args(args: impl IntoIterator<Item = OsString>) -> Vec<String> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    args.into_iter()
        .filter_map(|arg| system_open_document_path_from_arg(arg, &current_dir))
        .collect()
}

fn system_open_document_path_from_arg(arg: OsString, current_dir: &Path) -> Option<String> {
    let value = arg.to_string_lossy();
    if value.is_empty() || value.starts_with("-psn_") || value.starts_with("--") {
        return None;
    }

    let path = PathBuf::from(&arg);
    let path = if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    };
    system_open_document_path(path)
}

fn system_open_document_path(path: PathBuf) -> Option<String> {
    let canonical = canonical_readable_document_file(&path).ok()?;
    Some(canonical.to_string_lossy().to_string())
}

fn system_open_document_paths_from_urls(urls: Vec<tauri::Url>) -> Vec<String> {
    urls.into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .filter_map(system_open_document_path)
        .collect()
}

fn queue_system_open_files(app: &tauri::AppHandle, paths: Vec<String>) {
    let paths = unique_paths(paths);
    if paths.is_empty() {
        return;
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }

    match app.state::<PendingOpenFiles>().0.lock() {
        Ok(mut pending) => {
            pending.extend(paths.clone());
            *pending = unique_paths(std::mem::take(&mut *pending));
        }
        Err(_) => {
            eprintln!("memView open-file queue is poisoned");
        }
    }

    if let Err(err) = app.emit("mem-view-open-files", paths) {
        eprintln!("failed to emit mem-view-open-files: {}", err);
    }
}

fn unique_paths(paths: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for path in paths {
        if !path.trim().is_empty() && !unique.iter().any(|item| item == &path) {
            unique.push(path);
        }
    }
    unique
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

fn indexed_file_content_type(path: &Path) -> Option<&'static str> {
    document_content_type(path).or_else(|| asset_content_type(path))
}

fn asset_content_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "csv" | "tsv"
        | "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tif" | "tiff"
        | "heic" | "psd" | "ai" | "sketch" | "fig" | "xd" | "zip" | "rar" | "7z" | "mp4"
        | "mov" | "m4v" | "mp3" | "wav" => Some("asset"),
        _ => None,
    }
}

fn is_renderable_document_content_type(content_type: &str) -> bool {
    matches!(content_type, "markdown" | "html")
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

fn file_name_title(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback_title(path))
}

fn classify_indexed_file(relative_path: &str, content_type: &str) -> String {
    if content_type == "asset" {
        return "asset".to_string();
    }

    classify_doc(relative_path)
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
        .manage(PendingOpenFiles::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let initial_paths = system_open_document_paths_from_args(std::env::args_os().skip(1));
            queue_system_open_files(app.handle(), initial_paths);

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
            list_annotation_archives,
            read_annotation_archive,
            copy_annotation_archive_prompt,
            capture_annotation_screenshot,
            capture_annotation_webview_snapshot,
            copy_svg_to_clipboard,
            copy_image_to_clipboard,
            take_pending_open_files
        ])
        .build(tauri::generate_context!())
        .expect("error while building memView")
        .run(|app_handle, event| {
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = event {
                queue_system_open_files(app_handle, system_open_document_paths_from_urls(urls));
            }
        });
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
        assert_eq!(snapshot.counts.assets, 0);
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
    fn scans_source_material_attachments_as_assets() {
        let root = build_test_repo();
        let attachment_dir = root.join(
            "requirements/mvp/raw-sources/_attachments/source-materials/小程序开发-2026-05-10",
        );
        fs::create_dir_all(&attachment_dir).expect("attachment directory should create");
        fs::write(attachment_dir.join("C001.xlsx"), b"sheet")
            .expect("spreadsheet asset should write");
        fs::write(attachment_dir.join("详情页上传.png"), b"png")
            .expect("image asset should write");
        fs::write(attachment_dir.join("网站设计.psd"), b"psd")
            .expect("design asset should write");

        let snapshot =
            scan_repo(root.to_string_lossy().to_string()).expect("test mem repo should scan");

        assert_eq!(snapshot.counts.documents, 6);
        assert_eq!(snapshot.counts.assets, 3);
        assert!(snapshot.docs.iter().any(|doc| {
            doc.relative_path
                == "requirements/mvp/raw-sources/_attachments/source-materials/小程序开发-2026-05-10/C001.xlsx"
                && doc.content_type == "asset"
                && doc.kind == "asset"
                && doc.title == "C001.xlsx"
        }));
        assert_eq!(
            find_tree_node(
                &snapshot.tree,
                "requirements/mvp/raw-sources/_attachments/source-materials/小程序开发-2026-05-10/C001.xlsx",
            )
            .and_then(|node| node.content_type.as_deref()),
            Some("asset")
        );
    }

    #[test]
    fn skips_dot_prefixed_directories() {
        let root = build_test_repo();
        fs::create_dir_all(root.join(".codex/skills")).expect("hidden codex dir should create");
        fs::create_dir_all(root.join(".pytest_cache")).expect("hidden pytest dir should create");
        fs::create_dir_all(root.join(".venv/docs")).expect("hidden venv dir should create");
        fs::create_dir_all(root.join("docs/.drafts")).expect("nested hidden dir should create");
        fs::create_dir_all(root.join("requirements/.scratch")).expect("hidden requirement dir should create");
        fs::write(root.join(".codex/skills/README.md"), "# Hidden Codex\n")
            .expect("hidden codex doc should write");
        fs::write(root.join(".pytest_cache/cache.html"), "<h1>Hidden Cache</h1>\n")
            .expect("hidden pytest doc should write");
        fs::write(root.join(".venv/docs/runtime.md"), "# Hidden Runtime\n")
            .expect("hidden venv doc should write");
        fs::write(root.join("docs/.drafts/idea.md"), "# Hidden Draft\n")
            .expect("hidden nested doc should write");
        fs::write(root.join("requirements/.scratch/README.md"), "# Hidden Requirement\n")
            .expect("hidden requirement doc should write");

        let snapshot =
            scan_repo(root.to_string_lossy().to_string()).expect("test mem repo should scan");

        assert_eq!(snapshot.counts.documents, 6);
        assert_eq!(snapshot.counts.requirements, 1);
        assert!(!snapshot
            .docs
            .iter()
            .any(|doc| doc.relative_path.split('/').any(|part| part.starts_with('.'))));
        assert!(find_tree_node(&snapshot.tree, ".codex").is_none());
        assert!(find_tree_node(&snapshot.tree, ".pytest_cache").is_none());
        assert!(find_tree_node(&snapshot.tree, ".venv").is_none());
        assert!(find_tree_node(&snapshot.tree, "docs/.drafts").is_none());
        assert!(find_tree_node(&snapshot.tree, "requirements/.scratch").is_none());
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
    fn pull_repo_skips_branch_without_tracking_information() {
        let root = build_git_repo_without_upstream("pull-no-upstream");

        let result = pull_repo(root.to_string_lossy().to_string())
            .expect("local branch without upstream should not block repository reading");

        assert_eq!(result.root_path, root.to_string_lossy());
        assert!(result.message.contains("跳过"));
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
    fn resolves_system_open_document_paths_from_args() {
        let root = build_temp_dir("system-open-args");
        fs::create_dir_all(&root).expect("system open test dir should create");
        let markdown_path = root.join("direct.md");
        let text_path = root.join("note.txt");
        fs::write(&markdown_path, "# Direct\n").expect("markdown file should write");
        fs::write(&text_path, "not markdown").expect("text file should write");

        assert_eq!(
            system_open_document_path_from_arg(OsString::from("direct.md"), &root),
            Some(
                markdown_path
                    .canonicalize()
                    .expect("markdown path should canonicalize")
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert_eq!(
            system_open_document_paths_from_args(vec![
                OsString::from("-psn_0_123"),
                markdown_path.clone().into_os_string(),
                text_path.into_os_string(),
                OsString::from("--flag")
            ]),
            vec![markdown_path
                .canonicalize()
                .expect("markdown path should canonicalize")
                .to_string_lossy()
                .to_string()]
        );
    }

    #[test]
    fn resolves_system_open_document_paths_from_file_urls() {
        let root = build_temp_dir("system-open-urls");
        fs::create_dir_all(&root).expect("system open test dir should create");
        let path = root.join("direct.html");
        fs::write(&path, "<!doctype html><title>Direct</title>\n")
            .expect("html file should write");
        let canonical = path
            .canonicalize()
            .expect("html path should canonicalize")
            .to_string_lossy()
            .to_string();
        let url = tauri::Url::from_file_path(&path).expect("file URL should build");

        assert_eq!(system_open_document_paths_from_urls(vec![url]), vec![canonical]);
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
    fn builds_annotation_archive_dir_path_with_expected_name() {
        let base = PathBuf::from("/tmp");
        let path = annotation_archive_dir_path(&base, 12345);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("archive path should have file name");

        assert_eq!(path.parent(), Some(base.as_path()));
        assert!(file_name.starts_with("mem-view-annotations-"));
        assert!(file_name.ends_with("-12345"));
    }

    #[test]
    fn writes_annotation_bundle_with_readme_json_and_visual_evidence() {
        let payload = build_annotation_payload();
        let archive_root = build_temp_dir("annotation-archive");

        let bundle = write_annotation_export(payload, &archive_root).expect("annotation bundle should write");
        let prompt = build_annotation_prompt(&bundle.readme_path.to_string_lossy());
        let readme = fs::read_to_string(&bundle.readme_path).expect("README should be readable");
        let json = fs::read_to_string(&bundle.annotations_path)
            .expect("annotation JSON should be readable");

        assert!(bundle.directory_path.starts_with(&archive_root));
        assert!(bundle.archive_id.starts_with("mem-view-annotations-"));
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
    fn lists_and_reads_annotation_archives_from_root() {
        let payload = build_annotation_payload();
        let archive_root = build_temp_dir("annotation-archive-list");

        let bundle = write_annotation_export(payload, &archive_root).expect("annotation bundle should write");
        let summaries =
            list_annotation_archives_from_root(&archive_root).expect("annotation archives should list");
        let record = read_annotation_archive_from_root(&archive_root, &bundle.archive_id)
            .expect("annotation archive should read");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].archive_id, bundle.archive_id);
        assert_eq!(summaries[0].annotation_count, 1);
        assert_eq!(summaries[0].screenshot_unavailable_count, 1);
        assert_eq!(record.summary.archive_id, summaries[0].archive_id);
        assert_eq!(record.payload.documents[0].annotations[0].note, "需要补充边界条件");
        assert!(record.prompt.contains(&record.summary.readme_path));
    }

    #[test]
    fn writes_annotation_bundle_with_supplied_screenshot_file() {
        let root = build_temp_dir("annotation-screenshot");
        let archive_root = build_temp_dir("annotation-screenshot-archive");
        fs::create_dir_all(&root).expect("screenshot temp dir should create");
        let source_path = root.join("source.png");
        fs::write(&source_path, b"png").expect("source screenshot should write");

        let mut payload = build_annotation_payload();
        let evidence = payload.documents[0].annotations[0]
            .visual_evidence
            .as_mut()
            .expect("test payload should include visual evidence");
        evidence.screenshot_path = Some(source_path.to_string_lossy().to_string());
        evidence.capture_status = AnnotationCaptureStatus::Captured;
        evidence.capture_error = None;

        let bundle = write_annotation_export(payload, &archive_root).expect("annotation bundle should write");
        let bundled_screenshot = bundle.directory_path.join("images").join("ann-1.png");
        let json = fs::read_to_string(&bundle.annotations_path)
            .expect("annotation JSON should be readable");

        assert_eq!(
            fs::read(&bundled_screenshot).expect("bundled screenshot should be readable"),
            b"png"
        );
        assert!(json.contains("\"captureStatus\": \"captured\""));
        assert!(json.contains(&bundled_screenshot.to_string_lossy().to_string()));
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
                    note_position: Some(AnnotationNotePosition {
                        left: 160.0,
                        top: 20.0,
                    }),
                    note_collapsed: Some(false),
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

    fn build_git_repo_without_upstream(label: &str) -> PathBuf {
        let root = build_temp_dir(label);
        fs::create_dir_all(&root).expect("git test dir should create");
        run_git_command(&root, &["init", "-q"]);
        root.canonicalize()
            .expect("git test repo should canonicalize after init")
    }

    fn run_git_command(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("git command should run");
        assert!(
            output.status.success(),
            "git {:?} failed:\n{}",
            args,
            command_output_text(&output.stdout, &output.stderr)
        );
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
