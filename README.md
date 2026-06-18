# MTD Subtitle App

Rust 原生跨平台视频字幕工作台：选择视频，自动分离音频，调用 MOSS-Transcribe-Diarize 异步转写，生成 SRT/VTT 字幕，可选将字幕烧录回视频。

## 运行源码

```bash
cargo run
```

## 开发依赖

- Rust 1.93+
- ffmpeg，开发时需要能在命令行直接运行 `ffmpeg`，或设置 `FFMPEG_PATH`
- MOSS API Key
- HarmonyOS Sans SC Regular 字体文件，用于分发包内置 UI 字体

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

### 随包携带 ffmpeg

要做到跨平台随包携带 ffmpeg，不是把一个 ffmpeg 用在所有平台，而是为每个平台放入各自的 ffmpeg 二进制：

```text
vendor/ffmpeg/macos/ffmpeg
vendor/ffmpeg/linux/ffmpeg
vendor/ffmpeg/windows/ffmpeg.exe
```

然后在对应平台运行构建脚本。脚本会复制匹配平台的 ffmpeg：

- macOS：复制到 `MTDSubtitleApp.app/Contents/Resources/ffmpeg`
- Linux：复制到 `dist/linux/ffmpeg`
- Windows：复制到 `dist/windows/ffmpeg.exe`

应用启动后会优先找随包 ffmpeg；找不到时再找 `FFMPEG_PATH` 或系统 PATH。

注意：ffmpeg 的授权取决于你下载或编译的版本。很多静态构建是 LGPL，也有启用 GPL 组件的构建。正式商用分发前需要保留对应 license，并确认所选构建的授权要求。

### 随包携带鸿蒙字体

本项目要求分发包内置 HarmonyOS Sans SC Regular。把官方下载得到的字体文件放到：

```text
assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
```

然后运行平台构建脚本。脚本会在字体缺失时直接失败，避免误发没有字体或声明不完整的包。

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
```

这里记录了 HarmonyOS Sans、ffmpeg 和直接 Rust 依赖的声明。正式公开发布前，还应基于 `Cargo.lock` 生成完整的传递依赖 license 清单，并保留 ffmpeg 的版本、构建参数和授权文本。

## 输出

应用会在你选择的输出目录下创建 `MTD字幕-<timestamp>/`。

- `audio.m4a`：从视频分离出的音频
- `transcript.json`：模型返回的分段结果
- `subtitles.srt`：SRT 字幕
- `subtitles.vtt`：WebVTT 字幕
- `transcript.txt`：完整文本
- `subtitled.mp4`：勾选烧录字幕时生成

## 接口流程

1. 执行 `ffmpeg` 抽取音频。
2. `POST https://studio.mosi.cn/api/v1/files/upload` 上传音频。
3. `POST https://studio.mosi.cn/api/v1/asr/tasks` 创建 `moss-transcribe-diarize` 任务。
4. 轮询 `GET https://studio.mosi.cn/api/v1/asr/tasks/:task_id`，成功后生成字幕文件。
