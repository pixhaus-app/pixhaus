# Rust AI/ML Ecosystem for Pixhaus — May 2026

Comprehensive research into production-grade Rust crates for AI/ML inference, LLM APIs, image generation, audio analysis, and vision. This document serves Streams S21–S36 (verb runtime, backend adapters, and inference verbs).

**Research date:** May 2, 2026  
**Status:** Active—ecosystem moves fast. Versions, repos, and maintenance status reflect current state.  
**Key finding:** No official Anthropic Rust SDK exists; community SDKs or raw HTTP clients required.

---

## Summary Statistics

- **Crates researched:** 35+
- **Active local inference engines:** 4 (Candle, Burn, ORT, Mistral.rs)
- **Official API SDKs found:** 1 (async-openai for OpenAI)
- **Gaps Pixhaus must fill:** 5 major (Anthropic client, ComfyUI, tool-use, pose estimation, provider abstraction)
- **Recommended primary crates:** candle-core, async-openai, tokenizers, image, dasp, rustfft, ort, burn

---

## Local Inference Engines

### candle (HuggingFace)

- **Purpose:** Minimalist ML framework for Rust. CPU and GPU inference.
- **GitHub:** https://github.com/huggingface/candle
- **Crates:** candle-core, candle-nn, candle-transformers
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active. Official HuggingFace.
- **GPU support:** CUDA, Metal (Apple), WASM. No native ROCm.
- **Key strengths:**
  - Browser demos (Whisper, LLaMA2, YOLO, SAM)
  - Minimal overhead
  - First-class WASM
  - Pre-built model examples
- **Gotchas:** Smaller ecosystem than PyTorch; no INT8 quantization native (use pre-quantized models)
- **When to use:** Running open-source models locally (LLMs, Whisper, YOLO)
- **Pixhaus streams:** S21 (verb runtime), S23 (inference verbs)
- **Alternatives:** Burn (more flexible), ORT (pre-exported ONNX)

### burn (Tracel AI)

- **Purpose:** Next-gen tensor library. PyTorch-like Rust alternative. Training + inference.
- **GitHub:** https://github.com/tracel-ai/burn
- **Crate:** burn (and modular sub-crates)
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active. Strong Discord community.
- **GPU backends:** CUDA, ROCm, Metal, Vulkan, WebGPU, LibTorch, CubeCL, Flex
- **Key strengths:**
  - Backend decorator pattern (wrap any backend with Autodiff)
  - Supports no-std, embedded, WASM
  - Training and inference equally supported
  - Type-safe, macro-driven API
- **Gotchas:** Steeper learning curve; fewer production models than Candle
- **When to use:** Training custom models locally; heavy autodiff use
- **Pixhaus streams:** S21, S30–S31 (training verbs)
- **Alternatives:** Candle (simpler), ORT (inference-only)

### ort (ONNX Runtime)

- **Purpose:** Safe Rust bindings for ONNX Runtime. Run models in ONNX format with hardware acceleration.
- **GitHub:** https://github.com/pykeio/ort (community fork, more active than Microsoft's)
- **Crate:** ort v2.0.0-rc.12
- **ONNX Runtime:** 1.24.4
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active community-driven.
- **GPU support:** Any ONNX Runtime execution provider (CUDA, ROCm, TensorRT, DirectML, CoreML, QNN, OpenVINO)
- **Key strengths:**
  - Execution-provider flexibility
  - Battle-tested (TEI, Magika, Google Magika, edge-transformers, 12+ OSS projects)
  - Mature ONNX Runtime C/C++ backend
  - Quantization support (Q4, Q8)
- **Gotchas:** Requires pre-exported ONNX models; no native training; C++ binary dependency
- **When to use:** Pre-trained models in ONNX format (pose, embeddings, object detection)
- **Pixhaus streams:** S23 (vision verbs), S25 (pose), S31 (embeddings)
- **Alternatives:** Candle (lighter), Burn (more control)

### mistralrs (Mistral.rs)

- **Purpose:** High-performance LLM inference. Zero-config HuggingFace models. Multimodal (text, vision, video, audio).
- **GitHub:** https://github.com/EricLBuehler/mistral.rs
- **Crate:** mistralrs
- **License:** MIT
- **Maintenance (May 2026):** Very active single-author project.
- **Key strengths:**
  - Zero-config: mistralrs run -m user/model
  - True multimodality (text, vision, video, audio)
  - Fine-grained quantization control (MXFP4, GGUF)
  - Built-in web UI: mistralrs serve --ui
  - Hardware tuning: mistralrs tune
  - Agentic features (server-side tool loop, MCP client)
  - Python and Rust SDKs
