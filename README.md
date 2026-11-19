# PlayStreamer

<img src="public/brand/playstreamer.svg" width="88" height="88" alt="PlayStreamer 图标" />

PlayStreamer 在 Windows 电脑上接收 PlayStation 5（PS5）广播。OBS Studio（下文简称 OBS）读取本地画面，再向 Bilibili 推流。OOPZ 通过分享 OBS 投影窗口接收画面。

[项目官网](https://jxqg.github.io/PlayStreamer/) · [操作文档](https://jxqg.github.io/PlayStreamer/guide/) · [下载 Windows 版](https://github.com/JxQg/PlayStreamer/releases/latest) · [反馈问题](https://github.com/JxQg/PlayStreamer/issues)

```text
PS5 广播 → PlayStreamer 本地接收 → OBS
                                  ├─ Bilibili 推流
                                  └─ 投影窗口 → OOPZ 分享
```

## 功能与限制

- 接收指定 PS5 的广播，提供固定的本地播放地址。
- 检查网卡、端口、防火墙和域名解析，导出脱敏诊断文件。
- 显示接收码率、连接时长、媒体轨道和读取客户端数。
- 关闭窗口后驻留系统托盘，支持从托盘控制接收。
- 提供离线教程，使用随安装包附带的字体和接流组件。

当前正式版本为 [v0.1.1](https://github.com/JxQg/PlayStreamer/releases/tag/v0.1.1)。本地使用已完成测试。安全修复见[版本说明](docs/releases/0.1.1.md)。

Windows x64 安装包尚未签名。macOS 仅检查核心模块兼容性，尚无桌面安装包。

PS5 仍需访问 PlayStation Network（PSN）和 Twitch。账号关联和开播授权由平台完成。发布状态和设备回归要求见[版本与验证](docs/validation.md)。

## 使用条件

| 项目 | 条件 |
| --- | --- |
| 电脑 | Windows 10 22H2 或 Windows 11，x64 架构 |
| 运行组件 | Microsoft Edge WebView2 Runtime。缺少时，安装程序需要联网下载 |
| 局域网 | PS5 与电脑位于同一 IPv4 子网，建议使用网线 |
| 账号 | PS5 能登录 PSN、关联 Twitch 并发起广播 |
| 转播软件 | 同一台电脑安装 OBS。按用途准备 Bilibili 开播权限或 OOPZ 分享权限 |

PlayStreamer 使用域名系统（Domain Name System，DNS）解析直播入口。媒体接收使用实时消息传输协议（Real-Time Messaging Protocol，RTMP）。

## 首次使用

完整步骤见[快速开始](docs/quick-start.md)。

1. 从 [GitHub Releases](https://github.com/JxQg/PlayStreamer/releases/latest) 下载 Windows x64 安装包。
2. 按[快速开始](docs/quick-start.md)完成安装和连接向导。
3. 点击「开启接收」。
4. 按 [PS5 设置](docs/ps5-setup.md)配置 DNS 和 Twitch 广播。
5. 按 [OBS 与转播平台设置](docs/obs-platforms.md)添加媒体源。

同机 OBS 的媒体源「输入」使用固定地址：

```text
rtmp://127.0.0.1:1935/live/playstation
```

「正在接收」表示应用已收到媒体流。「正在读取」只表示读取客户端数，不识别具体播放器。平台是否收到音画，需要在观看设备上确认。

警告：完全退出、休眠和关机会停止 DNS。PS5 DNS 仍指向电脑时，主机可能无法解析域名。退出顺序见[停止和恢复](docs/ps5-setup.md#停止和恢复)。

## 文档导航

| 文档 | 内容 |
| --- | --- |
| [快速开始](docs/quick-start.md) | 安装、首次连接、OBS 接入和日常启停 |
| [PS5 设置](docs/ps5-setup.md) | DNS、账号关联、代理共存和网络配置修改 |
| [OBS 与转播平台设置](docs/obs-platforms.md) | 媒体源、Bilibili 推流和 OOPZ 音画分享 |
| [故障排查](docs/troubleshooting.md) | 诊断顺序、端口、黑屏、音频和反馈信息 |
| [版本与验证](docs/validation.md) | 正式版本、自动检查范围和设备回归要求 |
| [维护与发布](docs/releasing.md) | 开发环境、依赖维护、安装包和官网发布 |

电脑上的 TUN 虚拟网卡或虚拟专用网络（VPN）可与接流共存。接流使用物理局域网网卡。代理条件见 [PS5 设置](docs/ps5-setup.md#与-tunvpn-和现有代理共存)。

## 开发

项目使用 Rust、Tauri 2、React、TypeScript 和 MediaMTX。Windows 开发环境和检查命令见[维护与发布](docs/releasing.md#开发环境)。

前置条件：开发依赖已安装，终端位于仓库根目录。

1. 安装前端依赖。

   ```powershell
   pnpm install --frozen-lockfile
   ```

2. 准备随应用打包的接流组件。

   ```powershell
   node scripts/prepare-sidecar.mjs
   ```

3. 启动桌面开发环境。

   ```powershell
   pnpm tauri dev
   ```

`pnpm dev` 仅预览前端界面。真实接收、托盘和诊断依赖 Tauri 桌面进程。

警告：开发版与安装版使用相同的 DNS 和 RTMP 端口。并行接收会产生端口冲突。同一时间只保留 1 个接收实例。

## 许可

项目使用 [MIT 许可证](LICENSE)。第三方组件信息见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。

PlayStreamer 与 Sony、PlayStation、Twitch、Bilibili 和 OOPZ 无隶属关系。项目图标为原创标识。
