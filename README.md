<div align="center">

# dWarp

**Bring any AI model into your terminal (CPU Rendered)**

dWarp is a fork of [OpenWarp](https://github.com/zerx-lab/warp) (a community fork of [Warp](https://github.com/warpdotdev/warp)) with an opt-in CPU renderer on Linux. It draws frames with `softbuffer` instead of wgpu, which saves VRAM on machines whose GPU memory is needed for other tasks like AI models or gaming.
Keep the full Warp terminal experience — blocks, workflows, keymaps — while plugging in
**any OpenAI-compatible provider**, customizing system prompts with minijinja templates,
and keeping every credential local.

[简体中文](./README.zh-CN.md) · [Docs](https://docs.warp.dev) · [Upstream Warp](https://www.warp.dev)

> ⚠️ Early development. No official release yet. **Not affiliated with Warp, Inc.**

### 🖥️ CPU Rendering
GPU rendering remains the default. **Use CPU renderer** selects Warp's CPU rasterizer with `softbuffer`, without allocating a GPU rendering device for the window. Enable it under Settings → Features → System → Use CPU renderer, then restart Warp. The desktop compositor may still use the GPU.

The shared Winit CPU path is implemented for Linux/FreeBSD and Windows. Windows native validation is pending. Native macOS CPU presentation is not implemented yet.

While the CPU renderer is active, frames are paced to save CPU: 15 fps for steady terminal output, 30 fps while typing or clicking, 60 fps for short bursts such as resizing and scrolling. Override with environment variables:

```bash
WARP_CPU_RENDERER=1 dwarp          # one-off CPU renderer
WARP_CPU_FPS=30 dwarp              # one cap for every frame class
WARP_CPU_TERMINAL_FPS=10 dwarp     # steady-output cap only
```
Also available: `WARP_CPU_IDLE_FPS`, `WARP_CPU_INTERACTIVE_FPS`, `WARP_CPU_BURST_FPS`.

</div>

---

## ✨ Why dWarp

The official Warp client routes AI through Warp's cloud agent service.
dWarp opens that layer entirely:

| | Upstream Warp | dWarp |
| --- | --- | --- |
| AI provider | Warp gateway | **Any OpenAI-compatible endpoint** |
| Credentials | Cloud account | **Local config file, never leaves device** |
| System prompt | Server-assembled, opaque | **minijinja templates, fully editable** |
| UI language | English | **Native English + Simplified Chinese, extensible** |
| Cloud Agent / Computer Use | On by default | **Off by default, fully local** |
| Blocks / Workflows / Keymaps | ✓ | ✓ Fully preserved |
| License | AGPL-3.0 / MIT dual | Same as upstream |

## 🚀 Three steps to take AI fully into your own hands

**01 · Plug in any provider**
Paste a Base URL and API key in settings — any OpenAI Chat Completions–compatible
endpoint works out of the box. Credentials are stored locally only.

**02 · Author dynamic prompts**
A minijinja-powered template engine renders the system prompt in real time
based on the current working directory, language, and role.

**03 · Use it in the terminal immediately**
Switch models, conversations, and command suggestions with one click —
the experience is identical to Warp, but every layer is yours.

## 🧩 Verified providers

| Provider | Base URL | Notes |
| --- | --- | --- |
| **OpenAI** | `https://api.openai.com/v1` | Native protocol |
| **Anthropic** | via genai native | Claude 4.x family |
| **DeepSeek** | `https://api.deepseek.com/v1` | thinking + tool calling |
| **Gemini** | via genai native | Google AI Studio |
| **Ollama** | `http://localhost:11434/v1` | Local inference, no key |
| **OpenRouter** | `https://openrouter.ai/api/v1` | Aggregator gateway |
| **Qwen / Groq / Together / LM Studio / any OpenAI-compatible proxy** | — | Configure and go |

## 🔧 Core features

- **BYOP custom providers** — five native protocols (OpenAI / OpenAIResp / Anthropic / Gemini / Ollama / DeepSeek) explicitly bound on top of [genai](https://github.com/jeremychone/rust-genai) 0.6
- **SSE streaming** — incremental block rendering identical to Warp's first-party path
- **18 local tools** — shell / read / edit / search / mcp / drive docs / skills / ask, all executed locally
- **System prompt templates** — eight model-family prompts ported from opencode (default / anthropic / gpt / beast / gemini / kimi / codex / trinity)
- **CPU renderer** — opt-in `softbuffer` rendering with frame pacing. Windows code is included, with native validation pending.
- **models.dev integration** — searchable Providers subpage with thousands of preloaded model entries
- **Privacy first** — Cloud Agent / Computer Use / Referral disabled by default; no telemetry
- **Warp experience preserved** — continuously merged with upstream; Blocks, Workflows, AI commands, Keymaps and themes all kept
- **Localized UI** — Simplified Chinese + English, community-extensible

## 📦 Build from source

```bash
git clone https://github.com/IceFog72/warp-cpu.git
cd warp-cpu
./script/bootstrap   # platform-specific deps
./script/run         # build & run
./script/presubmit   # fmt / clippy / tests
```

If you prefer raw `cargo`, **always target the OSS binary explicitly**:

```bash
cargo build --release --bin dwarp
cargo run   --release --bin dwarp
```

To build the optimized `release-lto` binary and install it system-wide, use [`script/build-and-install.sh`](script/build-and-install.sh). It installs the OSS launcher, compatibility symlink, desktop entry, and icons; `sudo` is used for the system install:

```bash
./script/build-and-install.sh oss
# Equivalent form:
./script/build-and-install.sh --channel oss
```

The script also accepts `dev` or `local` channels. The OSS launcher is installed as `dwarp`.

On Windows, run [`script/build-and-install.bat`](script/build-and-install.bat) to build the OSS installer and open its installation wizard. Use `-BuildOnly` to produce the installer without installing it. Prerequisites and options are in [Windows build instructions](docs/windows-build.md).

> ⚠️ Do not run `cargo build --release` / `cargo run --release --bin {warp,stable,dev,preview}`
> without a filter — those entry points (`local.rs` / `stable.rs` / `dev.rs` / `preview.rs`) load
> their channel config through Warp's private `warp-channel-config` binary, which lives in a
> closed-source repo. Compilation succeeds, but the resulting executables panic at startup
> asking you to run `./script/install_channel_config`. That script clones an SSH repo only
> Warp employees can access. dWarp users only need the `dwarp` binary.

See [WARP.md](WARP.md) for the full engineering guide (style, testing, platform notes).

## 📜 License

Same as upstream Warp:

- `warpui_core` / `warpui` crates — [MIT](LICENSE-MIT)
- Everything else — [AGPL-3.0](LICENSE-AGPL)

## 🌿 Branches & upstream sync

`zerx-lab/warp` keeps two long-lived branches:

| Branch | Tracks | Purpose |
| --- | --- | --- |
| `main` | `zerx-lab/warp:main` (default) | dWarp's main development line. **All PRs target this.** |
| `warp-upstream` | `warpdotdev/warp:master` | Pristine mirror of upstream Warp, used to pull in new commits. **No fork-local changes.** |

**For contributors**

Open PRs against **`main`**. Never against `warp-upstream`.

**For maintainers (write access)**

⚠️ **Do not click the "Sync fork" button** on `main` in the GitHub web UI. It would merge the entire upstream history straight into dWarp's main line and trigger large-scale conflicts. Pull upstream changes through the mirror branch instead:

```bash
# one-time setup
git remote add upstream https://github.com/warpdotdev/warp.git

# refresh the mirror
git checkout warp-upstream
git pull                          # fast-forwards from upstream/master
git push origin warp-upstream

# bring selected commits into main
git checkout main
git cherry-pick <sha>             # or merge warp-upstream when a full sync makes sense
```

## 🤝 Contributing

Community contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full flow.

Before filing, please [search existing issues](https://github.com/zerx-lab/warp/issues).
Security vulnerabilities should be reported privately per
[CONTRIBUTING.md#reporting-security-issues](CONTRIBUTING.md#reporting-security-issues).

## 🙏 Acknowledgements

dWarp stands on the shoulders of the Warp team and many open-source projects:

[Warp](https://github.com/warpdotdev/warp) · [genai](https://github.com/jeremychone/rust-genai) · [opencode](https://github.com/opencode-ai/opencode) · [models.dev](https://models.dev) · [Tokio](https://github.com/tokio-rs/tokio) · [NuShell](https://github.com/nushell/nushell) · [Alacritty](https://github.com/alacritty/alacritty) · [Hyper](https://github.com/hyperium/hyper) · [minijinja](https://github.com/mitsuhiko/minijinja)

## Uninstall

See [uninstall instructions](docs/uninstall.md) for Linux, Windows, and macOS.
Use `script/uninstall.sh --purge` on Linux/macOS or
`script\uninstall.bat -Purge` on Windows for complete current-user data removal.
Use the documented dry-run option first to inspect the affected files.
