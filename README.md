# PlayStreamer

<img src="public/brand/playstreamer.svg" width="88" height="88" alt="PlayStreamer 图标" />

PlayStreamer 是 Windows 优先的 PlayStation 直播接流助手。它在局域网提供针对 PS5 直播入口的 DNS 解析和本地媒体接收，将画面交给 OBS，再由 OBS 转播到 Bilibili，或通过投影窗口分享到 OOPZ。

[项目官网](https://jxqg.github.io/PlayStreamer/) · [操作文档](https://jxqg.github.io/PlayStreamer/guide/) · [下载 Windows 版](https://github.com/JxQg/PlayStreamer/releases/latest) · [反馈问题](https://github.com/JxQg/PlayStreamer/issues)

```text
PS5 广播 → PlayStreamer 本地接收 → OBS
                                  ├─ Bilibili 推流
                                  └─ 投影窗口 → OOPZ 分享
```

当前版本为 v0.1.1，安全更新见 [版本说明](docs/releases/0.1.1.md)。PS5、OBS、Bilibili 和 OOPZ 的真实设备联合验证状态见 [验证清单](docs/validation.md)。macOS 当前预留跨平台核心并在 CI 中检查，尚不提供桌面安装包。

## 功能

- 一键开启接收、停止接收与本地 RTMP 地址复制。
- 选择局域网网卡、指定 PS5 地址，检测 DNS 与 RTMP 端口、防火墙和接流状态。
- 显示码率、连接时长、音视频轨道；导出经过脱敏的诊断信息。
- 自定义标题栏、关闭到系统托盘、托盘开启/停止/复制地址。
- 应用与托盘共用原创图标，界面与教程统一使用随包附带的离线字体。
- 内置 PS5、OBS、Bilibili、OOPZ 教程。Windows 安装包由 GitHub Actions 按版本标签构建。

## 使用

需要 Windows 10/11 x64、Microsoft Edge WebView2 Runtime、OBS Studio，以及连接同一 IPv4 子网的 PS5 和电脑。建议电脑与 PS5 使用网线，并在路由器中保留 DHCP 地址。PS5 仍需能访问 PSN 和 Twitch 完成账号关联与开播；PlayStreamer 不替代这些服务的网络访问。

1. 从项目 Releases 下载 Windows x64 安装包；当前安装包未签名。
2. 在连接设置中选择网卡并输入 PS5 IPv4 地址。按需使用“配置防火墙”，Windows 会显示权限提示。
3. 开启接收，把 PS5 首选 DNS 设置为应用显示的电脑 IPv4 地址，按 [PS5 设置](docs/ps5-setup.md) 配置其余项目。
4. 在 PS5 选择 Twitch 并开始广播；应用显示正在接收后，在 OBS 添加媒体源。
5. 将以下地址填入 OBS 媒体源的“输入”，不要填入 OBS 的直播推流设置：

```text
rtmp://127.0.0.1:1935/live/playstation
```

关闭窗口后应用继续驻留托盘。停止接收会保留 DNS 解析，避免 PS5 因仍使用电脑 DNS 而立即失去解析服务；完全退出或关闭电脑前，请先结束 PS5 广播，并把 PS5 DNS 恢复自动。

完整教程：[快速开始](docs/quick-start.md) · [PS5 设置](docs/ps5-setup.md) · [OBS 与平台设置](docs/obs-platforms.md) · [故障排查](docs/troubleshooting.md)

电脑可以保留现有 TUN / VPN，但接流应选择物理局域网网卡。PS5 可按 [代理共存说明](docs/ps5-setup.md#与-tunvpn-和现有代理共存) 复用电脑已有的 HTTP / mixed 局域网代理；仅开启电脑 TUN 不会自动代理 PS5，应用的上游 DNS 也不是代理入口。

## 开发

技术栈为 Rust + Tauri 2、React + TypeScript、MediaMTX。Windows 需要 Rust 1.88 或更新 stable 版本、Visual Studio C++ Build Tools、WebView2、Node.js 22 或更新版本以及 pnpm 10。

```powershell
pnpm install --frozen-lockfile
node scripts/prepare-sidecar.mjs
pnpm tauri dev
```

只预览界面可以运行 `pnpm dev`；真实接收、托盘和诊断需要 Tauri 桌面进程。

```powershell
pnpm typecheck
pnpm build
node --test scripts/test-release.mjs
cargo fmt --all --check
cargo clippy --locked -p playstreamer-core --all-targets -- -D warnings
cargo test --locked -p playstreamer-core
cargo check --locked --manifest-path src-tauri/Cargo.toml
pnpm tauri build --bundles nsis
```

NSIS 安装包生成于 `target/release/bundle/nsis/`。不要同时运行开发版与安装版接收服务，它们需要相同的 DNS 和 RTMP 端口。

MediaMTX 固定为 v1.21.4。准备脚本下载官方 Windows x64 压缩包，并使用仓库中锁定的 SHA256 校验；不执行远程安装脚本。升级流程和 GitHub 发布流程见 [维护与发布](docs/releasing.md)。

## 许可

本项目使用 MIT 许可证。第三方组件信息见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。PlayStreamer 不属于 Sony、PlayStation、Twitch、Bilibili 或 OOPZ，图标为项目原创标识。
