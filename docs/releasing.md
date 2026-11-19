# 维护与发布

本文面向维护者，说明开发、检查、依赖和发布流程。用户连接步骤见[快速开始](quick-start.md)。

## 开发环境

| 依赖 | 要求 |
| --- | --- |
| 操作系统 | Windows x64，用于完整桌面开发和安装包构建 |
| Rust | stable 工具链，版本不小于 1.88，包含 rustfmt 和 Clippy |
| Windows 工具 | Visual Studio C++ Build Tools 和 Microsoft Edge WebView2 Runtime |
| Node.js | 版本不小于 22，持续集成使用 22 |
| pnpm | 版本见 `package.json` 的 `packageManager`，当前为 10.33.0 |
| MediaMTX | 由准备脚本下载，当前锁定 v1.21.4 |

终端命令均从仓库根目录运行。接收服务只支持 Windows。macOS 自动检查仅覆盖核心模块。

1. 安装锁文件指定的前端依赖。

   ```powershell
   pnpm install --frozen-lockfile
   ```

2. 准备接流组件。

   ```powershell
   node scripts/prepare-sidecar.mjs
   ```

3. 启动桌面开发环境。

   ```powershell
   pnpm tauri dev
   ```

`pnpm dev` 仅启动前端预览。托盘、真实接收和诊断依赖桌面进程。

警告：开发版与安装版使用相同接收端口。并行接收会产生冲突。同一时间只保留 1 个接收实例。

## 本地检查

前置条件：开发依赖和接流组件已准备完成。

1. 运行前端类型检查。

   ```powershell
   pnpm typecheck
   ```

2. 构建前端。

   ```powershell
   pnpm build
   ```

3. 运行发布流程检查。

   ```powershell
   node --test scripts/test-release.mjs
   ```

