//! In BYOP mode, infer supported multimodal attachment types from `api_type` and `model_id`.
//!
//! genai 0.6 的 `ContentPart::Binary` 在线协议层全自动适配(参见
//! `chat_stream.rs` 注释表格):
//! - OpenAI: image→`image_url{data:URL}`,pdf/file→`type:"file" file_data:data:URL`,audio→`input_audio`
//! - Anthropic: image→`image base64`,其它→`document base64`(实际仅 PDF 有效)
//! - Gemini: 全部走 `inline_data`
//!
//! **Wire protocol support** does not mean **model support**. This checks what the model can consume,
//! so images are not sent to text-only models such as GPT-3.5 or Claude Sonnet 1.0.
//!
//! 判定走 model_id 子串匹配,与 `prompt_renderer::resolve_template` 风格对齐。
//! Substring matching is intentionally broad. It covers future minor releases in the same family
//! instead of enumerating exact versions, favoring lower maintenance cost over precision.

use super::models_dev;
use crate::settings::{AgentProviderApiType, AgentProviderModel};

/// 一个模型对附件类型的支持能力表。
#[derive(Debug, Clone, Copy, Default)]
pub struct AttachmentCaps {
    /// Whether image input (image/* MIME) is supported.
    pub images: bool,
    /// Whether PDF input (application/pdf MIME) is supported.
    pub pdf: bool,
    /// Whether audio input (audio/* MIME) is supported.
    pub audio: bool,
}

impl AttachmentCaps {}

/// Resolve a model's final capabilities, including the user's three-state override.
/// Priority: explicit settings override, models.dev catalog inference, then fallback rules.
/// 3. catalog miss → substring fallback
///
/// `provider_id` enables exact catalog matching for aggregate providers such as OpenRouter.
/// Fallback rules do not need it when the catalog has no match.
pub fn resolve_for_model(
    provider_id: &str,
    api_type: AgentProviderApiType,
    model: &AgentProviderModel,
) -> AttachmentCaps {
    let inferred = if let Some(c) = models_dev::lookup_caps(provider_id, &model.id) {
        AttachmentCaps {
            images: c.vision,
            pdf: c.pdf,
            audio: c.audio,
        }
    } else {
        caps_for_by_substring(api_type, &model.id)
    };
    AttachmentCaps {
        images: model.image.unwrap_or(inferred.images),
        pdf: model.pdf.unwrap_or(inferred.pdf),
        audio: model.audio.unwrap_or(inferred.audio),
    }
}

/// Return inferred capabilities for the UI, ignoring user overrides and using catalog/fallback data.
/// Chip tooltips use this to explain the automatic result.
pub fn inferred_for_model(
    provider_id: &str,
    api_type: AgentProviderApiType,
    model_id: &str,
) -> AttachmentCaps {
    if let Some(c) = models_dev::lookup_caps(provider_id, model_id) {
        AttachmentCaps {
            images: c.vision,
            pdf: c.pdf,
            audio: c.audio,
        }
    } else {
        caps_for_by_substring(api_type, model_id)
    }
}

