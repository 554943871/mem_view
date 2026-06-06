# memView

[English](README.md) | [简体中文](README.zh-CN.md)

memView 是一个极简 macOS 本地应用，用来浏览并手动刷新本地 Markdown
记忆库。它适合处理包含大量 `.md` 文件和 Mermaid 图的记忆库，目标是快速、
安静地查看内容，而不是编辑内容。

memView 不会写死某个记忆库。首次启动时选择一个本地目录即可，选择结果会保存在本地，
下次启动会自动打开上次选择的记忆库。

## 下载

最新 macOS 打包版本已发布在 GitHub Releases：

[下载 memView v0.2.31](https://github.com/554943871/mem_view/releases/tag/v0.2.31)

直接下载 dmg：

[memView_0.2.31_arm64.dmg](https://github.com/554943871/mem_view/releases/download/v0.2.31/memView_0.2.31_arm64.dmg)

## 重要：macOS Gatekeeper 提示

当前公开构建是 macOS arm64 版本，使用 ad-hoc 签名，但还没有 Apple 公证。
要把它称为完全稳定的 macOS 正式分发版，仍然需要 Apple Developer ID 签名和公证。

如果下载后 macOS 提示“Apple 无法验证是否包含恶意软件”，请只在你确认信任本仓库和
下载的 Release 资产时继续。可以先对 dmg 移除 quarantine 标记：

```bash
xattr -d com.apple.quarantine /path/to/memView_0.2.31_arm64.dmg
open /path/to/memView_0.2.31_arm64.dmg
```

如果已经把应用复制到了 Applications 或其他目录，也可以对 app bundle 移除 quarantine：

```bash
xattr -dr com.apple.quarantine /path/to/memView.app
```

## 功能

- 本地 macOS 桌面应用，基于 Tauri 打包。
- 浏览 Markdown，并为 Git 记忆库提供显式的拉取并刷新动作。
- 拉取并刷新按钮会先执行 `git pull --ff-only`，再重新索引文件。
- 首次启动选择本地记忆库目录。
- 后续启动自动打开上次选择的记忆库。
- 扫描记忆库并展示文件树。
- 渲染 Markdown 内容和 Mermaid 图。
- 展示文件信息和简单阅读链。
- 支持按标题、路径、类型搜索。
- 支持中英双语 UI，可在中文和 English 之间切换。
- Mermaid 大图查看器：
  - 每个图右上角提供放大按钮
  - 纯白不透明背景
  - 打开后默认裁掉 Mermaid 内部空白并适配可视区域
  - 鼠标滚轮按指针位置缩放
  - 鼠标拖拽移动视图
  - 可复制图到系统图片剪贴板
  - 提供适配、放大、缩小、关闭控制

## 技术栈

| 部分 | 技术 | 提供的能力 |
| --- | --- | --- |
| 桌面壳 | Tauri 2 | 原生 macOS 应用打包和 Rust 命令桥接 |
| 后端 | Rust | 本地文件扫描、Markdown 读取、路径安全检查 |
| 前端 | Svelte + TypeScript | 极简阅读界面和交互状态 |
| 构建 | Vite | 前端开发服务和生产构建 |
| Markdown | markdown-it | Markdown 转 HTML |
| 图表 | Mermaid | Mermaid 图渲染 |

## 开发

安装依赖：

```bash
npm install
```

启动开发模式：

```bash
npm run dev
```

只构建前端：

```bash
npm run build:web
```

运行 Rust 测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

运行完整检查：

```bash
npm run check
```

构建 macOS app 和 dmg：

```bash
npm run build
```

只构建 app 和已签名的 updater 包：

```bash
npm run build:updater
```

生成 GitHub Release 在线更新元数据：

```bash
npm run release:latest-json
```

上传前校验 macOS Release 签名：

```bash
npm run verify:mac-release -- src-tauri/target/release/bundle/macos/memView.app
```

构建产物：

```text
src-tauri/target/release/bundle/macos/memView.app
src-tauri/target/release/bundle/dmg/memView_0.2.0_x64.dmg
dist-release/latest.json
dist-release/memView_<version>_arm64.dmg
dist-release/memView_<version>_arm64.app.tar.gz
```

如果本机 Node.js 运行时是 x64，Tauri 生成的 dmg 文件名可能仍带 `x64`。
以 `file` 检查 app 二进制为准；当前 Release 会在二进制为 arm64 时上传为
`arm64` 资产名。

在线更新使用 Tauri updater 和 GitHub Release 里的 `latest.json`，不需要额外
维护更新服务端。

## 记忆库路径

memView 打开你在应用里选择的记忆库目录。这个路径只保存在本地应用存储里，
不会写入记忆库。需要切换时，可以在左侧栏的记忆库选择区重新选择。

## 项目结构

```text
.
+-- src/                 # Svelte 前端
+-- src-tauri/           # Tauri 和 Rust 后端
+-- docs/                # 发布检查清单和打包说明
+-- package.json         # npm 脚本和前端依赖
+-- vite.config.ts       # Vite 配置
+-- README.md
+-- README.zh-CN.md
```

## 隐私和许可证

- 隐私说明：[PRIVACY.zh-CN.md](PRIVACY.zh-CN.md)
- License：[MIT](LICENSE)
- 发布检查清单：[docs/RELEASE.md](docs/RELEASE.md)

## MVP 说明

- 主要面向 macOS。
- 设计上以浏览为主；唯一会写入记忆库的路径是显式点击拉取并刷新。
- 记忆库路径由用户选择，并保存在本地。
- 它面向本地 Markdown 和 Mermaid 记忆库，不是通用笔记编辑器。