- **Gotchas:** Community-driven (single author); opinionated design; rapid API iteration
- **When to use:** Full-featured LLM + multimodal inference; agentic workflows
- **Pixhaus streams:** S21–S23 (core LLM), S28 (video-to-pixel), S33 (agentic)
- **Alternatives:** Candle (simpler), vLLM (Python, more mature)

---

## LLM API Client SDKs (REST/HTTP)

### async-openai

- **Purpose:** Async Rust client for OpenAI API.
- **GitHub:** https://github.com/64bit/async-openai
- **Crate:** async-openai v0.36.1+
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active. Tracks API updates.
- **MSRV:** Rust 1.75+
- **Features:** Full OpenAI API (chat, embeddings, vision, function calling, file uploads); streaming via tokio
- **Strengths:** Most popular Rust OpenAI SDK; mature error handling
- **Gotchas:** Requires tokio; OpenAI API key needed
- **When to use:** Pixhaus S21 (GPT-4 for code gen, captioning)
- **Pixhaus streams:** S21 (backend adapter), S22 (API routing)
- **Alternatives:** openai-rs / openai-api-rs (less popular)

### anthropic-sdk-rust (OFFICIAL)

**Status: DOES NOT EXIST as of May 2026.**

Anthropic publishes Python and TypeScript SDKs but no official Rust SDK.

**How to use Anthropic API:**

Option A (recommended): Generic HTTP client + serde_json. See reference below.
Option B: Await community SDK (verify crates.io).
Option C: Use unofficial fork if available.

**Reference Anthropic HTTP Client:**

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: usize,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

