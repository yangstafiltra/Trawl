# Trawl

> A modal TUI web browser with image rendering and tiling tabs — built in Rust.

> 一个模态终端网页浏览器，支持行内图片和平铺标签——Rust 构建。

---

## Inspiration / 灵感来源

| What | Why |
|------|-----|
| [bilibili-tui](https://github.com/kanjieater/bilibili-tui) | Terminal Bilibili client with video grid and mpv playback — the core idea for Trawl's video experience |
| [lazygit](https://github.com/jesseduffield/lazygit) | Panel-based TUI layout design — inspired Trawl's LazyGit mode with sidebar + content split |
| [scroll](https://github.com/isene/scroll) | Rust terminal web browser with vim keys — proved a TUI browser is feasible in Rust |
| [ratatui](https://github.com/ratatui/ratatui) | The TUI framework that makes terminal applications pleasant to build |

Trawl started as an experiment: "what if we took bilibili-tui's video grid and mpv integration, embedded it in a browser-like tab/tiling system, and made the architecture reusable for any terminal tool?"

Trawl 的起点是一个实验：「如果把 bilibili-tui 的视频网格和 mpv 播放嵌入到浏览器风格的标签/平铺系统中，再把架构抽象成可复用的终端工具骨架，会怎样？」

---

## What It Is / 它是什么

Trawl is **not** a general-purpose web browser. It cannot render JavaScript, CSS layouts, or modern web applications. It is a **terminal-native media browser** optimized for:

Trawl **不是**通用网页浏览器。它无法渲染 JavaScript、CSS 布局或现代 Web 应用。它是一个针对以下场景优化的**终端原生媒体浏览器**：

- **Video site browsing** — dedicated card grid layout, thumbnail covers, inline images, mpv playback
- **Search result aggregation** — structured card view for Bing/Google/DuckDuckGo results
- **Multi-tab tiling** — slot-based tiling window manager with 5 modes (Auto/Vertical/Horizontal/Master/Single)
- **Quick info scanning** — text-heavy pages, documentation, articles
  
- **视频站点浏览** — 专用卡片网格布局、缩略图封面、行内图片、mpv 播放
- **搜索结果聚合** — 结构化卡片视图（Bing/Google/DuckDuckGo）
- **多标签平铺** — 基于 slot 的平铺窗口管理器，5 种模式
- **快速信息扫描** — 文本类页面、文档、文章

---

## Limitations / 局限性

| Limitation | Why |
|------------|-----|
| **No JavaScript** | No JS engine — most modern sites render as empty pages |
| **No CSS layout** | Only basic text styling (bold, color), no flex/grid |
| **HTTP/1.1 only** | Uses `ureq` (blocking), no HTTP/2 |
| **No downloads** | Binary content detected but download not implemented |
| **Early stage** | v0.1.0, no tests, AI-generated code quality |

| 局限性 | 原因 |
|--------|------|
| **无 JavaScript** | 无 JS 引擎——大多数现代页面渲染为空 |
| **无 CSS 布局** | 仅支持基础文本样式（粗体、颜色），无 flex/grid |
| **仅 HTTP/1.1** | 使用 `ureq`（阻塞），不支持 HTTP/2 |
| **无下载功能** | 二进制内容可检测但未实现下载 |
| **早期阶段** | v0.1.0，无测试，AI 生成代码质量 |

---

## Quick Start

```bash
git clone https://github.com/yangstafiltra/Trawl.git
cd Trawl
cargo build --release
```


---

## Key Bindings

| Key | Action |
|-----|--------|
| `j`/`k` | Scroll down/up |
| `d`/`u` | Half page down/up |
| `g`/`G` | Top/bottom |
| `v` | View mode (cursor navigation, Enter to mark range, `y` to yank) |
| `Enter` | Show links sidebar |
| `l` | Link mode (search result cards) |
| `/` | Search |
| `:` | Open URL |
| `t` | New tab |
| `x` | Close tab |
| `T`/`N` | Prev/next tab |
| `H`/`L` | Back/forward |
| `s` | Cycle search engine |
| `\` | Toggle layout (Chrome ↔ LazyGit) |
| `f` | Toggle focus frame |
| `?` | Help |
| `q` | Quit |

### Alt+Key (Tiling)

| Key | Action |
|-----|--------|
| `Alt+1-9` | Switch to slot |
| `Alt+Shift+1-9` | Move tab to slot |
| `Alt+h/j/k/l` | Focus tile |
| `Alt+Shift+h/j/k/l` | Swap tile |
| `Alt+v` | Vertical tiling |
| `Alt+b` | Horizontal tiling |
| `Alt+f` | Fullscreen (single) |
| `Alt+q` | Quit |

---

## Stack

| Layer | Crate |
|-------|-------|
| TUI | ratatui 0.30 + crossterm 0.29 |
| Images | ratatui-image 11 (Kitty/iTerm2/Sixel) |
| HTTP | ureq 3 |
| HTML | scraper 0.22 |
| JSON | serde + serde_json |
| Config | directories 6 |

---

## AI-Assisted Programming / AI 辅助编程

This project was entirely written through conversations with AI
本项目全部通过与 AI

---

## License

MIT
