use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct RepoSnapshot {
    root_path: String,
    tree: Vec<TreeNode>,
    docs: Vec<DocMeta>,
    counts: RepoCounts,
}

#[derive(Debug, Serialize)]
struct RepoCounts {
    markdown: usize,
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
    markdown: String,
    has_mermaid: bool,
    read_chain: Vec<ChainItem>,
}

#[derive(Debug, Serialize)]
struct ChainItem {
    label: String,
    path: String,
    title: String,
}

#[tauri::command]
fn scan_repo(repo_path: String) -> Result<RepoSnapshot, String> {
    let root = normalize_repo_path(&repo_path)?;
    let mut docs = scan_markdown_docs(&root)?;
    docs.sort_by(|a, b| sort_key(&a.relative_path).cmp(&sort_key(&b.relative_path)));

    let counts = RepoCounts {
        markdown: docs.len(),
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

    read_markdown_document(&requested, relative_path, read_chain)
}

#[tauri::command]
fn read_markdown_file(path: String) -> Result<Document, String> {
    let requested = PathBuf::from(path.trim());
    let requested = canonical_readable_markdown_file(&requested)?;
    let relative_path = requested
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown.md")
        .to_string();

    read_markdown_document(&requested, relative_path, Vec::new())
}

fn read_markdown_document(
    requested: &Path,
    relative_path: String,
    read_chain: Vec<ChainItem>,
) -> Result<Document, String> {
    let markdown = fs::read_to_string(&requested)
        .map_err(|err| format!("读取文档失败：{} ({})", requested.display(), err))?;
    let title = extract_title(&markdown).unwrap_or_else(|| fallback_title(&requested));
    let has_mermaid = markdown.contains("```mermaid");

    Ok(Document {
        id: relative_path.clone(),
        title,
        path: requested.to_string_lossy().to_string(),
        relative_path: relative_path.clone(),
        kind: classify_doc(&relative_path),
        markdown,
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

fn scan_markdown_docs(root: &Path) -> Result<Vec<DocMeta>, String> {
    let mut docs = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| entry.file_name().to_string_lossy() != ".git")
    {
        let entry = entry.map_err(|err| format!("扫描文件失败：{}", err))?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .map_err(|_| "路径解析失败".to_string())?
            .to_string_lossy()
            .to_string();
        let markdown = fs::read_to_string(path).unwrap_or_default();
        let title = extract_title(&markdown).unwrap_or_else(|| fallback_title(path));

        docs.push(DocMeta {
            id: relative_path.clone(),
            title,
            path: path.to_string_lossy().to_string(),
            kind: classify_doc(&relative_path),
            relative_path,
            has_mermaid: markdown.contains("```mermaid"),
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
    let markdown = fs::read_to_string(&path).unwrap_or_default();
    chain.push(ChainItem {
        label: label.to_string(),
        title: extract_title(&markdown).unwrap_or_else(|| fallback_title(&path)),
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

fn canonical_readable_markdown_file(requested: &Path) -> Result<PathBuf, String> {
    let canonical = requested
        .canonicalize()
        .map_err(|err| format!("Markdown 文件不存在或不可读：{} ({})", requested.display(), err))?;
    if !canonical.is_file() {
        return Err(format!("Markdown 路径不是文件：{}", canonical.display()));
    }
    let extension = canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("md") {
        return Err(format!("只能打开 Markdown 文件：{}", canonical.display()));
    }
    Ok(canonical)
}

fn extract_title(markdown: &str) -> Option<String> {
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
        .invoke_handler(tauri::generate_handler![scan_repo, read_document, read_markdown_file])
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
        assert_eq!(snapshot.counts.markdown, 4);
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
    fn reads_baseline_readme_with_chain() {
        let root = build_test_repo();
        let repo_path = root.to_string_lossy().to_string();
        let doc = read_document(
            repo_path,
            root.join("baseline/README.md").to_string_lossy().to_string(),
        )
        .expect("baseline README should read");

        assert_eq!(doc.kind, "baseline");
        assert!(doc.markdown.contains("# "));
        assert_eq!(doc.read_chain.len(), 1);
    }

    #[test]
    fn reads_standalone_markdown_file() {
        let root = build_temp_dir("standalone");
        fs::create_dir_all(&root).expect("standalone test dir should create");
        let path = root.join("loose-note.md");
        fs::write(&path, "# Loose Note\n\nStandalone markdown.\n")
            .expect("standalone markdown should write");

        let doc = read_markdown_file(path.to_string_lossy().to_string())
            .expect("standalone markdown should read");

        assert_eq!(doc.title, "Loose Note");
        assert_eq!(doc.relative_path, "loose-note.md");
        assert!(doc.read_chain.is_empty());
    }

    #[test]
    fn rejects_standalone_non_markdown_file() {
        let root = build_temp_dir("standalone-txt");
        fs::create_dir_all(&root).expect("standalone test dir should create");
        let path = root.join("note.txt");
        fs::write(&path, "not markdown").expect("text file should write");

        let err = read_markdown_file(path.to_string_lossy().to_string())
            .expect_err("non-markdown file should be rejected");

        assert!(err.contains("Markdown"));
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
