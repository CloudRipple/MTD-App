# MTD Subtitle App

Rust 原生跨平台字幕工作台：选择音频或视频，调用 MOSS-Transcribe-Diarize 异步转写，生成 SRT/VTT 字幕；视频输入可选将字幕添加回视频。

## 运行源码

```bash
cargo run
```

## 开发依赖

- Rust 1.93+
- ffmpeg，开发时需要能在命令行直接运行 `ffmpeg`，或设置 `FFMPEG_PATH`
- MOSS API Key
- HarmonyOS Sans SC Regular 字体文件和随包许可，用于分发包内置 UI 字体

ffmpeg 查找顺序：

1. `FFMPEG_PATH` 环境变量
2. 应用同目录的 `ffmpeg` 或 `ffmpeg.exe`
3. macOS `.app` 内的 `Contents/Resources/ffmpeg`
4. 系统 PATH

## 编译分发

Rust 通常在当前系统构建当前系统的产物：

- 在 macOS 上构建 macOS 应用
- 在 Windows 上构建 Windows 应用
- 在 Linux 上构建 Linux 应用

通用构建：

```bash
cargo build --release
```

正式分发请使用平台脚本，因为脚本会检查并复制字体、声明文件和可选 ffmpeg。

平台脚本：

```bash
scripts/build-macos.sh
scripts/build-linux.sh
```

Windows PowerShell：

```powershell
scripts\build-windows.ps1
```

构建产物位于：

- `dist/macos/MTDSubtitleApp.app`
- `dist/linux/MTDSubtitleApp`
- `dist/windows/MTDSubtitleApp.exe`

这些平台脚本生成的产物都面向直接打开应用窗口：

- macOS 使用 `.app` bundle
- Windows release 使用 GUI subsystem，不会额外弹出控制台窗口
- Linux 同时生成 `MTDSubtitleApp.desktop`，其中 `Terminal=false`

macOS 直接双击/`open` 启动需要通过 Apple 认可的代码签名身份。构建脚本支持：

```bash
MACOS_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" scripts/build-macos.sh
```

如果没有设置 `MACOS_CODESIGN_IDENTITY`，脚本会使用 ad-hoc 签名作为开发产物。某些较严格的 macOS 安全策略会拒绝通过 LaunchServices 直接启动 ad-hoc 或未知证书签名的 `.app`；正式分发需要 Developer ID 签名，并按发布渠道完成 notarization。

### 随包携带 ffmpeg

FFmpeg 源码作为 git submodule 管理：

```text
vendor/ffmpeg-src
```

首次拉取仓库后先初始化 submodule：

```sh
git submodule update --init --depth 1 vendor/ffmpeg-src
```

分发脚本会在打包前从该源码编译当前平台的 ffmpeg，并输出到：

```text
vendor/ffmpeg/macos/ffmpeg
vendor/ffmpeg/linux/ffmpeg
vendor/ffmpeg/windows/ffmpeg.exe
```

然后脚本会复制匹配平台的 ffmpeg：

- macOS：复制到 `MTDSubtitleApp.app/Contents/Resources/ffmpeg`
- Linux：复制到 `dist/linux/ffmpeg`
- Windows：复制到 `dist/windows/ffmpeg.exe`

应用启动后会优先找随包 ffmpeg；找不到时再找 `FFMPEG_PATH` 或系统 PATH。

默认构建参数使用 LGPL-compatible 配置：`--pkg-config-flags=--static --disable-gpl --disable-nonfree --enable-libass`。`libass` 是必须项，因为“添加字幕到视频”使用 FFmpeg 的 `subtitles` filter。

macOS 打包脚本还会收集 Homebrew/本地前缀中的 FFmpeg 运行时 dylib 依赖，复制到 `.app/Contents/Resources/lib`，改写为 `@rpath` 路径并重新签名，避免用户机器上必须安装 Homebrew。

构建依赖：

- macOS：`brew install libass pkg-config`
- Linux：安装 `pkg-config` 和 `libass-dev`/`libass-devel`
- Windows：使用 MSYS2 MinGW，安装 `mingw-w64-x86_64-libass` 和 `pkgconf`，必要时设置 `MSYS2_BASH`

脚本会把 FFmpeg 的 license 文件和 `BUILD_INFO.txt` 复制到分发包的 `legal/ffmpeg` 目录。正式商用分发前仍需基于实际构建参数复核 LGPL/GPL 要求。

### 随包携带鸿蒙字体

本项目要求分发包内置 HarmonyOS Sans SC Regular。仓库中的字体来自华为官方 HarmonyOS 设计资源页下载的字体包，并保留了随包许可文件：

```text
assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt
```

运行平台构建脚本时，脚本会在字体或许可缺失时直接失败，避免误发没有字体或声明不完整的包。

字体会被复制到：

- macOS：`MTDSubtitleApp.app/Contents/Resources/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- Linux：`dist/linux/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- Windows：`dist/windows/fonts/HarmonyOS_Sans_SC_Regular.ttf`

应用启动时会优先加载随包字体，也支持 `HARMONYOS_FONT_PATH` 用于开发调试。

### 第三方声明

分发包会包含：

```text
NOTICE.md
THIRD_PARTY_NOTICES.md
HarmonyOS_Sans_SC_LICENSE.txt
```

这里记录了 HarmonyOS Sans、ffmpeg 和直接 Rust 依赖的声明。正式公开发布前，还应基于 `Cargo.lock` 生成完整的传递依赖 license 清单，并保留 ffmpeg 的版本、构建参数和授权文本。

## 输出

应用会在你选择的输出目录下创建 `MTD字幕-<timestamp>/`。

支持的视频和音频输入：

- 视频：MP4、MOV、MKV、WebM、M4V、AVI
- 纯音频：WAV、MP3、AAC、FLAC、OGG、MPEG、M4A、MP4 音频流、WebM 音频流、PCM

- `audio.m4a`：从视频分离出的音频，纯音频输入不会额外生成此文件
- `transcript.json`：模型返回的分段结果
- `subtitles.srt`：SRT 字幕
- `subtitles.vtt`：WebVTT 字幕
- `transcript.txt`：完整文本
- `subtitled.mp4`：视频输入选择“添加字幕到视频”后生成，纯音频输入不支持此操作

## 接口流程

1. 视频输入先执行 `ffmpeg` 抽取音频，纯音频输入直接上传。
2. `POST https://studio.mosi.cn/api/v1/files/upload` 上传音频。
3. `POST https://studio.mosi.cn/api/v1/asr/tasks` 创建 `moss-transcribe-diarize` 任务。
4. 轮询 `GET https://studio.mosi.cn/api/v1/asr/tasks/:task_id`，成功后生成字幕文件。