4. 运行网卡回归检查。

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-network-interfaces.ps1
   ```

5. 检查 Rust 格式。

   ```powershell
   cargo fmt --all --check
   ```

6. 运行核心静态检查。

   ```powershell
   cargo clippy --locked -p playstreamer-core --all-targets -- -D warnings
   ```

7. 运行核心测试。

   ```powershell
   cargo test --locked -p playstreamer-core
   ```

8. 运行桌面编译检查。

   ```powershell
   cargo check --locked --manifest-path src-tauri/Cargo.toml
   ```

## 合成媒体测试

默认核心测试会忽略媒体集成测试。显式运行需要 MediaMTX 和 FFmpeg。测试会使用 TCP 1935，不能与正在接收的客户端并行运行。

前置条件：FFmpeg 的实际 `ffmpeg.exe` 位于命令搜索路径中。Chocolatey 转发程序不满足这个条件。

1. 设置 MediaMTX 的程序路径。

   ```powershell
   $env:PLAYSTREAMER_MEDIAMTX = (Resolve-Path 'src-tauri/binaries/mediamtx-x86_64-pc-windows-msvc.exe').Path
   ```

2. 设置 FFmpeg 的程序路径。

   ```powershell
   $env:PLAYSTREAMER_FFMPEG = (Get-Command ffmpeg.exe -CommandType Application -ErrorAction Stop).Source
   ```

3. 运行媒体集成测试。

   ```powershell
   cargo test --locked -p playstreamer-core --test media_integration -- --ignored --test-threads=1
   ```

持续集成（Continuous Integration，CI）工作流通过 Chocolatey 安装 FFmpeg。工作流直接指定实际程序路径，避免遗留发布进程。FFmpeg 不随客户端分发。

## 依赖和资源

### MediaMTX

版本、下载文件名和 SHA256 摘要固定在 `scripts/sidecar-manifest.json`。`prepare-sidecar.mjs` 下载官方压缩包，校验摘要后提取程序和许可证。

| 路径 | 内容 |
| --- | --- |
| `.cache/mediamtx/` | 下载缓存，不提交到 Git |
| `src-tauri/binaries/` | 接流可执行文件，不提交到 Git |
| `src-tauri/resources/mediamtx-LICENSE` | 随应用分发的 MIT 许可证 |

警告：摘要失败表示下载内容与锁定文件不一致。直接采用失败文件的摘要会绕过校验。升级摘要必须来自上游正式校验文件。

1. 核对目标版本的协议和配置兼容性。
2. 从官方 Release 的 `checksums.sha256` 获取目标摘要。
3. 更新 `scripts/sidecar-manifest.json`。
4. 运行 `node scripts/prepare-sidecar.mjs`。
5. 完成核心检查。
6. 运行合成媒体测试。
7. 按[版本与验证](validation.md)验证真实接流。

### glib 安全补丁

Tauri 的 Linux 图形依赖需要 glib 0.18。根目录的 Cargo patch 指向 `vendor/glib-0.18.5`。安全补丁来源和移除条件见 [PLAYSTREAMER-PATCH.md](https://github.com/JxQg/PlayStreamer/blob/main/vendor/glib-0.18.5/PLAYSTREAMER-PATCH.md)。

CI 和 Release 在 Linux 优化构建中运行 `tools/glib-security` 回归测试。测试失败会阻止发布。Tauri 或图形依赖升级后，需要重新评估补丁是否仍有必要。

### 图标和字体

图标源文件为 `public/brand/playstreamer.svg`。`pnpm icons` 从源文件生成应用 PNG、ICO 和托盘 PNG。生产构建也会重新生成图标。

1. 修改源 SVG。
2. 运行 `pnpm icons`。
3. 检查 16 像素和 32 像素图标。

应用和教程图使用 Noto Sans SC Variable。前端通过 `@fontsource-variable/noto-sans-sc` 离线打包字体。许可证位于 `public/fonts/NotoSansSC-OFL.txt`。原生托盘菜单和系统对话框使用系统字体。

## 本地打包和校验

前置条件：

- 本地检查已通过。
- 当前版本的 `docs/releases/<版本>.md` 已存在。
- `target/release/bundle/nsis/` 仅包含本次版本的安装产物。
- `dist-release/` 不存在或为空。

1. 从项目配置读取版本。

   ```powershell
   $releaseVersion = (Get-Content -LiteralPath package.json -Raw | ConvertFrom-Json).version
   ```

2. 校验版本一致性。

   ```powershell
   node scripts/check-release-version.mjs ('v' + $releaseVersion)
   ```

3. 准备接流组件。

   ```powershell
   node scripts/prepare-sidecar.mjs
   ```

4. 构建 Windows 安装包。

   ```powershell
   pnpm tauri build --bundles nsis
   ```

5. 整理发布资产。

   ```powershell
   node scripts/package-release.mjs
   ```

6. 计算安装包摘要。

   ```powershell
   Get-FileHash -Algorithm SHA256 -LiteralPath ('.\dist-release\PlayStreamer_{0}_windows_x64_setup.exe' -f $releaseVersion)
   ```

7. 将结果与 `dist-release/SHA256SUMS` 比较。

安装包位于 `target/release/bundle/nsis/`。打包脚本要求目录中恰有 1 个 NSIS 安装包。脚本拒绝向非空 `dist-release/` 写入，避免混用历史产物。

发布资产固定为：

| 文件 | 用途 |
| --- | --- |
| `PlayStreamer_<版本>_windows_x64_setup.exe` | Windows x64 安装包 |
| `SHA256SUMS` | 安装包完整性摘要 |
| `RELEASE_NOTES.md` | 版本说明和安装条件 |

## 发布新版本

版本标签使用 `vMAJOR.MINOR.PATCH` 或 `vMAJOR.MINOR.PATCH-rc.N`。预发布标签包含 `-rc.N`。标签必须指向已提交的版本。

前置条件：GitHub 尚无同名 Release。

1. 更新根 `Cargo.toml` 的 workspace 版本。
2. 更新 `package.json` 的版本。
3. 更新 `src-tauri/tauri.conf.json` 的版本。
4. 更新锁文件。
5. 更新受行为变更影响的教程。
6. 在 `docs/releases/<版本>.md` 编写版本说明。
7. 按[版本与验证](validation.md)记录测试结果。
8. 运行[本地检查](#本地检查)。
9. 运行版本一致性检查。
10. 将版本变更提交并合并到 `main`。
11. 确认对应 GitHub CI 已通过。
12. 在版本提交上创建版本标签。
13. 将版本标签推送到 GitHub。
14. 检查 [Release 工作流](https://github.com/JxQg/PlayStreamer/actions/workflows/release.yml)。
15. 在工作流通过后核对公开资产。

Release 工作流校验标签、版本号和已有发布记录。Windows 任务构建安装包。macOS 检查核心模块。Linux 检查 glib 安全补丁。

全部任务通过后，工作流创建草稿并上传 3 个资产。上传完成后，工作流公开 Release。包含 `-rc.N` 的版本标记为预发布。

### 手动触发

前置条件：版本标签已存在，GitHub 尚无同名 Release。

1. 打开仓库的「Actions → Release」。
2. 点击「Run workflow」。
3. 在 `tag` 中填写已有版本标签。
4. 点击「Run workflow」提交任务。
5. 检查工作流结果和发布资产。

手动触发不会创建标签。工作流使用校验阶段解析的提交 SHA，上传前会再次核对标签。

### 发布失败

构建或检查失败时，工作流不会公开 Release。上传失败可能留下草稿。已有同名 Release 会阻止重试。

1. 检查失败任务的错误原文。
2. 检查是否遗留同名草稿或公开 Release。
3. 按失败原因修复构建、资产或草稿状态。
4. 确认版本标签仍指向预期提交。
5. 按[手动触发](#手动触发)重试。

当前安装包尚未签名，工作流不需要签名密钥。接入代码签名时，证书必须存入 GitHub Secrets。摘要和发布资产必须基于签名后的最终安装包。macOS 桌面打包、签名和公证尚未提供。

## 官网与操作文档

官网地址为 [jxqg.github.io/PlayStreamer](https://jxqg.github.io/PlayStreamer/)。`site/` 保存模板和样式，`docs/` 保存网页使用的 Markdown 源文档。`site/site.json` 定义站点地址、导航和文档路由。

前置条件：前端依赖已安装。

1. 修改 `docs/` 中对应的源文档。
2. 构建网站。

   ```powershell
   pnpm site:build
   ```

3. 检查网站链接和锚点。

   ```powershell
   pnpm site:check
   ```

4. 将变更提交并合并到 `main`。
5. 检查 [Website 工作流](https://github.com/JxQg/PlayStreamer/actions/workflows/pages.yml)。
6. 打开官网核对已发布页面。

产物位于 `site-dist/`，不提交到仓库。检查脚本覆盖本地链接、图片、脚本和锚点。客户端离线教程位于 `src/guides.ts`，行为变更需要同时核对教程内容。

GitHub 仓库「Settings → Pages」的构建来源为「GitHub Actions」。拉取请求只构建和检查。文档或网站源文件合并到 `main` 后，Website 工作流自动部署。

| 配置 | 默认值 |
| --- | --- |
| `BASE_PATH` | `/PlayStreamer/` |
| `SITE_URL` | `https://jxqg.github.io/PlayStreamer/` |

迁移域名或仓库名时，站点配置、工作流环境变量和仓库网站地址需要一致。部署结果见 Actions 记录和 `github-pages` 环境。
