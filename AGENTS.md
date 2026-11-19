# AGENTS.md

这个仓库是 **DSH 远控伴侣**，一个便携式 Windows 桌面窗口，用来显示
[`dsh-remote-control`](https://github.com/ohmykreee/dsh-remote-control) 的远控页面，并补充麦克风选择、
安全环境和 VR 截图能力。面向用户的说明在 [README.md](README.md)；插件侧接入任务在
[HANDOFF.md](HANDOFF.md)。

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
    copy.ts               全部界面文案
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
  settings.rs             Data/settings.json
  capture.rs              OpenVR 截图、3 秒提示音
  bridge.rs               仅回环的 WebSocket 截图桥
  paths.rs                portable Data 布局
  state.rs                运行时状态、桥 token
assets/
  remote_bootstrap.js     注入远端页面的麦克风 shim 与 API 容器
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
pnpm test                # frontend check/build + Windows amd64 测试编译与执行
pnpm build               # 前端生产构建
pnpm run build:rust      # tauri build --no-bundle
pnpm run dist            # 构建并组装 release/DSH-Remote-Companion-win-x64
node test/mock-remote.mjs
```

Rust 测试（只针对 Windows amd64；项目本身只支持 Windows）：

```bash
pnpm run test:rust:compile   # cargo test --no-run --target x86_64-pc-windows-msvc
pnpm run test:rust           # 执行 Windows amd64 测试，单线程
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
- 打开菜单时远端页面会隐藏；关闭菜单要恢复。
- 打开菜单前，通过 WebView2 自身的 `CapturePreview`（JPEG）截取远端画面，作为本地覆盖层的模糊背景；
  不要用屏幕截图或页面内 canvas。截图数据只在内存中，覆盖层关闭时丢弃；截图失败时退回纯色背景，不阻塞菜单使用。
- 关于弹窗与加载页的 logo 直接引用 `src-tauri/icons/icon.png`，保证和 exe/任务栏图标一致；不要再用 Lucide 拼一个替代 logo。
  标题栏仍按设计不显示 logo。

### 远端权限与麦克风

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
- 插件接入细节在 [HANDOFF.md](HANDOFF.md)，不要在 README 里展开协议。

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

- 当前已使用：Button、Input、Checkbox、Switch、Select、Dialog、Progress、Separator、DropdownMenu、Sonner、Tooltip。
- `components.json` 的 style 使用生成器支持的 `nova`。`$lib/utils.ts` 的 `cn` 来自 `cn` 包。
- 图标只用 `@lucide/svelte`；不要画自定义功能图标、不要 emoji。
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
- 新增或修改文案时同步多语言字典并重新构建对应产物。本项目当前只有简体中文，界面文案集中在
  `src/lib/copy.ts`；后端给用户看的中文错误在 `src-tauri/src/`（尤其 `capture.rs`、`remote.rs`）。
  当一个字符串同时出现在 UI 和错误路径时，两处必须保持同一措辞。
- 修改插件（dsh-remote-control）时按插件要求同步 **侧边栏 / Host / 手机端三份字典**，见
  [HANDOFF.md](HANDOFF.md)。
- README 只写功能、安装摘要、使用方式、平台支持与已知限制；架构、协议、构建细节放本文件。

## 文案与界面同步

- 新增设置项时同步：`src/lib/copy.ts`、`src/lib/components/SettingsPanel.svelte`、`src-tauri/src/settings.rs`
  （字段、默认值、sanitize、持久化）以及 `HANDOFF.md` 中“是否需要插件配合”。
- 与远端页面有关的设置项要明确“刷新后生效”还是“重连后生效”。
- 修改页面结构后，运行 `pnpm check` 与 `pnpm build`；修改 Rust 后运行 `cargo test` 与 `cargo check`。

## 开发调试

- 当前验收状态：
  - 已用真实 SteamVR + Beat Saber 画面验证 `TakeStereoScreenshot` 返回 PNG（约 11 MB / 2747×2746）。
  - 已用真实鼠标事件验证标题栏拖动、应用菜单、设置弹窗、窗口最大化和还原。
  - 已用 LAN http 地址验证安全上下文豁免（`window.isSecureContext === true`）和远端独立调试端口。
  - 已验证按 label 选择麦克风后，远端页面 `getUserMedia` 返回所选设备。
- 本地 mock 远控页：`node test/mock-remote.mjs`，默认 `http://127.0.0.1:39001/`；页面会显示
  `isSecureContext`、伴侣 API 是否可用、麦克风与截图结果。
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

产物在 `release/DSH-Remote-Companion-win-x64/`，同时会生成 zip。`scripts/assemble-portable.mjs`
复制 exe、准备 `Data/README.txt`、从构建机 SteamVR 复制 `openvr_api.dll`（没有则运行时自动查找），
并带上 README、LICENSE、THIRD_PARTY_NOTICES。

## CI/CD

GitHub Actions 只在 **windows-latest** 上运行，目标固定为 Windows amd64；当前不做 Ubuntu 交叉编译。

- `.github/workflows/checks.yml`（可复用）：`pnpm check`、`pnpm build`、编译并执行 Windows amd64
  Rust 测试（`pnpm run test:rust:compile` + `pnpm run test:rust`）。
- `.github/workflows/build.yml`：
  - `pull_request`：只跑 checks，不打包。
  - `push` 到 `main` / `workflow_dispatch`：先通过 checks，再 `pnpm run dist` 生成
    `release/DSH-Remote-Companion-win-x64.zip`，并上传 artifact `DSH-Remote-Companion-win-x64`。
- 暂未添加 publish/release 工作流；不要在没有相应要求时创建。
- CI 产物位于 Actions 的 artifact 中，保留 14 天。

仓库初始化和行尾/二进制规则见 `.gitattributes`；项目截图暂不纳入仓库。
