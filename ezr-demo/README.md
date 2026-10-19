# ezr-tui-demo（ezr-demo）

swarm-ezr2 仓库中 ezr-demo 子项目：Rust + ratatui 的终端 TUI 演示应用（下载器界面演示）。

## 快速运行

```bash
chmod +x bin/ezr-tui-demo
./bin/ezr-tui-demo
```

需使用支持鼠标事件与 ANSI 颜色的终端（xterm-256color 及以上）。

## 目录结构

- `bin/ezr-tui-demo` — 随包预编译的 release 二进制（x86-64 Linux, stripped）
- `src/` — 全部源码（`src/ui.rs` 布局与渲染、`src/app.rs` 交互与模拟器）
- `docs/` — 设计文档，含《ezr-demo UI界面及交互设计规范.md》（与实现同步维护）
- `scripts/` — 回归/验证脚本（见下）
- `Cargo.toml` / `Cargo.lock` — 构建清单

## 变更记录（相对上游）

1. **页脚双行重构（m2）**
   - 第一行：常用命令图例 `A 添加 · D 删除 · M 修改 · Q 退出 · H 帮助`，
     鼠标点击执行对应命令（按压反色高亮 + 400ms 余辉 = 点击效果，与键盘同口径）
   - 第二行：toast 提示独占一行（◆ 黄色）
2. **页脚左右内边距（m2 补丁）**
   - 两行内容左右各留 1 列空白，与头部面板右侧图区到右边框的留白同距；
     命中矩形随内容平移，点击语义不变
3. **并发列表滚动到底即停（m2 补丁，修复闪烁）**
   - 滚到底后继续下滚不再移动窗口、不再反复重绘；根因是滚轮事件未立即
     钳制 + 渲染层越界钳制过宽，现改为事件内钳制 + 渲染层「最后满窗」双重防线
4. **浮层翻转零清屏（修复打开新窗口整屏闪烁）**
   - 移除浮层翻转时的 `terminal.clear()` 全屏清屏（清屏瞬间整终端空白即闪烁）
   - 残影根因（diff 跳过被浮层覆盖的宽字符尾格）已由渲染层
     `clip_wide_at_edges` 在缓冲区层根治，前后帧模型自洽，清屏不再需要

## 重新构建

```bash
cargo build --release
# 产物在 target/release/ezr-tui-demo
```

Rust stable 工具链即可；依赖见 Cargo.lock。

## 回归/验证脚本

```bash
python3 scripts/test_focus_style.py         # 聚焦/滚动条/拖拽 56 项
python3 scripts/check_overlay_redraw.py     # 浮层翻转零清屏 + 残影检测 46 项
python3 scripts/check_popup_integrity.py    # 浮层边框完整性（理想终端模型）5 项
python3 scripts/verify_footer_pad_scroll.py # 页脚内边距 + 滚动修复专项 12 项
python3 scripts/smoke_v_sync.py             # 冒烟 + v-sync
```