/// 按 (api_type, model_id 子串) 兜底查表。
///
/// Unknown models conservatively return all false. This avoids sending binary data to unsupported
/// models and getting a 400 response. New models must be added here, along with other model metadata.
fn caps_for_by_substring(api_type: AgentProviderApiType, model_id: &str) -> AttachmentCaps {
    let lower = model_id.to_ascii_lowercase();
    match api_type {
        AgentProviderApiType::OpenAi | AgentProviderApiType::OpenAiResp => {
            // GPT-4o / 4.1 / 5 families: image + PDF. The 3.5 family is text-only.
            if lower.contains("gpt-4o")
                || lower.contains("gpt-4.1")
                || lower.contains("gpt-5")
                || lower.contains("o1")
                || lower.contains("o3")
                || lower.contains("o4")
            {
                AttachmentCaps {
                    images: true,
                    pdf: true,
                    audio: false,
                }
            } else if lower.contains("gpt-4o-audio") || lower.contains("gpt-realtime") {
                AttachmentCaps {
                    images: true,
                    pdf: true,
                    audio: true,
                }
            } else {
                AttachmentCaps::default()
            }
        }
        AgentProviderApiType::Anthropic => {
            // All Claude 3 / 3.5 / 4 / 4.5 / 4.7 models support vision + documents (PDF).
            if lower.contains("claude-3")
                || lower.contains("claude-4")
                || lower.contains("claude-opus")
                || lower.contains("claude-sonnet")
                || lower.contains("claude-haiku")
            {
                AttachmentCaps {
                    images: true,
                    pdf: true,
                    audio: false,
                }
            } else {
                AttachmentCaps::default()
            }
        }
        AgentProviderApiType::Gemini => {
            // All Gemini 1.5+ / 2 / 2.5 models are multimodal. inline_data supports image/PDF/audio/video.
            if lower.contains("gemini-1.5")
                || lower.contains("gemini-2")
                || lower.contains("gemini-pro-vision")
            {
                AttachmentCaps {
                    images: true,
                    pdf: true,
                    audio: true,
                }
            } else {
                AttachmentCaps::default()
            }
        }
        AgentProviderApiType::Ollama => {
            // Most Ollama models are text-only. Enable image support for vision models by model_id
            // substring (LLaVA / bakllava / llama3.2-vision / qwen2-vl / minicpm-v / moondream).
            // PDF/audio are generally unavailable through the Ollama protocol, so return false.
            let vision = lower.contains("llava")
                || lower.contains("bakllava")
                || lower.contains("vision")
                || lower.contains("-vl")
                || lower.contains("minicpm-v")
                || lower.contains("moondream");
            AttachmentCaps {
                images: vision,
                pdf: false,
                audio: false,
            }
        }
        AgentProviderApiType::DeepSeek => {
            // Current public DeepSeek models (v3/r1/coder/chat) are text-only.
            // Enable this if a deepseek-vl family becomes available.
            if lower.contains("vl") {
                AttachmentCaps {
                    images: true,
                    pdf: false,
                    audio: false,
                }
            } else {
                AttachmentCaps::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_4o_supports_image_and_pdf() {
        // Use fallback rules when the test environment has not loaded the models.dev catalog.
        let caps = caps_for_by_substring(AgentProviderApiType::OpenAi, "gpt-4o-2024-08-06");
        assert!(caps.images);
        assert!(caps.pdf);
        assert!(!caps.audio);
    }

    #[test]
    fn openai_3_5_text_only() {
        let caps = caps_for_by_substring(AgentProviderApiType::OpenAi, "gpt-3.5-turbo");
        assert!(!caps.images && !caps.pdf && !caps.audio);
    }

    #[test]
    fn claude_sonnet_supports_image_and_pdf() {
        let caps = caps_for_by_substring(AgentProviderApiType::Anthropic, "claude-sonnet-4-5");
        assert!(caps.images);
        assert!(caps.pdf);
    }

    #[test]
    fn gemini_2_5_full_multimodal() {
        let caps = caps_for_by_substring(AgentProviderApiType::Gemini, "gemini-2.5-pro");
        assert!(caps.images);
        assert!(caps.pdf);
        assert!(caps.audio);
    }

    #[test]
    fn ollama_default_text_only() {
        let caps = caps_for_by_substring(AgentProviderApiType::Ollama, "qwen2.5:7b");
        assert!(!caps.images && !caps.pdf && !caps.audio);
    }

    #[test]
    fn ollama_vision_models_get_images() {
        let caps = caps_for_by_substring(AgentProviderApiType::Ollama, "llava:13b");
        assert!(caps.images);
        assert!(!caps.pdf);
    }

    #[test]
    fn deepseek_chat_text_only() {
        let caps = caps_for_by_substring(AgentProviderApiType::DeepSeek, "deepseek-chat");
        assert!(!caps.images && !caps.pdf && !caps.audio);
    }
}
