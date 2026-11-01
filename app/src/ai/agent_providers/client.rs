//! genai client construction for BYOP providers.

use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

use crate::settings::AgentProviderApiType;

/// 把 `AgentProviderApiType` 一对一映射到 genai `AdapterKind`。
fn adapter_kind_for(api_type: AgentProviderApiType) -> AdapterKind {
    match api_type {
        AgentProviderApiType::OpenAi => AdapterKind::OpenAI,
        AgentProviderApiType::OpenAiResp => AdapterKind::OpenAIResp,
        AgentProviderApiType::Gemini => AdapterKind::Gemini,
        AgentProviderApiType::Anthropic => AdapterKind::Anthropic,
        AgentProviderApiType::Ollama => AdapterKind::Ollama,
        AgentProviderApiType::DeepSeek => AdapterKind::DeepSeek,
    }
}

/// 规范化用户填写的 `base_url`,产出供 genai adapter 拼接 service path 的 endpoint URL。
///
/// genai 0.6.x 所有 adapter 都假设 endpoint 以 `/` 结尾、且已经包含版本路径段:
/// - Anthropic:`format!("{base_url}messages")` 期望 `…/v1/`
/// - Gemini:`format!("{base_url}models/{m}:streamGenerateContent")` 期望 `…/v1beta/`
/// - OpenAI / OpenAIResp / DeepSeek:`Url::join("chat/completions" 或 "responses")` 期望 `…/v1/`
/// - Ollama:`format!("{base_url}api/chat")` 期望根路径 `…/`
///
/// 用户实际三种填法:
/// 1. 纯 host(`https://ai.zerx.dev`)— 早期默认行为只补尾 `/` 会拼成 `https://ai.zerx.dev/messages`
///    漏掉 `/v1/` 导致 404。**这里按 api_type 自动追加默认版本路径段**(Anthropic/OpenAI 系→`/v1/`,
///    Gemini→`/v1beta/`,Ollama 不补)。
/// 2. 完整带版本路径(`https://ai.zerx.dev/v1`)— 仅补尾 `/`,不动 path。
/// 3. 留空 — 用 [`AgentProviderApiType::default_base_url`]。
fn normalize_endpoint_url(api_type: AgentProviderApiType, base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return api_type.default_base_url().to_owned();
    }

    // 解析失败(用户填了畸形 URL)→ 退化到原"补尾 /"行为,让上游报错而不是这里 panic。
    let parsed = match url::Url::parse(trimmed) {
        Ok(u) => u,
        Err(_) => {
            let stripped = trimmed.trim_end_matches('/');
            return format!("{stripped}/");
        }
    };

    // path == "/" 或为空 → 用户只填了 host,自动补上 api_type 默认版本路径段。
    if parsed.path() == "/" || parsed.path().is_empty() {
        // 从 default_base_url 抽 path 部分(如 "/v1/" / "/v1beta/" / "/")。
        let default_path = url::Url::parse(api_type.default_base_url())
            .ok()
            .map(|u| u.path().to_owned())
            .unwrap_or_else(|| "/".to_owned());
        let host_part = trimmed.trim_end_matches('/');
        return format!("{host_part}{default_path}");
    }

    // 用户已自带 path → 仅确保尾随 `/`(genai format!/Url::join 都依赖)。
    let stripped = trimmed.trim_end_matches('/');
    format!("{stripped}/")
}

/// 构造 genai Client。每次请求新建(开销低 — Client 内部只是 reqwest::Client + adapter 表)。
/// `ServiceTargetResolver` capture 当前请求的 endpoint/key/api_type 后,把每次 exec_chat_stream
/// 都强制路由到指定 AdapterKind,完全绕过 genai 默认的"按模型名识别"。
pub(super) fn build_client(
    api_type: AgentProviderApiType,
    base_url: String,
    api_key: String,
) -> Client {
    let adapter_kind = adapter_kind_for(api_type);
    let endpoint_url = normalize_endpoint_url(api_type, &base_url);
    log::info!("[byop] build_client: adapter={adapter_kind:?} endpoint_url={endpoint_url}");
    let key_for_resolver = api_key.clone();
    let resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let ServiceTarget { model, .. } = service_target;
            let endpoint = Endpoint::from_owned(endpoint_url.clone());
            let auth = AuthData::from_single(key_for_resolver.clone());
            // 用我们指定的 AdapterKind 覆盖 genai 的"按模型名"识别结果,
            // 但保留 model_name 以便上游服务正确寻址模型。
            let model = ModelIden::new(adapter_kind, model.model_name);
            Ok(ServiceTarget {
                endpoint,
                auth,
                model,
            })
        },
    );
    Client::builder()
        .with_service_target_resolver(resolver)
        .build()
}
