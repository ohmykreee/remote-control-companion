# AGENTS.md

这个仓库是 **DSH 远控伴侣**，一个桌面窗口，用来显示
[`dsh-remote-control`](https://github.com/ohmykreee/dsh-remote-control) 的远控页面，并补充麦克风选择、
VR 截图和安全的远控环境。面向用户的说明在 [README.md](README.md)。

支持 Windows 10/11 与 Linux（优先 KDE Plasma / GNOME）。

## 概览

- Tauri 2（Rust 后端）+ Svelte 5 runes + Tailwind v4 + shadcn-svelte。
- 便携发行：`exe` + 同目录 `Data/`，不写用户漫游目录。
- 窗口默认较小（`960x600`，16:10，最小 `800x500`），首屏左侧地址、右侧设置。
- 页面使用两个 WebView2 子 WebView：本地 chrome 与远端页面，而不是 iframe。
  `dsh-remote-control` 的静态页面会发送 `frame-ancestors 'none'`，iframe 方案不可用。
- 远端 WebView 是顶层页面，因此其初始化脚本可以注入自定义 API，且不受 iframe 限制。

## 目录

```text
src/                      Svelte 渲染层（本地 chrome WebView）
  App.svelte              外壳、布局状态、设置弹窗、覆盖层
  lib/
    api.ts                Tauri command 契约与类型
    copy.svelte.ts        中英文文案对象、当前语言状态与 `copy` 代理
    state.svelte.ts       应用状态、URL 规范化、加载/刷新流程
    components/
      TitleBar.svelte     自绘标题栏、应用菜单、刷新、窗口按钮
      HomeView.svelte     首次打开的左地址 / 右设置
      SettingsPanel.svelte
      MicrophonePanel.svelte
      AboutDialog.svelte
      ui/                 shadcn-svelte 官方组件（不要手写替代）
src-tauri/src/
  lib.rs                  启动、窗口、插件、命令注册
  commands.rs             IPC 命令；会操作 WebView 的命令必须避开主线程死锁
  layout.rs               子 WebView 的尺寸、位置、显示/隐藏
  remote.rs               URL、安全上下文参数、远端初始化脚本
  backdrop.rs             用 WebView2 自身截取远控画面，作为菜单模糊背景
  motion.rs               把减少动态效果同步给远控页面的 prefers-reduced-motion
  language.rs             通过 WebView2 消息接收远控页面语言、主题与状态
  osc.rs                  最小 OSC 编码与 UDP 发送
  vrchat.rs               VRChat 聊天框状态面板格式与刷新线程
  linux_webview.rs        Linux 原生应用栏/工具栏，以及 GTK 侧边栏布局
  window_behavior.rs      窗口透明度、全局老板键、靠边隐藏滑动
  settings.rs             Data/settings.json
  capture.rs              OpenVR 截图、3 秒提示音
  bridge.rs               仅回环的 WebSocket 截图桥
  paths.rs                portable Data 布局
  state.rs                运行时状态、桥 token
assets/
  remote_bootstrap.js     注入远端页面的麦克风 shim、状态回报与 API 容器
  remote_vr_api.js        暴露给 dsh-remote-control 的截图 API
test/mock-remote.mjs      本地远控页 mock
scripts/                  开发期 CDP 辅助脚本、便携打包
```

## 命令

```bash
pnpm install
pnpm dev                 # 只起 Vite
pnpm dev:tauri           # 开发桌面应用
pnpm check               # svelte-check，必须零 warning（或明确记录）
pnpm test                # frontend check/build + 原生平台 Rust 测试
pnpm build               # 前端生产构建
pnpm run build:rust      # tauri build --no-bundle
pnpm run dist            # 构建并组装当前平台的便携包
pnpm tauri build --no-bundle --target <triple>  # CI 的单目标构建
node scripts/assemble-portable.mjs --target <triple>
node test/mock-remote.mjs
```

Rust 测试在 Linux/macOS/Windows 的当前平台运行：

```bash
pnpm run test:rust:compile   # cargo test --no-run
pnpm run test:rust           # cargo test -- --test-threads=1
pnpm run test:rust:windows   # 需要显式 Windows amd64 时使用
```

需要 SteamVR 和已唤醒头显的截图冒烟测试（ignored by default）：

```bash
cargo test --manifest-path src-tauri/Cargo.toml capture_openvr_smoke -- --ignored --nocapture
```

**不要同时运行 `pnpm dev:tauri` 和 `cargo test`/`tauri build`**：Cargo 共用 `src-tauri/target`，会互相等锁，
文件 watcher 也会反复重启正在运行的应用。

## 架构约束

### 两个 WebView

一个无边框 `Window`（label `main`），里面有两个 child WebView：

- `chrome`：本地 Tauri 页面，初始铺满窗口；首屏显示地址和设置。
- `remote-<generation>`：连接到远控地址，底部 WebView；每次连接/刷新创建新 label，旧 WebView 关闭。

布局规则（`layout.rs`）：

- 首屏：`chrome` 全窗，远端不存在。
- 已连接、无覆盖层：`chrome` 只有 44px 标题栏，远端在 y=44 之下铺满。
- 菜单/设置/关于打开：先隐藏远端，把 `chrome` 扩到全窗；关闭后先缩回 44px，再显示远端。
  这样菜单和弹窗不会被 44px 的 WebView 裁切，也不会遮住远端交互。

覆盖层会临时遮住远端页面，这是有意的；不要改成“全窗透明 WebView + 点击穿透”，
现有方案在当前需求下更稳。

### 创建/调整 WebView 必须放到阻塞线程

Tauri 命令里直接调用 `Window::add_child` / `Webview::set_size` 会在某些调用路径上等待主线程事件循环，
造成 IPC 命令永不返回（曾实际发生：首屏一直停在“连接中”）。

- `open_remote`、`refresh_remote`、`set_chrome_overlay` 必须保持 `async` + `spawn_blocking`。
- 这些命令内部自行通过 `AppHandle::state::<AppState>()` 获取状态，避免把 `State<'_>` 移入 `'static` 闭包。
- 窗口最小化、最大化、关闭、拖动由前端 `@tauri-apps/api/window` 调用；不要在命令里另做一套。
- 新增会碰 WebView/窗口的函数时，先确认它所在线程不会阻塞主线程。

### 标题栏

- 无边框窗口，自绘标题栏 44px。
- 拖动区域是一个独立的绝对定位背景层 `.titlebar-drag`，带 `data-tauri-drag-region`，位于所有按钮下面。
  **不要**把 `data-tauri-drag-region` 放在标题栏根元素或按钮祖先上：这会让真实点击被拖动处理截获，
  表现为应用菜单完全点不开。
- 标题栏按钮需要比 drag 层更高的 z-index。
- 按钮使用 shadcn `Button`；图标只用 Lucide。禁止手绘功能图标。
- 左侧只保留「应用」和（连接后）「刷新」两个平铺文字项；不要放 logo。应用图标只用于窗口/任务栏。
- 应用菜单的顺序是置顶/取消置顶、分隔线、设置、回到连接页（仅远控页面）、关于本软件、分隔线、退出。
  置顶状态通过 `core:window:allow-is-always-on-top` / `allow-set-always-on-top` 查询和切换，不持久化。
- 连接页标题栏显示语言下拉菜单；进入远控后隐藏。语言选项只写入本地设置。
- 打开菜单时远端页面会隐藏；关闭菜单要恢复。
- 设置、关于等所有弹窗的遮罩从标题栏下方开始（`inset: var(--titlebar-height) 0 0 0`），标题栏
  `z-index: 60` 且 `pointer-events: auto`，弹窗内容也整体下移；打开弹窗后标题栏必须保持清晰、可点击、
  可拖动。弹窗的 `onInteractOutside` 只在事件目标位于 `.titlebar` 内时阻止关闭，不要改成全局 ignore，
  否则点击遮罩也无法关闭。
- 打开菜单前，通过 WebView2 自身的 `CapturePreview`（JPEG）截取远端画面，作为本地覆盖层的模糊背景；
  不要用屏幕截图或页面内 canvas。截图数据只在内存中，覆盖层关闭时丢弃；截图失败时退回纯色背景，不阻塞菜单使用。
- 关于弹窗与加载页的 logo 直接引用 `src-tauri/icons/icon.png`，保证和 exe/任务栏图标一致；不要再用 Lucide 拼一个替代 logo。
  标题栏仍按设计不显示 logo。

### 窗口透明度、老板键与靠边隐藏

- 窗口透明度使用原生分层窗口（`WS_EX_LAYERED` + `SetLayeredWindowAttributes`）；WebView2 子窗口会随父窗口
  一起透明。范围固定为 **5–100%**：低于 5% 会让窗口难以找到，后端 `sanitize` 和前端 Slider 都钳制到该范围。
  不要改成设置 WebView 的 CSS opacity，那只影响页面，不会让整个窗口透明。
- 老板键由官方 `tauri-plugin-global-shortcut` 注册，全局生效。`save_settings` 是 async + `spawn_blocking`，
  不能改回同步命令：插件注册会 `run_on_main_thread` 并等待；在主线程或同步命令里调用会死锁。
  启动时的注册放在单独线程；更换老板键时先注册新键、成功后再注销旧键，避免冲突时丢失原快捷键。
- 靠边隐藏由 `window_behavior.rs` 中的 Win32 巡检线程完成（100ms 轮询）。收起/出现使用 14 步、
  每步 12ms 的 ease-out cubic 滑动。窗口内侧距边缘不超过 10px、或已经越过边缘但仍与屏幕相交时算贴边；
  鼠标离开窗口 650ms 后才收起，鼠标进入边缘 160px 的区域会恢复。鼠标左键按下时、窗口最大化或覆盖层打开时
  不收起，避免拖动/菜单期间误触。只收起 6px 边缘（DWM 实际边框会多几像素）。禁用靠边隐藏或移除当前方向时，
  线程会在下一轮把窗口滑回。
- 多方向设置允许为空；空数组表示暂时不隐藏，不要自动补一个方向。设置只接受 `top`、`left`、`right`，
  `sanitize` 会去重和过滤。
- 靠边隐藏判定允许窗口越过边缘（例如左边缘 x=-60），只要仍与屏幕相交就算贴边；恢复时按吸附后的边缘位置
  显示，不要把越界位置记成正常位置。窗口完全离开屏幕时不算贴边。
- 设置 `reduce_motion` 为真时，靠边隐藏直接 `SetWindowPos`，不播放滑动；前端同时给 `<html>` 加
  `reduce-motion` 类，全局缩短动画和过渡。不要只做其中一半。
- 远端页面通过 Chromium 的 `Emulation.setEmulatedMedia` 收到同一个设置：`motion.rs` 在新建/刷新远控
  WebView 时，以及设置变化时，把 `prefers-reduced-motion` 设为 `reduce`；关闭时发空 `features` 清除覆盖，
  让页面重新跟随系统。远端页面自己的 `matchMedia` 监听会因此切换，切页等动画必须同步关闭。
  不要只改本地类名而没有通知远端 WebView。
- 测试/调试边缘行为时，可以直接用 Win32 `MoveWindow` 把窗口贴边并 `SetCursorPos` 到窗口外；
  不要用截图判断，读取 `GetWindowRect` 的位置即可。

### 远控页面状态上报（单次注入）

- 每个远控 WebView 只调用一次 `initialization_script`：`remote.rs::state_script` 注入
  `remote_bootstrap.js`，同时负责语言、主题、VRChat 状态、麦克风 shim 与减少动态效果。不要为语言、
  主题或状态再 `ExecuteScript` / `evaluate_javascript` 轮询，也不要把一次页面读取拆成多个注入。
- 同一个注入脚本内有多个行为：
  - `companion-ui`：`DOMContentLoaded` 和 `MutationObserver` 在 `lang` / `class` / `data-theme` 变化时
    立即上报 `language` / `theme`，去重后只发变化的字段；
  - `companion-status`：每秒轮询 `window.__DSH_REMOTE__`，只上报 VRChat `status`；
  - 麦克风 shim、reduce-motion 样式仍由这同一个脚本处理，不额外注入。
- `language.rs` 统一处理 `companion-ui`（发 `remote-language` / `remote-theme` 事件并更新 Linux 原生栏）
  和 `companion-status`（交给 `vrchat::handle_status_message`）。
- VR 截图 API 是唯一按设置单独注入的部分：`remote.rs::vr_api_script` 只在
  `replace_clipboard_with_vr` 开启时作为第二个 `initialization_script` 注入；它不能再塞回状态脚本，
  也不要把 bridge token 暴露到全局。
- 主题读取仅保留 `remote_bootstrap.js::readRemoteState` 的逻辑（`body.dataset.theme`、`html.light/dark`、
  `meta[name=theme-color]`）。页面加载中或读取失败时不发 theme，保留上一次标题栏主题；返回连接页
  `go_home` 调用 `language::reset_theme` 恢复暗色标题栏。
- Linux 原生栏的 `set_language` / `set_theme` 由 `language.rs` 在收到 `companion-ui` 时调用；
  不要恢复第二个 `evaluate_javascript` 轮询。
- 亮色标题栏使用 `#eef2f8`，菜单使用白底深色文字；暗色保持现有风格。标题栏之外的主界面仍按暗色设计。

### i18n 与语言同步

- 全部本地界面文案在 `src/lib/copy.svelte.ts`，**中文一个对象、英文一个对象**，由 `copy` Proxy 按当前语言返回；
  `setCopyLocale` 维护当前语言。组件中不要新增硬编码中文/英文，两个对象的键必须保持一致。
- 界面字体统一来自 `@fontsource-variable`（Inter、Noto Sans SC、JetBrains Mono），在 `src/main.ts`
  导入，`app.css` 的 `--font-sans` / `--font-mono` 只引用这些内部字体，不添加系统字体族。
  新增字体也必须用 Fontsource 打包进 `dist`，不要依赖系统安装或外部 CDN。
- App 在设置加载、用户选择语言、远控页面报告语言时更新文案；设置字段 `language` 只接受 `zh` / `en`，
  下次启动由 `get_bootstrap` 恢复。连接页标题栏显示语言菜单，进入远控后隐藏。
- 远控页面语言是连接后的权威值：`remote_bootstrap.js` 在 `companion-ui` 消息的 `language` 字段中，
  在 `<html lang>` 变化时立即上报；`language.rs` 安装消息监听并发 `remote-language`；App 收到后保存设置。
  不要改用 Tauri command（远端 WebView 没有对应权限）。
- 不要在批量替换 UTF-8 文本时使用 PowerShell `Get-Content`/`Set-Content`：本机 PowerShell 曾把中文变成乱码。
  使用编辑工具或显式 UTF-8 读写。
- “回到连接页”走 `go_home`：关闭远端 WebView、清空运行时的 remote URL/overlay，并重新同步本地 chrome；
  菜单项只在远控模式显示。

### 远端权限与麦克风

- 远端 WebView 的 User-Agent 从本地 chrome WebView **实时读取**，再追加
  `RC-Companion/<版本>`；不要写死 UA。这样 `dsh-remote-control` 的原始 UA 悬停内容仍包含真实的
  WebKit/WebView2 平台与引擎版本。
- 远端 WebView 打开剪贴板访问，并将 WebView2 权限请求静默设为 `Allow`。
- `remote_bootstrap.js` 注入到所有文档，但检查 `window.top === window` 与 `location.origin`；只在目标页顶层生效。
- 若设置里保存了麦克风 label，会包装 `navigator.mediaDevices.getUserMedia`：在远端页面自己的
  device list 中按 label 找到设备，并强制 `deviceId.exact`。设备 id 是 origin 作用域的，不能用本地页的 id。
- `initialization_script` 在 Windows 会进入子 frame；上面的 guard 不可删除。

### 安全上下文

- 用户开启「强制开启安全上下文」时，对 `http` 地址给远端 WebView 添加
  `--unsafely-treat-insecure-origin-as-secure=<origin>`。
- WebView2 规定：**浏览器参数不同的 WebView 必须使用不同 user data directory**。参数会 hash 成
  `remote-<hash>` profile；不要共用一个 profile。
- 该开关按“下一次创建远控 WebView”生效，即重连/刷新后。界面提示必须与实际行为一致。
- 不要通过 JS 伪造 `isSecureContext`，浏览器不接受，且会影响插件判断。

### 截图 API 与桥

- 只有 `replace_clipboard_with_vr` 开启时才注入 `window.__DSH_COMPANION__`；否则页面不应检测到它。
- 对外 API：`version`、`platform`、`capabilities.vrScreenshot`、`captureVrScreenshot(): Promise<Blob>`。
  返回 PNG Blob（SteamVR 的 preview 图）。
- API 通过 WebSocket 连接 `127.0.0.1:<随机端口>`，token 每次启动随机，握手校验失败直接 401。
  **只监听回环地址**，不要改成 0.0.0.0。token/端点只注入给已连接的远端顶层页面。
- 协议是二进制信封：4 字节 little-endian JSON 头长度 + JSON 头 + PNG 字节；失败时发送 JSON 文本错误。

### 覆盖层背景截图

- `capture_remote_backdrop` 与 VR 截图 API 是两套东西：前者只用于菜单/设置的背景伪装，后者给插件使用。
- 通过 `Webview::with_webview` 取得 WebView2 controller，调用 `ICoreWebView2::CapturePreview` 输出 JPEG；
  不要用 GDI 屏幕截图，避免遮挡、DPI 和窗口捕获问题。
- 捕获发生在 `set_chrome_overlay` 之前，否则远端 WebView 已被隐藏。`backdropTask` 需要合并并发调用。
- 图像以 data URL 放在本地 chrome，带 CSS blur；不写磁盘，不进入桥，不暴露给远端页面。
- 失败路径必须静默降级到纯色背景，菜单仍然可用；不要把错误弹给终端用户。

### VR 截图

- 不静态链接 OpenVR SDK。运行时按顺序找 `openvr_api.dll`：程序目录 → `%LOCALAPPDATA%\openvr\openvrpaths.vrpath`
  → 默认 SteamVR 安装目录。发布打包时如果构建机有 SteamVR，会复制 DLL 到发行目录。
- 使用官方 `IVRScreenshots` 的 `TakeStereoScreenshot`，通过 `VR_GetGenericInterface` 获取
  `FnTable:IVRScreenshots_001`。函数表布局以 openvr.h 为准。
- OpenVR `EVRApplicationType` 数值容易记错：`Scene = 1`、`Overlay = 2`、`Background = 3`、`Utility = 4`。
  使用 `Utility` 会得到错误码 123（`NotAvailableToUtilityApps`）；使用 `Background` 可以拿到接口，但没有
  截图纹理。这里使用 **Overlay (2)**。
- `TakeStereoScreenshot` 只是让 compositor 去读当前 scene textures；成功返回不代表已写文件。
  没有正在显示的 VR 应用时，SteamVR 日志会写 `Failed to get the correct textures`，随后不产出图片。
  因此等待超时后要提示用户先打开 VR 应用/唤醒头显。默认等待 10 秒。
- 截图路径必须是普通绝对路径。**建议不要 `fs::canonicalize`**：Windows 会加 `\\?\` 前缀，
  SteamVR 的 C 文件写入不保证接受这种路径；当前直接使用 `Data/Cache/VRScreenshots` 的绝对路径。
  路径中包含非 ASCII 时也要留意。
- 延迟 3 秒时：播放内嵌的 `src-tauri/assets/capture-countdown.wav`（前 3 秒每秒一声提示音，第 3 秒
  是相机快门），等待 3 秒后截图。快门取自 CC0 的 "DSLR Shutter fast 006.wav"（见
  THIRD_PARTY_NOTICES），与三声提示音预先混合。声音使用 `PlaySoundW(SND_MEMORY | SND_ASYNC)`，不依赖
  MIDI 设备。这是临时音效，不要添加持久的声音生成脚本；需要调整时直接替换 WAV，并保留可试听文件。
- 截图在返回插件前会被裁剪：SteamVR 的 preview 是近方形纹理，下方有未渲染区域；当前取顶部
  `57.2%` 高度（依据用户红线参考），仅对近方形图生效。裁剪在 `crop_preview` 中完成，失败时退回原图。
  不要把这个裁剪交给插件页面。
- 截图接口可用性冒烟测试：`capture_openvr_smoke`。该测试需要头显里有活动 VR 画面；没有画面时会超时，
  这是运行时依赖，不是单元测试失败。
- 截图结束必须调用 `VR_ShutdownInternal`；同一时间只允许一个截图任务（`capture_lock`）。

### VRChat OSC 聊天框

- 设置字段：`vrchat_osc_enabled`、`vrchat_osc_host`（默认 `127.0.0.1`）、`vrchat_osc_port`（默认 `9000`）、
  `vrchat_osc_mode`（`clock` / `music` / `system` / `agent` / `custom`）、`vrchat_osc_custom_text`、
  `vrchat_osc_push_enabled`。这些字段由后台线程每轮重新读取，修改后立即生效；`vrchat_osc_push_enabled`
  需要刷新一次远控页面，选中回答的推送按钮才会按新配置出现。
- 数据只来自远控页面的 `window.__DSH_REMOTE__`：`remote_bootstrap.js` 每秒把时间、天气、媒体、性能、
  会话与余额裁剪成 `companion-status` 消息；`language.rs` 的消息监听转给
  `vrchat::handle_status_message`。不要为这些信息另开系统 API，也不要读取 dsh-remote-control 的内部实现。
- `vrchat.rs` 生成顶部装饰标题行 + 文字的小面板，**没有底部横线**。普通内容最多三行文字（共四行）；性能面板
  固定四行（CPU 型号、CPU 与内存、GPU 型号、GPU 与显存），共五行。VRChat 聊天框按行居中，所以
  **不要加左右竖线，也不要用行尾空格对齐**；标题两侧的 `·=` 花纹必须对称，标题用 `« »` 框住。
  所有文案跟随 `settings.language`，中英文标题、状态和天气标签都在 `vrchat.rs` 内。
- 单行超过 27 列时由 `scroll_line` 取 `line + 8 个空格` 的循环窗口，右端始终显示 `…`，每帧移动 2 个
  字符；下一份文字在上一份离开前通过间隔进入，不要等整行完全消失后重置。每一行独立滚动；自定义行也走同一套
  逻辑。快速问答的三行阅读动画是独立行为，不套用这个窗口循环。
- `custom` 模式**不加任何装饰**：直接发送 `vrchat_osc_custom_text`，最多 4 行、160 个字符（`settings.rs`
  的 `sanitize_custom_text`），空内容不发送。自定义文字不依赖远控页面，没有连接页面时也可以持续显示。
- 自定义文字支持 `{time}`、`{date}`、`{weekday}`、`{location}`、`{weather}`、`{temperature}`、`{song}`、
  `{artist}`、`{cpu}`、`{cpu_usage}`、`{gpu}`、`{gpu_usage}`、`{ram}`、`{vram}`、`{agent}`、`{balance}`、
  `{today_spend}`。实际发送时从当前状态替换，未连接远控时显示 `--`；`get_osc_preview` 供设置页显示当前值，
  未连接时返回示例值。设置页 codeblock 在文字为空时列出全部占位符，有文字时显示替换预览；右上角切换按钮可在
  两种视图间手动切换，预览由前端每 2 秒刷新。
  占位符列表固定两列；面板预览和文字预览居中显示，占位符列表保持左对齐。
- 内置模式也使用同一个 `panel_preview` 生成 codeblock 预览（tick 0，长行显示省略号）；设置页每 2 秒刷新一次。
  预览只是展示，不进入 OSC 发送队列。
- 快速问答推送开关放在同一张 VRChat OSC 卡片内。`remote_bootstrap.js` 只在 `config.oscPush` 为真时监听
  `.quickqa-chat-assistant` 中的文字选择，显示原生悬浮按钮并通过 `osc-push` 消息发送选中文本；这是注入脚本的
  行为部分，不新增第二个信息注入。按钮使用高对比度蓝底白字。选择文本取自 **渲染后的 DOM**
  （`selection.toString()`），不要读取 `quickQa.answer` 的 Markdown 源；已用 marked 渲染的标题、粗体、
  列表、引用、链接和代码块验证，选择结果不含 `#`、`**`、反引号、列表符号或 URL。换行和连续空白在 JS 与
  Rust 两侧都折叠为一个空格，推送始终是单段纯文本。
- `osc-push` 由 `language.rs` 交给 `vrchat::handle_push_message`，文本折叠为单行并最多 2000 字符，存入
  `AppState.osc_push`。后台线程临时用它覆盖普通面板：固定三行、无标题/装饰，先停留 5 帧供阅读，再每帧滚动
  1 个字符，最后停留 4 帧后恢复；推送帧间隔 1.1 秒。若后面仍有文字，末行显示 `…`；滚到剩余不超过三行时去掉
  省略号并进入结尾停顿。推送结束后若普通内容为空，要发送一次空聊天框消息清除。
- 整条消息必须 ≤ 144 字符（VRChat 聊天框上限）；单行文字留有截断保护，标题规则也受同一预算约束。
  发送频率固定约 1.5 秒，符合 VRChat 约 5 条 / 5 秒的限制；不要改成逐帧或明显更快的刷新。
- `osc.rs` 只实现最小的 `/chatbox/input` 编码：地址、`,sT`（立即发送）和字符串参数，四项对齐补零；
  通过 `UdpSender` 发到配置的地址与端口。不要引入第三方 OSC 依赖，也不要发 `/chatbox/typing`。
- 没有远控页面、或状态超过 6 秒未更新时，发送「等待远控页面」面板，且每 8 秒最多一次；关闭功能或
  关闭远控页面后不再发送。发送失败只记录目标地址与错误，并做 30 秒节流。
- 图标只使用在 VRChat 聊天框里实测能显示的字符：标题 `⏱ ⚙ ★ ♪`，天气 `☀ ● ☁ ☂ ❄ ⚡`，
  CPU/GPU `■ ▤`。已实测缺字或会退化成圆圈的字符有 `☾`、`◈`、`≋`、`▭`、`▬`；新增图标必须先在游戏里确认再使用。
- 不要把面板内容（位置、歌曲、余额）写进日志；这些是用户隐私数据。

## 数据与便携约定

- 发行版目录：

```text
DSHRemoteCompanion.exe
openvr_api.dll          # 可选，打包时自动复制
Data/
  settings.json
  Logs/session.log
  Cache/Webview/...
  Cache/VRScreenshots/...
```

- 开发时 `app_root` 找包含 `package.json` 和 `src-tauri` 的祖先目录；发行版则用 exe 所在目录。
- Data 写入失败不能让程序崩溃；提示用户放到可写目录。
- 设置明文保存，远控地址可能包含 token，不要提交 `Data/`，不要把真实地址写入测试 fixture 或文档。
- 日志允许记录诊断详情，但 URL 日志必须走 `logging::redact_url` 去掉 query。

## shadcn-svelte 与界面规范

- **所有交互控件来自 shadcn-svelte 官方组件**。用官方 helper 生成/更新，不要手写一份相似组件：

```bash
pnpm dlx shadcn-svelte@latest add <component> --overwrite --yes
```

- 当前已使用：Button、Input、Textarea、Checkbox、Switch、Select、ToggleGroup、Slider、Dialog、Progress、Separator、DropdownMenu、Sonner、Tooltip。
  设置页的「显示内容」使用 ToggleGroup 做分段选择，不要退回下拉菜单或手写分段控件。
- `components.json` 的 style 使用生成器支持的 `nova`。`$lib/utils.ts` 的 `cn` 来自 `cn` 包。
- 图标只用 `@lucide/svelte`；不要画自定义功能图标、不要 emoji。
- 设置页的 codeblock 是展示用 `<pre>`，shadcn 没有对应组件；它的切换按钮、Textarea、Switch 等交互控件仍必须
  使用 shadcn。注入到远控页面的“推送到 OSC”悬浮按钮只能使用原生 `<button>`，不能把本地 Svelte 组件带进页面。
- 界面保持小而安静：初始窗口只有左侧连接、右侧设置；不要再加入大段装饰、状态卡堆叠或第二个主按钮。
- 应用/任务栏图标来自 dsh-remote-control 图标，并在右下角叠加一个小播放徽标；通过
  `pnpm tauri icon src-tauri/icons/icon.svg` 从该 SVG 生成全套图标。
- 远端页打开时，chrome 只有 44px；任何未处于覆盖层中的界面元素都不可见或不可点击。新功能必须明确它在哪个层。

## 文案规范（必须遵守）

面向终端用户的文字（侧边栏、手机端、README、错误与提示）只回答「这是什么、能做什么、怎么做、有什么限制」，
不解释实现方式。

- 只写用户看得见的结果和能做的操作；不写代码路径、模块名、协议名、运行时、数据库、缓存、轮询、归一化、
  探测、CDP / SMTC / MPRIS / WebSocket / Node.js / Rust 等实现细节。
- 配置项可以写用户需要提供的内容（证书文件、端口、城市、音频文件），但不要解释内部如何处理。
- 提示语尽量一句话；范围和限制只在会影响用户决策时出现，不在 README 里展开机制。
- 错误信息要告诉用户下一步能做什么，不要只抛内部错误名或后端状态。
- 诊断 / 系统信息页可以展示排障需要的版本、路径与平台信息；这些是诊断数据，不是功能说明。
- 新增或修改文案时同步中英文两个对象并重新构建。本项目当前只有简体中文和英文，界面文案集中在
  `src/lib/copy.svelte.ts`；后端给用户看的中文错误在 `src-tauri/src/`（尤其 `capture.rs`、`remote.rs`）。
  本地界面只显示本地化后的通用错误，不直接展示后端中文错误。
- 修改插件（dsh-remote-control）时按插件要求同步 **侧边栏 / Host / 手机端三份字典**。
- README 只写功能、安装摘要、使用方式、平台支持与已知限制；架构、协议、构建细节放本文件。

## 文案与界面同步

- 新增设置项时同步：`src/lib/copy.svelte.ts`、`src/lib/components/SettingsPanel.svelte`、`src-tauri/src/settings.rs`
  （字段、默认值、sanitize、持久化）。
- 与远端页面有关的设置项要明确“刷新后生效”还是“重连后生效”。窗口透明度、老板键、靠边隐藏属于本地窗口行为，
  不需要刷新远控页面。新增窗口字段时同时检查 `window_behavior::apply_*` 的启动应用路径。
- VRChat OSC 的开关、地址、端口与显示内容都立即生效，不需要刷新远控页面；文案同步
  `src/lib/copy.svelte.ts`、`src/lib/components/SettingsPanel.svelte`、`src-tauri/src/settings.rs`。
- 修改页面结构后，运行 `pnpm check` 与 `pnpm build`；修改 Rust 后运行 `cargo test` 与 `cargo check`。

## 开发调试

- 当前验收状态：
  - 已用真实 SteamVR + Beat Saber 画面验证 `TakeStereoScreenshot` 返回 PNG（约 11 MB / 2747×2746）。
  - 已用真实鼠标事件验证标题栏拖动、应用菜单、设置弹窗、窗口最大化和还原。
  - 已验证应用菜单置顶、窗口透明度、老板键、靠边隐藏及滑动恢复。
  - 已验证远控页面亮/暗主题切换会同步标题栏和菜单配色。
  - 已验证连接页语言切换、远控页面语言回报与保存、回到连接页。
  - 已验证开启减少动态效果后靠边隐藏不再播放滑动。
  - 已验证减少动态效果通过 CDP 同步到远控页面，`matchMedia` 收到 change 事件。
  - 已用 LAN http 地址验证安全上下文豁免（`window.isSecureContext === true`）和远端独立调试端口。
  - 已验证按 label 选择麦克风后，远端页面 `getUserMedia` 返回所选设备。
  - 已用真实 VRChat（游戏内开启 OSC）验证聊天框面板：时间天气、音乐与进度、居中对称标题规则、
    `▶` / `⏸` 状态图标和实时进度刷新。
  - 已验证单次注入同时处理 `companion-status` 轮询、`companion-ui` 语言/主题钩子与 reduce-motion；
    VR 截图 API 只在设置开启时作为单独的第二个注入出现。
  - 已用本地 UDP mock 验证长行省略号滚动、自定义行滚动、占位符替换和 codeblock 预览；也验证了快速问答
    推送的三行显示、初始停留、逐字滚动、结尾停留和恢复原内容。
- 本地 mock 远控页：`node test/mock-remote.mjs`，默认 `http://127.0.0.1:39001/`；页面会显示
  `isSecureContext`、伴侣 API 是否可用、麦克风与截图结果，并提供 `window.__DSH_REMOTE__` 假数据，
  用于不连接真实远控页面时测试 VRChat OSC 面板。
- OSC 接收 mock：`node scripts/dsh-osc-mock.mjs` 按时间顺序打印 `/chatbox/input`；加 `--live` 后每次收到
  消息先清空终端，只重绘最新一条，模拟 VRChat 聊天框替换内容的效果。它是开发工具，不进入测试套件或 CI；
  默认监听 `127.0.0.1:9000`，VRChat 运行时需先关闭游戏或改用其他端口。
- VRChat OSC 的本机测试：游戏内开启 OSC 后，在设置中开启「VRChat OSC 展示」并选择内容；默认发到
  `127.0.0.1:9000`。没有真实远控页面时可以先用上面的 mock 页，聊天框会显示它的假数据。
- 调试 WebView（仅开发构建）：直接运行 `pnpm dev:tauri`。本地 chrome 使用 DevTools 端口 `9222`，
  远端 WebView 使用独立端口 `9223`（`DSH_COMPANION_REMOTE_DEBUG_PORT` 可覆盖），不需要设置浏览器参数环境变量。
  `scripts/cdp-shot.mjs`、`cdp-eval.mjs`、`cdp-click.mjs`、`cdp-console.mjs` 用于本地验证。
  `cdp-click.mjs` 用真实鼠标事件，能抓到 drag region 之类的交互问题；程序化 `.click()` 抓不到。
- 如果手动把 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 加进开发环境，它会作用于所有 WebView2 环境，
  并可能与远端自己的调试端口冲突；正常调试不要设置它。
- 首次截图前确认头显里有活动 VR 画面；只有 SteamVR 主界面而没有场景时，截图请求会等待后报超时。

## 发布

便携包不是安装器：

```bash
pnpm run dist
```

Windows 产物在 `release/DSH-Remote-Companion-win-x64/`，Linux 为
`release/DSH-Remote-Companion-linux-x64/`，同时生成同名 zip。`scripts/assemble-portable.mjs`
接受 `--target <triple>`，复制对应 `src-tauri/target/<triple>/release` 的二进制、准备
`Data/README.txt`，Windows 上还会从构建机 SteamVR 复制 `openvr_api.dll`，并带上 README、
LICENSE、THIRD_PARTY_NOTICES。

## Linux 平台差异（当前实现）

- Linux 使用 WebKitGTK。Tauri 的 Linux 多 WebView 容器是 `GtkBox`，子 WebView 的绝对 bounds
  在那里不生效；因此 Linux **不使用** 与 Windows 相同的“44px chrome + 绝对定位 remote”行为。
- `linux_webview.rs` 在窗口里创建原生 GTK 顶栏：44px 应用栏（标题和窗口按钮）+ 38px 工具栏
  （设置 / 返回 / 刷新 / 关于 / 语言）。Linux 的连接页、远控页和设置页都使用这套顶栏；本地 chrome
  WebView 在远控时隐藏，远控 WebView 在工具栏下方占满剩余窗口；打开设置/关于时显示全窗口本地 chrome。
- 应用栏必须用 `GtkEventBox` 接收空白区/标题的按钮事件，拖动调用
  `gtk::Window::begin_move_drag(button, root_x, root_y, time)`。Tauri 的 `start_dragging` 在 Wayland
  不能开始移动；不要退回它，也不要把拖动处理只挂在 `GtkBox` 或标题 `GtkLabel` 上。
- 内网 HTTP 的 Linux 安全上下文由 `loopback_proxy.rs` 提供：把远控页加载为
  `http://127.0.0.1:<随机端口>/...`，原始 HTTP/WebSocket 字节转发到真实服务器。localhost 是 WebKitGTK
  的安全上下文；`remote_bootstrap.js` 的 origin 必须用这个本地页面地址，而 `RuntimeState.remote_url`
  仍保存用户输入的原始地址。代理只监听回环地址，`go_home`/刷新时释放。
- Linux 远端 WebView 必须连接 WebKitGTK 的 `permission-request`，对
  `UserMediaPermissionRequest` 调用 `allow()`；只设置 Tauri 的 `on_permission_request` 不够。
- `force_secure_context` 默认关闭；`auto_secure_context` 默认开启。连接内网 HTTP 地址时，只有自动开关
  开启才会把 `force_secure_context` 设为 true、立即保存，并通过 `LayoutState.force_secure_context`
  同步到前端。`is_private_http_url` 只判断 IP 字面量，不解析主机名。
- 不要在 Linux 上尝试用 `set_position` / `set_size` 摆第二个 WebView；上游 Tauri 会忽略这些 bounds。
  也不要把它重新塞进 `GtkFixed` 或 reparent WebKit 控件，这会在 GTK/Wayland 下崩溃。
- 原生栏按钮通过 `linux-open-settings`、`linux-back-home`、`linux-refresh` 事件驱动前端，语言由
  `set_language` 更新；主题由 `set_theme` 更新。事件名和按钮文案必须与 `App.svelte` 的监听保持一致。
- Linux 不再单独轮询主题/语言：`language.rs` 收到 `companion-ui` 后调用原生栏的
  `set_language` / `set_theme`，同时发 `remote-theme` / `remote-language` 事件给 chrome。
- Linux 的减少动态效果由 `remote_bootstrap.js` 注入样式；Windows 仍使用 CDP 的
  `Emulation.setEmulatedMedia`。
- Linux VR 截图使用 OpenVR 的 `libopenvr_api.so`：程序目录、`$HOME` 下的 SteamVR 安装和
  `openvrpaths.vrpath` 都会被搜索；`DSH_COMPANION_OPENVR_PATH` 可指定测试库。
  `DSH_COMPANION_VR_MOCK=1` 返回确定性预览，只用于无头测试，不应作为生产截图。
- 安全上下文豁免在 Windows 使用 Chromium 启动参数；Linux 使用回环转发获得 localhost 安全上下文。
  覆盖层背景截图在 Linux 暂时降级为纯色背景。
- 窗口靠边隐藏使用 Tauri 的鼠标/窗口 API；KDE/GNOME Wayland 可能拒绝移动窗口，X11 会话可完整工作。
  全局老板键由 `tauri-plugin-global-shortcut` 提供，Wayland 下可能不可用。

## CI/CD

所有质量检查都在 **ubuntu-latest** 上运行；打包使用原生架构 runner，不做跨架构 sysroot。

- `.github/workflows/checks.yml`（可复用，Ubuntu）：
  - `pnpm check`、`pnpm build`
  - `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`
  - `pnpm run test:rust:compile` + `pnpm run test:rust`（Linux 原生测试）
- `.github/workflows/build.yml`：
  - `pull_request`：只跑 checks，不打包。
  - `push` 到 `main` / `workflow_dispatch`：checks 通过后并行构建：
    - `windows-latest` → `x86_64-pc-windows-msvc`
    - `ubuntu-latest` → `x86_64-unknown-linux-gnu`
  - 每个目标运行 `pnpm tauri build --no-bundle --target <triple>`，再由
    `scripts/assemble-portable.mjs --target <triple>` 生成
    `release/DSH-Remote-Companion-<platform>-<arch>.zip` 并上传 artifact。
- 暂未添加 publish/release 工作流；不要在没有相应要求时创建。
- CI 产物位于 Actions 的 artifact 中，保留 14 天。

仓库初始化和行尾/二进制规则见 `.gitattributes`；项目截图暂不纳入仓库。
