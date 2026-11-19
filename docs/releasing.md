# 维护与发布

## 依赖和图标

MediaMTX 的版本、下载文件名和 SHA256 固定在 `scripts/sidecar-manifest.json`。Windows 运行 `node scripts/prepare-sidecar.mjs`，脚本从官方 GitHub Release 下载压缩包、校验锁定摘要，提取可执行文件和 MIT 许可证。下载缓存保存在 `.cache/mediamtx/`，可执行文件保存在 `src-tauri/binaries/`，两者不提交到 Git。

升级 MediaMTX 时，先核对上游版本与协议/配置兼容性，从该 Release 官方 `checksums.sha256` 取得目标文件的摘要，再更新 manifest。校验失败必须停止准备，不应通过改成实际下载摘要的方式绕过失败。完成核心测试和真实接流验证后再发布。许可证保存在 `src-tauri/resources/mediamtx-LICENSE`，随应用打包。

GTK 依赖暂时要求 glib 0.18，因此根 Cargo patch 使用 `vendor/glib-0.18.5` 中回移官方安全修复的源码；来源、补丁及移除条件见该目录的 `PLAYSTREAMER-PATCH.md`。CI 和 Release 都在 Linux 上以优化构建运行 `tools/glib-security` 的上游迭代器回归测试，失败会阻止发布。更新 Tauri/GTK 时应重新检查能否移除该补丁。

原创图标的唯一来源为 `public/brand/playstreamer.svg`。`pnpm icons` 使用同一 SVG 生成应用 PNG、多尺寸 ICO 和托盘 PNG，生产构建也会自动重新生成。修改图形时只修改源 SVG，并检查 16/32 像素下的识别度；托盘保持同一图形，运行状态由提示文字和菜单表达。

应用界面统一使用随安装包附带的 Noto Sans SC Variable，教程示意图继承同一字体。字体通过 `@fontsource-variable/noto-sans-sc` 随前端离线构建，许可证位于 `public/fonts/NotoSansSC-OFL.txt`。Windows 原生托盘菜单与系统对话框仍由系统绘制。

## 发布新版本

1. 同时更新根 `Cargo.toml` 的 workspace 版本、`package.json` 和 `src-tauri/tauri.conf.json`，并更新锁文件。
2. 更新实际行为对应的教程和验证记录，在 `docs/releases/<版本>.md` 编写该版本的变更说明；打包脚本将其加入 Release 正文。完成本地检查。
3. 提交后创建并推送已有提交上的版本标签，例如 `v0.1.0` 或 `v0.2.0-rc.1`。
4. GitHub Actions 的 Release 工作流自动校验标签格式、三个版本号一致、标签存在且尚无同名 Release。Windows 构建安装包，macOS 检查核心，Linux 验证 glib 安全补丁。
5. 所有任务通过后，工作流创建草稿、上传安装包和校验文件，再公开 Release。包含 `-rc.N` 的版本会标记为预发布。

也可以在 Actions 的 Release 页面手动运行 `workflow_dispatch`，填写已经存在的版本标签。不会创建新标签，也不会用用户输入拼接 shell 命令。构建使用校验时解析的提交 SHA；上传前再次确认标签没有移动。

发布资产为 `PlayStreamer_<版本>_windows_x64_setup.exe`、`SHA256SUMS` 和 `RELEASE_NOTES.md`。任何构建、检查或上传失败都不应公开一个缺少资产的 Release；上传失败可能留下草稿，维护者应检查并处理草稿后再重试。已经存在的 Release 不会被工作流覆盖。

当前工作流不包含代码签名，不需要签名密钥。正式分发前如接入签名，应将证书放入 GitHub Secrets，并在生成最终安装包后校验和发布；不可把私钥提交到仓库。macOS 完整应用、签名与公证属于后续平台工作，不由核心检查代替。

Windows CI 通过 Chocolatey 安装 FFmpeg，仅用于生成合成音视频验证接收、播放和会话恢复，不随客户端发布。真实 PS5 和平台验证仍需单独完成。

## 本地打包和校验

```powershell
node scripts/check-release-version.mjs v0.1.0
node scripts/prepare-sidecar.mjs
pnpm tauri build --bundles nsis
node scripts/package-release.mjs
Get-FileHash -Algorithm SHA256 .\dist-release\PlayStreamer_0.1.0_windows_x64_setup.exe
```

`package-release.mjs` 要求安装包目录中只有一个 NSIS 产物，且 `dist-release` 为空，避免把上次产物混入新版本。本地校验摘要应与 `SHA256SUMS` 中对应文件相同。