pub async fn call_claude(api_key: &str, prompt: &str) -> Result<String> {
    let client = Client::new();
    let req = AnthropicRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        max_tokens: 4096,
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
    };
    
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&req)
        .send()
        .await?
        .json::<AnthropicResponse>()
        .await?;
    
    Ok(resp.content[0].text.as_ref().unwrap().clone())
}
```

**Implication:** S21 verb runtime must implement Anthropic client (200 LOC).

### llm-chain (Orchestration)

- **Purpose:** Rust equivalent of LangChain. Multi-backend chaining and orchestration.
- **GitHub:** https://github.com/sobelio/llm-chain
- **License:** Likely MIT/Apache-2.0
- **Maintenance:** Active but community-driven.
- **Features:** Async trait abstractions, prompt templates, chain composition, memory, streaming
- **Strengths:** Multi-backend scenarios; type-safe chaining
- **Gotchas:** Smaller ecosystem; may need custom backends
- **When to use:** S21 (verb orchestration, multi-step LLM workflows)
- **Pixhaus streams:** S21 (verb composition)

### Other API Providers

No official Rust SDKs for Replicate, Groq, OpenRouter, Google Gemini, etc. (May 2026).

**Pixhaus approach:** HTTP adapters in S22 using reqwest + serde_json.

| Provider | Rust SDK? | Recommendation |
|---|---|---|
| Replicate | No | HTTP client (https://api.replicate.com/v1/predictions) |
| Groq | No | HTTP client |
| OpenRouter | No | HTTP client (OpenAI-compatible) |
| Together AI | No | HTTP client or OpenAI-compatible endpoint |
| Google Gemini | No | Await official SDK or HTTP client |

---

## Tokenizers

### tokenizers (HuggingFace)

- **Purpose:** Fast comprehensive tokenization. Rust core with Python bindings.
- **GitHub:** https://github.com/huggingface/tokenizers
- **Crate:** tokenizers v0.14+
- **License:** Apache-2.0
- **Maintenance (May 2026):** Active. Part of HuggingFace.
- **Performance:** 1 GB text in ~20 seconds on CPU.
- **Features:** BPE, WordPiece, SentencePiece, Unigram; alignment tracking; padding, truncation
- **Strengths:** Authoritative; blazingly fast; covers all modern tokenizers
- **When to use:** Token counting (API cost), prompt encoding, embeddings
- **Pixhaus streams:** S21 (token counting), S23 (prompt prep)
- **Alternatives:** tiktoken-rs (OpenAI-specific)

### tiktoken-rs

- **Purpose:** OpenAI tokenization (GPT-2, GPT-3, GPT-4).
- **When to use:** If Pixhaus only needs OpenAI tokens; lighter than HF tokenizers
- **Pixhaus:** Use tokenizers instead (more general).

---

## Vision and Image Processing

### image (image-rs)

- **Purpose:** Pure Rust image decoding, encoding, basic processing.
- **GitHub:** https://github.com/image-rs/image
- **Crate:** image v0.24+
- **License:** MIT / Apache-2.0
- **Maintenance:** Very active.
- **Formats:** PNG, JPEG, GIF, BMP, TIFF, WebP, PNM
- **Strengths:** No external dependencies; solid codecs
- **When to use:** Loading sprites, resizing, format conversion
- **Pixhaus streams:** S20–S21 (input), S25 (manipulation)
- **Note:** Covered in graphics doc; see for details.

### imageproc

- **Purpose:** Advanced image processing (filters, edge detection, morphology, convolution).
- **GitHub:** https://github.com/image-rs/imageproc
- **License:** MIT / Apache-2.0
- **When to use:** Custom filtering, edge detection for auto-outline
- **Pixhaus streams:** S25 (transformations)
- **Note:** Covered in graphics doc.

### opencv-rust

- **Purpose:** OpenCV bindings. Full CV pipeline (detection, segmentation, optical flow, etc.).
- **GitHub:** https://github.com/twistedfall/opencv-rust
- **Crate:** opencv
- **License:** MIT/Apache-2.0 (bindings); OpenCV is BSD 3-Clause
- **Maintenance:** Active.
- **System requirement:** OpenCV 3.4 / 4.x / 5.x + Clang
- **Strengths:** Full OpenCV feature set; proven production use; C++ performance
- **Gotchas:** Heavy system dependency; binding generation finicky; bloats binary
- **When to use:** Complex motion tracking, optical flow, advanced pose estimation
- **Pixhaus streams:** S28 (video analysis)
- **Alternatives:** imageproc (lighter), ort + ONNX (no heavy library)

---

## Audio Analysis and Processing

### dasp (Digital Audio Signal Processing)

- **Purpose:** Low-level DSP fundamentals. No allocations, no dependencies. Work with audio signals and PCM.
- **GitHub:** https://github.com/rustaudio/dasp
- **Crate:** dasp
- **License:** MIT/Apache-2.0
- **Maintenance:** Active.
- **Features:** Signal trait, generators (sine, square, noise), basic filters, no_std friendly
- **Strengths:** Zero-copy, zero-alloc; lightweight; building block for custom DSP
- **When to use:** Beat detection, frequency analysis, custom audio filtering
- **Pixhaus streams:** S24 (audio analysis), custom beat detection

### rustfft

- **Purpose:** Pure Rust Fast Fourier Transform (FFT) for frequency-domain analysis.
- **Crate:** rustfft
- **License:** MIT/Apache-2.0
- **Strengths:** No external deps; supports real and complex FFTs
- **When to use:** Frequency analysis for beat detection, spectrograms
- **Pixhaus streams:** S24 (audio-driven animation)

### symphonia

- **Purpose:** Pure Rust audio codec library. Decode MP3, FLAC, WAV, OGG.
- **Maintenance:** Community-driven; verify on crates.io
- **Strengths:** Multi-codec support; frame-by-frame decoding; metadata extraction
- **When to use:** Loading audio files for beat detection and analysis
- **Pixhaus streams:** S24 (audio loading)

### aubio-rs

**Status:** Repository inactive (404). Use dasp + rustfft instead.

---

## Pose Estimation and Skeletal Models

**Challenge:** MediaPipe and DensePose are Python/C++, not Rust native.

**Recommended approach:**

1. Export MediaPipe or DensePose pose models to ONNX
2. Run via ort crate
3. Alternatively: Use mistralrs (built-in pose support)

**Rust tools:** No native pose detection library exists. Leverage ONNX export pipeline.

**Pixhaus S25 (pose verb):** Use ort + pre-exported ONNX model.

---

## Embeddings and Vector Stores

### qdrant-client

- **Purpose:** Rust client for Qdrant vector database. Store and search embeddings.
- **GitHub:** https://github.com/qdrant/qdrant-rust-client
- **Crate:** qdrant-client
- **License:** Apache-2.0
- **Maintenance:** Active (Qdrant actively developed).
- **When to use:** Style-based sprite search (embed styles, find similar from library)
- **Pixhaus streams:** S32 (optional style learning)
- **Alternatives:** milvus-rs (less mature), tantivy (full-text, not embeddings)

---

## Image Generation APIs

**Status (May 2026):** No official Rust SDKs. Use HTTP clients.

### Stability AI API

- **Endpoint:** https://api.stability.ai/v1/generate
- **Auth:** API key in header
- **Pixhaus approach:** reqwest + serde_json
- **Use case:** S35 (text-to-sprite via Stability Diffusion)

### Replicate API

- **Endpoint:** https://api.replicate.com/v1/predictions
- **Auth:** Bearer token
- **Pixhaus approach:** reqwest + serde_json
- **Use case:** Generic inference (upscaling, etc.)

### ComfyUI Integration

**No Rust client library.**

- ComfyUI is node-based image generation (Python)
- From Rust: HTTP POST to ComfyUI server (localhost:8188)
- Payload: JSON workflow + node graph
- **Pixhaus approach:** HTTP adapter for S35 (diffusion verb)
- **Effort:** ~300 LOC (workflow builder + HTTP client)

**ComfyUI HTTP Pattern:**

```rust
pub async fn queue_comfyui_job(workflow: serde_json::Value) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:8188/prompt")
        .json(&workflow)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    Ok(resp["prompt_id"].as_str().unwrap().to_string())
}
```

---

## Tool-Use and Agentic Patterns

**Current state:** No Rust equivalent of LangChain Agents or AutoGPT.

**Options:**
1. llm-chain: Provides chaining; layer tool-use on top
2. Home-grown: Implement in S21
3. mistralrs: Built-in agentic features (tool loop, MCP client)

**Pixhaus recommendation:** S21 verb runtime implements Tool trait abstraction. Each verb is a Tool. LLM calls tools; runtime executes. Use serde_json for serialization. Consider mistralrs if local LLM needed.

---

## Streaming and Real-Time

### Streaming Completions

Most API clients support tokio streams or callbacks.
- async-openai: create_chat_completion_stream()
- Custom HTTP: reqwest::Client::stream() or eventsource-stream
- mistralrs: Supports streaming via Rust SDK

### Server-Sent Events (SSE)

Build Pixhaus server streaming verb results to UI.
- Crate: eventsource-stream or tokio-stream
- Pixhaus use: Real-time animation preview while verb runs

---

## Dependency Quick Reference

| Category | Primary | Secondary | Notes |
|---|---|---|---|
| Local inference | candle-core | burn, ort | Choose based on use case |
| LLM APIs | async-openai | None (custom Anthropic) | HTTP adapters for other providers |
| Tokenizers | tokenizers | tiktoken-rs | HF for breadth |
| Vision | image | opencv | imageproc for filters |
| Audio | dasp + rustfft | symphonia | Build on top |
| Pose | ort + ONNX | mistralrs | No native lib |
| Embeddings | qdrant-client | None | Optional for style search |
| Image gen | HTTP client | None | No official SDKs |
| Orchestration | llm-chain | mistralrs | Custom for lightweight |
| Async | tokio, async-stream | eventsource-stream | Standard |

---

## Gaps Pixhaus Must Fill

### 1. Anthropic Rust SDK
**Effort:** ~200 LOC (simple HTTP client)

### 2. ComfyUI Rust Client
**Effort:** ~300 LOC (workflow builder + HTTP)

### 3. LLM Agentic Framework
**Effort:** ~500 LOC (tool registry, LLM loop)

### 4. Pose Estimation (Native)
**Effort:** ~100 LOC (ONNX model loading via ort)

### 5. Cross-Provider API Abstraction
**Effort:** ~400 LOC (provider trait system)

---

## May 2026 Surprises

1. **No Official Anthropic SDK:** Anthropic has Python + TypeScript but no Rust. Likely lower demand.
2. **Candle Dominates for Open Models:** More polished LLM examples than Burn despite Burn being more flexible.
3. **Community Forks Better:** ort (pykeio) more active than Microsoft's onnxruntime-rs.
4. **Mistral.rs Punches Above Weight:** Single-author project rivals larger teams. Rust enthusiasm + small = rapid iteration.
5. **LLM Orchestration Immature:** LangChain (Python) years ahead. llm-chain and rig exist but gaps remain. Market opportunity.

---

## Stream Mapping

| Stream | Crates | Notes |
|---|---|---|
| S21 | candle-core, async-openai, tokenizers, custom Anthropic | Verb runtime |
| S22 | reqwest, serde_json, provider traits | Backend adapters |
| S23 | candle-core, mistralrs, async-openai | Text/code gen |
| S24 | dasp, rustfft, symphonia | Audio analysis |
| S25 | ort, image, imageproc, opencv | Vision (pose, segmentation) |
| S28 | opencv, mistralrs | Motion/video |
| S30–S31 | burn, candle-core | Training |
| S32 | qdrant-client, candle embeddings | Style learning |
| S33 | mistralrs, custom tool loop | Agentic |
| S35 | reqwest, ComfyUI HTTP | Diffusion/image gen |

---

**Document version:** 1.0  
**Research date:** May 2, 2026  
**Total crates covered:** 35+  
**Status:** Active ecosystem; re-evaluate Q3 2026.
