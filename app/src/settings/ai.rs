//! Settings for Blocklist AI.
//!
//! These settings are currently used to configure the underlying model/API used to power the AI
//! UX, as well as small UX configurations.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::report_if_error;
use crate::terminal::CLIAgent;
use regex::Regex;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, UpdateModel};

use settings::{
    define_settings_group, RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud,
};
use warp_core::execution_mode::AppExecutionMode;

use serde::{de::Deserializer, Deserialize, Serialize};
use strum_macros::EnumIter;

pub enum FocusedTerminalInfoEvent {
    TerminalInfoUpdated,
}

/// Singleton model that is used to track the remote sessions in the terminal.
/// Useful for organizations that have restrictions on using AI in sessions in
/// remote sessions.
#[derive(Default, Clone, Debug)]
pub struct FocusedTerminalInfo {
    contains_any_remote_blocks: bool,
    contains_any_restored_remote_blocks: bool,
}

impl FocusedTerminalInfo {
    pub fn new(_: &mut ModelContext<Self>) -> Self {
        Self {
            contains_any_remote_blocks: false,
            contains_any_restored_remote_blocks: false,
        }
    }

    pub fn contains_any_remote_blocks(&self) -> bool {
        self.contains_any_remote_blocks
    }

    pub fn contains_any_restored_remote_blocks(&self) -> bool {
        self.contains_any_restored_remote_blocks
    }

    /// Updates both remote blocks and restored blocks status in a single atomic operation.
    /// Only emits a TerminalInfoUpdated event if either value changes.
    /// Returns true if the event was emitted.
    pub fn update(
        &mut self,
        contains_any_remote_blocks: bool,
        contains_any_restored_remote_blocks: bool,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let remote_changed = self.contains_any_remote_blocks != contains_any_remote_blocks;
        let restored_changed =
            self.contains_any_restored_remote_blocks != contains_any_restored_remote_blocks;

        if remote_changed || restored_changed {
            self.contains_any_remote_blocks = contains_any_remote_blocks;
            self.contains_any_restored_remote_blocks = contains_any_restored_remote_blocks;
            ctx.emit(FocusedTerminalInfoEvent::TerminalInfoUpdated);
            return true;
        }

        false
    }
}

impl Entity for FocusedTerminalInfo {
    type Event = FocusedTerminalInfoEvent;
}

impl SingletonEntity for FocusedTerminalInfo {}

/// The default mode for new terminal sessions.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Default mode for new sessions.",
    rename_all = "snake_case"
)]
pub enum DefaultSessionMode {
    /// New sessions start in the terminal mode (default).
    #[default]
    Terminal,
    /// New sessions open a user-defined tab config.
    /// The specific config is identified by the companion `default_tab_config_path` setting.
    TabConfig,
}

settings::macros::implement_setting_for_enum!(
    DefaultSessionMode,
    AISettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "general.default_session_mode",
    description: "The default mode for new terminal sessions.",
);

impl DefaultSessionMode {
    /// Display name for the settings dropdown.
    pub fn display_name(&self) -> &'static str {
        match self {
            DefaultSessionMode::Terminal => "Terminal",
            DefaultSessionMode::TabConfig => "Tab Config",
        }
    }
}

// ---------------------------------------------------------------------------
// 自定义 Agent 提供商配置(进程内 Provider)
// ---------------------------------------------------------------------------

/// Agent 提供商支持的协议类型。
///
/// 第一阶段仅支持 OpenAI 兼容协议(适用于 OpenAI、DeepSeek、智谱 GLM、
/// Moonshot、通义千问 DashScope-OpenAI 兼容端点、SiliconFlow、OpenRouter、
/// 任何 OpenAI 兼容的本地服务等)。后续可在此扩展 Anthropic、Google、Bedrock。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderKind {
    /// OpenAI 兼容的 Chat Completions / `/v1/models` 协议。
    OpenAiCompatible,
}

impl Default for AgentProviderKind {
    fn default() -> Self {
        Self::OpenAiCompatible
    }
}

/// BYOP provider 实际使用的 API 协议类型 — 显式指定,
/// 由 chat_stream 通过 genai `ServiceTargetResolver` 一对一映射到对应的
/// `AdapterKind`,完全绕过"按模型名识别"的默认行为,避免误识别。
///
/// **注意**:这是相对 [`AgentProviderKind`] 的更细粒度维度。
/// `AgentProviderKind` 目前只有 `OpenAiCompatible`(语义"用户自管 endpoint"),
/// `AgentProviderApiType` 决定 genai 用哪种原生协议序列化请求 / 解析响应。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderApiType {
    /// OpenAI Chat Completions(`POST /v1/chat/completions`)。
    /// 适用于:OpenAI 官方、DeepSeek、SiliconFlow、OpenRouter、智谱 GLM、
    /// Moonshot、DashScope-OpenAI 兼容、本地 vLLM/llama.cpp 等。
    OpenAi,
    /// OpenAI Responses API(`POST /v1/responses`)。
    /// 适用于:GPT-5 / Codex / Pro 等较新模型。
    OpenAiResp,
    /// Google Gemini 原生协议(generativelanguage.googleapis.com)。
    Gemini,
    /// Anthropic Messages API 原生协议(api.anthropic.com)。
    Anthropic,
    /// Ollama 原生协议(本地或自建 Ollama)。
    Ollama,
    /// DeepSeek 原生协议。与 OpenAI 兼容相比:多轮 thinking 模式必须把
    /// `reasoning_content` 字段带回服务端(否则 400),仅 genai DeepSeek
    /// adapter 处理这个非标字段。`deepseek-reasoner / deepseek-v4-flash` 等
    /// thinking-mode 模型必须选这个类型,普通 chat 模型(`deepseek-chat`)
    /// 选 OpenAI 也可以工作。
    DeepSeek,
}

impl Default for AgentProviderApiType {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// Provider 级别的 reasoning effort(思考深度)偏好。
///
/// 语义说明:
/// - `Auto`(默认):不向 genai 传 effort。OpenAI / Anthropic adapter 内部会按
///   模型名后缀(`-low` / `-high` / `-zero` 等)自动推断;Gemini / DeepSeek 不推断。
/// - `Off`:对支持 reasoning 的模型显式发送 `none`,关闭思考链。
/// - 其他档位:client 端先用 `reasoning::model_supports_reasoning` 判定,**仅在该
///   模型支持时**注入,避免向 claude-3-5-haiku / gpt-4o / gemini-1.5-pro 等老模型
///   注入 thinking 参数被上游 400 拒绝。
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffortSetting {
    #[default]
    Auto,
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ReasoningEffortSetting {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Off => "Off",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "XHigh",
            Self::Max => "Max",
        }
    }

    /// 转成 genai `ReasoningEffort`。`Auto` 返回 None(调用方不要 set)。
    pub fn to_genai(self) -> Option<genai::chat::ReasoningEffort> {
        use genai::chat::ReasoningEffort as GE;
        Some(match self {
            Self::Auto => return None,
            Self::Off => GE::None,
            Self::Minimal => GE::Minimal,
            Self::Low => GE::Low,
            Self::Medium => GE::Medium,
            Self::High => GE::High,
            Self::XHigh => GE::XHigh,
            Self::Max => GE::Max,
        })
    }
}

impl AgentProviderApiType {
    /// 设置 UI dropdown 显示文字。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::OpenAiResp => "OpenAI-Response",
            Self::Gemini => "Gemini",
            Self::Anthropic => "Anthropic",
            Self::Ollama => "Ollama",
            Self::DeepSeek => "DeepSeek",
        }
    }

    /// 反向解析 Debug 格式名(`OpenAi` / `DeepSeek` 等),用于 BYOPLastUsedReasoningMap
    /// 这种 `<api_type>:<model_id>` 复合 key 的 hydrate 场景。未知字符串返回 None。
    pub fn from_debug_str(s: &str) -> Option<Self> {
        Some(match s {
            "OpenAi" => Self::OpenAi,
            "OpenAiResp" => Self::OpenAiResp,
            "Gemini" => Self::Gemini,
            "Anthropic" => Self::Anthropic,
            "Ollama" => Self::Ollama,
            "DeepSeek" => Self::DeepSeek,
            _ => return None,
        })
    }

    /// 当用户没填 base_url 时使用的默认 endpoint。新建 provider / 切换 ApiType
    /// 时,UI 可调用此方法预填,便于新手。
    ///
    /// **必须以 `/` 结尾**:genai 0.6.x 的 adapter 内部用 `format!("{base_url}messages")` /
    /// `Url::join` 拼接 service path,缺尾随 `/` 会拼出乱地址(Anthropic 是 `.devmessages` 直接连)
    /// 或被 `Url::join` 吃掉 path 最后一段。client 端 `build_client` 也会兜底补 `/`,
    /// 这里依然要求显式尾随 `/`,保证 UI 预填值落到 settings.toml 后即使绕过 client 兜底也是对的。
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1/",
            Self::OpenAiResp => "https://api.openai.com/v1/",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/",
            Self::Anthropic => "https://api.anthropic.com/v1/",
            Self::Ollama => "http://localhost:11434/v1/",
            Self::DeepSeek => "https://api.deepseek.com/v1/",
        }
    }
}

/// 一条用户自定义的 Agent 提供商配置。
///
/// `api_key` **不**保存在这里,而是保存在 `AgentProviderSecrets` 单例(secure storage),
/// 通过 `id` 关联。这样设置文件 (settings.toml) 不会泄漏敏感信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentProvider {
    /// 提供商唯一 ID,首次创建时生成,持久化到设置中作为 secret 的关联键。
    #[serde(default = "AgentProvider::default_id")]
    pub id: String,

    /// 用户给这个提供商起的显示名(例如 "DeepSeek 官方"、"本地 Ollama")。
    pub name: String,

    /// 协议类型,目前固定为 OpenAI 兼容(语义"用户自管 endpoint")。
    /// 实际请求/响应序列化协议由 [`AgentProvider::api_type`] 决定。
    #[serde(default)]
    pub kind: AgentProviderKind,

    /// 显式指定的 API 协议类型(OpenAI / OpenAI-Response / Gemini / Anthropic / Ollama)。
    /// 老配置(无此字段)反序列化为 `OpenAi` 兼容老语义。
    #[serde(default)]
    pub api_type: AgentProviderApiType,

    /// API base URL,例如 `https://api.deepseek.com/v1`、`http://localhost:11434/v1`。
    /// 不要带尾随斜杠,但代码侧会做容错。
    pub base_url: String,

    /// 用户配置的、希望暴露给 Agent 选择的模型列表。
    /// 每条同时含 `name`(显示名) 与 `id`(发送给上游 API 的 model 字段值)。
    #[serde(default)]
    pub models: Vec<AgentProviderModel>,
}

impl AgentProvider {
    fn default_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// 构造一个新的、空的提供商。
    pub fn new_empty() -> Self {
        Self {
            id: Self::default_id(),
            name: String::new(),
            kind: AgentProviderKind::default(),
            api_type: AgentProviderApiType::default(),
            base_url: String::new(),
            models: Vec::new(),
        }
    }
}

impl settings_value::SettingsValue for AgentProvider {}

/// 单条模型条目: `name` 是用户在 model picker 中看到的显示名,
/// `id` 是真正发给上游 OpenAI 兼容 API 的 `model` 字段值。
///
/// 序列化为 toml 时形如:
/// ```toml
/// [[agent_providers.models]]
/// name = "DS V3 通用"
/// id   = "deepseek-chat"
/// ```
///
/// 反序列化兼容老格式 `models = ["deepseek-chat", "deepseek-coder"]`
/// (每个字符串视为 `{ name = id, id = id }`),便于现有用户无痛升级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AgentProviderModel {
    pub name: String,
    pub id: String,

    /// 上下文窗口(tokens)。来源:用户填或 models.dev 自动带入。
    /// 0 表示未知 — chat_stream 退化到不做主动截断,完全交给上游服务报错。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub context_window: u32,

    /// 单次最大输出 tokens。0 表示未指定。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub max_output_tokens: u32,

    /// 是否支持 reasoning(思考/CoT)输出。
    #[serde(default, skip_serializing_if = "is_false")]
    pub reasoning: bool,

    /// 是否支持 function/tool calling。
    /// 默认 `true` — 老配置升级 + 用户手填新 model 时不要默认禁工具,
    /// 不支持工具调用的模型由 models.dev 数据带入显式 false。
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub tool_call: bool,

    // ----- 多模态附件 capability,三态语义:
    // - `None`(toml 字段缺省)= Auto: 运行时按 models.dev catalog → substring fallback 推断
    // - `Some(true)` = Force-On: 用户强制开,绕过推断
    // - `Some(false)` = Force-Off: 用户强制关
    //
    // 字段命名故意用 `image` 而非 `vision`,跟 models.dev `modalities.input: ["image"]`
    // 字面对应,语义最窄不歧义(避免用户误以为 vision = image+pdf+...)。
    /// 是否支持图像输入(image/* MIME)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
    /// 是否支持 PDF 文档输入(application/pdf)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<bool>,
    /// 是否支持音频输入(audio/* MIME)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn is_true(v: &bool) -> bool {
    *v
}
fn default_true() -> bool {
    true
}

impl AgentProviderModel {
    pub fn from_id(id: String) -> Self {
        Self {
            name: id.clone(),
            id,
            context_window: 0,
            max_output_tokens: 0,
            reasoning: false,
            tool_call: true,
            image: None,
            pdf: None,
            audio: None,
        }
    }
}

impl<'de> Deserialize<'de> for AgentProviderModel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Plain(String),
            Full {
                #[serde(default)]
                name: String,
                id: String,
                #[serde(default)]
                context_window: u32,
                #[serde(default)]
                max_output_tokens: u32,
                #[serde(default)]
                reasoning: bool,
                #[serde(default = "default_true")]
                tool_call: bool,
                #[serde(default)]
                image: Option<bool>,
                #[serde(default)]
                pdf: Option<bool>,
                #[serde(default)]
                audio: Option<bool>,
            },
        }
        match Either::deserialize(deserializer)? {
            Either::Plain(id) => Ok(AgentProviderModel::from_id(id)),
            Either::Full {
                name,
                id,
                context_window,
                max_output_tokens,
                reasoning,
                tool_call,
                image,
                pdf,
                audio,
            } => {
                let name = if name.is_empty() { id.clone() } else { name };
                Ok(AgentProviderModel {
                    name,
                    id,
                    context_window,
                    max_output_tokens,
                    reasoning,
                    tool_call,
                    image,
                    pdf,
                    audio,
                })
            }
        }
    }
}

impl settings_value::SettingsValue for AgentProviderModel {}

/// Keys are regex patterns (insertion-ordered), values are serialized CLIAgent names (e.g. "Claude").
/// An empty string value means "Any CLI Agent" (CLIAgent::Unknown).
///
/// Uses `IndexMap` to preserve insertion order so the settings UI list is deterministic.
/// Supports backward-compatible deserialization from the legacy `Vec<String>` format,
/// where each string is converted to a key with an empty agent value.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolbarCommandMap(IndexMap<String, String>);

impl ToolbarCommandMap {
    pub(crate) fn new(map: IndexMap<String, String>) -> Self {
        Self(map)
    }
}

impl<'de> Deserialize<'de> for ToolbarCommandMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum MapOrVec {
            Map(IndexMap<String, String>),
            Vec(Vec<String>),
        }

        match MapOrVec::deserialize(deserializer) {
            Ok(MapOrVec::Map(map)) => Ok(ToolbarCommandMap::new(map)),
            Ok(MapOrVec::Vec(vec)) => {
                let map = vec
                    .into_iter()
                    .map(|pattern| (pattern, String::new()))
                    .collect();
                Ok(ToolbarCommandMap::new(map))
            }
            Err(e) => Err(e),
        }
    }
}

impl schemars::JsonSchema for ToolbarCommandMap {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ToolbarCommandMap")
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        gen.subschema_for::<HashMap<String, String>>()
    }
}

impl std::ops::Deref for ToolbarCommandMap {
    type Target = IndexMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl settings_value::SettingsValue for ToolbarCommandMap {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or_default()
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        // Try map format first (using from_value to preserve insertion order), then legacy array format.
        if value.is_object() {
            if let Ok(map) = serde_json::from_value::<IndexMap<String, String>>(value.clone()) {
                return Some(ToolbarCommandMap::new(map));
            }
        }
        if let Some(arr) = value.as_array() {
            let result: IndexMap<String, String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| (s.to_string(), String::new())))
                .collect();
            return Some(ToolbarCommandMap::new(result));
        }
        None
    }
}

/// 持久化记忆"上次某 (api_type, model) 用过的 reasoning effort 档位"。
/// key 形式:`<api_type>:<model_id>`,例如 `DeepSeek:deepseek-v4-pro`。
/// value 是 `ReasoningEffortSetting` 枚举(serde_json snake_case)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BYOPLastUsedReasoningMap(pub IndexMap<String, ReasoningEffortSetting>);

impl BYOPLastUsedReasoningMap {
    pub fn new(map: IndexMap<String, ReasoningEffortSetting>) -> Self {
        Self(map)
    }

    /// 拼 key:`<api_type>:<model_id>`。api_type 用 Debug 拼出 `DeepSeek` 等驼峰名,
    /// 跟 ReasoningEffortSetting 的 serde 形式无关。
    pub fn make_key(api_type: AgentProviderApiType, model_id: &str) -> String {
        format!("{api_type:?}:{model_id}")
    }
}

impl std::ops::Deref for BYOPLastUsedReasoningMap {
    type Target = IndexMap<String, ReasoningEffortSetting>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl schemars::JsonSchema for BYOPLastUsedReasoningMap {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "BYOPLastUsedReasoningMap".into()
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        gen.subschema_for::<HashMap<String, String>>()
    }
}

impl settings_value::SettingsValue for BYOPLastUsedReasoningMap {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or_default()
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        if value.is_object() {
            if let Ok(map) =
                serde_json::from_value::<IndexMap<String, ReasoningEffortSetting>>(value.clone())
            {
                return Some(Self::new(map));
            }
        }
        None
    }
}

define_settings_group!(AISettings, settings: [
    // If `false`, all AI features are disabled.
    is_any_ai_enabled: IsAnyAIEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: false,
        toml_path: "agents.warp_agent.is_any_ai_enabled",
        description: "Controls whether all AI features are enabled.",
    },
    // This field should not be referenced directly to lookup active AI enablement -- use the
    // `is_active_ai_enabled()` getter.
    is_active_ai_enabled_internal: IsActiveAIEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: false,
        toml_path: "agents.warp_agent.active_ai.enabled",
        description: "Controls whether proactive AI features like suggestions are enabled.",
    },
    // This field should not be referenced directly to lookup intelligent autosuggestion enablement
    // -- use the `is_intelligent_autosuggestions_enabled()` getter.
    intelligent_autosuggestions_enabled_internal: IntelligentAutosuggestionsEnabled {
        type: bool,
        default: true, // TODO(roland): revisit this when launched to stable
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.intelligent_autosuggestions_enabled",
        description: "Controls whether AI-powered intelligent autosuggestions are enabled.",
    }

    // Whether to use locally loaded AWS credentials for Bedrock-enabled requests.
    aws_bedrock_credentials_enabled: AwsBedrockCredentialsEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_credentials_enabled",
        description: "Whether Warp should use your local AWS credentials for Bedrock-enabled requests.",
    }
    // Whether to automatically run the AWS login command when Bedrock credentials are expired.
    //
    // When true, the configured login command will be run automatically without asking.
    // When false (default), a prompt will be shown asking for permission.
    aws_bedrock_auto_login: AwsBedrockAutoLogin {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_auto_login",
        description: "Whether to automatically run the AWS login command when Bedrock credentials expire.",
    }
    // Command to run to refresh AWS credentials when using Bedrock auto-login.
    aws_bedrock_auth_refresh_command: AwsBedrockAuthRefreshCommand {
        type: String,
        default: "aws login".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_auth_refresh_command",
        description: "The command to run to refresh AWS credentials for Bedrock.",
    }
    // AWS profile name to use when loading credentials from the local AWS credential/config chain.
    aws_bedrock_profile: AwsBedrockProfile {
        type: String,
        default: "default".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_profile",
        description: "The AWS profile name to use for Bedrock credentials.",
    }
    // Whether the AWS Bedrock login banner has been permanently dismissed.
    //
    // Not a user-visible setting - we model it as a setting so we can track state.
    aws_bedrock_login_banner_dismissed: AwsBedrockLoginBannerDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }













    // Whether to render the CLI agent footer for commands like Claude, Codex, Gemini, etc.
    // This is independent of the "Use Agent" footer setting.
    should_render_cli_agent_footer: ShouldRenderCLIAgentToolbar {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.should_render_cli_agent_toolbar",
        description: "Whether to show the CLI agent footer for coding agent commands.",
    }
    // When enabled and a CLI agent session has a plugin listener, rich input
    // auto-closes when the session enters a Blocked state (the agent requires
    // direct keyboard interaction) and auto-opens when it leaves Blocked.
    auto_toggle_rich_input: AutoToggleRichInput {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_toggle_composer",
        description: "Whether CLI agent Rich Input automatically closes and reopens based on the agent's blocked state.",
    }

    // When enabled and a CLI agent session has a plugin listener, rich input
    // auto-opens once when the session starts or when the listener is registered.
    auto_open_rich_input_on_cli_agent_start: AutoOpenRichInputOnCLIAgentStart {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_open_composer_on_cli_agent_start",
        description: "Whether CLI agent Rich Input automatically opens when a CLI agent session starts.",
    }

    // When enabled and a CLI agent session does NOT have a plugin listener,
    // rich input auto-closes after the user submits a prompt.
    // When the plugin IS present, this setting has no effect (auto-show/hide
    // from auto_toggle_rich_input handles rich input lifecycle).
    auto_dismiss_rich_input_after_submit: AutoDismissRichInputAfterSubmit {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_dismiss_composer_after_submit",
        description: "Whether CLI agent Rich Input automatically closes after the user submits a prompt.",
    }

    // Maps custom toolbar command regex patterns to specific CLI agents.
    // Keys are regex patterns matched against the full command string.
    // Values are serialized CLIAgent names (empty string = any agent).
    // Supports migration from the legacy Vec<String> format.
    cli_agent_footer_enabled_commands: CLIAgentToolbarEnabledCommands {
        type: ToolbarCommandMap,
        default: ToolbarCommandMap::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.cli_agent_toolbar_enabled_commands",
        max_table_depth: 1,
        description: "Maps custom toolbar command patterns to specific CLI agents.",
    }





    // The raw stored default mode for new sessions. Use `default_session_mode()` to retrieve the
    // effective value, which is gated on AI availability.
    default_session_mode_internal: DefaultSessionMode,

    // The file path of the tab config used when default_session_mode_internal is TabConfig.
    // Only read when mode is TabConfig; ignored for all other modes.
    // Machine-local (tab config paths vary per machine), so never synced to cloud.
    default_tab_config_path: DefaultTabConfigPath {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "general.default_tab_config_path",
    }








    // Controls whether agent notifications (mailbox button, toasts, notification items) are shown.
    show_agent_notifications: ShowAgentNotifications {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.other.show_agent_notifications",
        description: "Whether agent notifications are shown.",
    }

    // Per-agent, per-host tracking of whether the user dismissed the plugin install chip.
    // Keys are "<agent_prefix>" for local sessions or "<agent_prefix>@<host>" for remote.
    // Local-only so dismissal doesn't sync across devices.
    plugin_install_chip_dismissed_map: PluginInstallChipDismissedMap {
        type: HashMap<String, bool>,
        default: HashMap::default(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }

    // Per-agent, per-host tracking of the MINIMUM_PLUGIN_VERSION for which the user
    // dismissed the plugin update chip. Empty/absent means not dismissed.
    // Keys are "<agent_prefix>" for local sessions or "<agent_prefix>@<host>" for remote.
    // Local-only so dismissal doesn't sync across devices.
    plugin_update_chip_dismissed_for_version_map: PluginUpdateChipDismissedForVersionMap {
        type: HashMap<String, String>,
        default: HashMap::default(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }

    // 用户自定义 Agent 提供商列表。第一阶段仅支持 OpenAI 兼容协议。
    //
    // 注意: 提供商的 `api_key` 不在这里持久化,见 `AgentProviderSecrets`。
    agent_providers: AgentProviders {
        type: Vec<AgentProvider>,
        default: Vec::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.warp_agent.providers",
        description: "User-configured custom Agent providers (OpenAI-compatible).",
    }








    // OpenWarp BYOP 模型 + 思考深度持久化(picker 切换后立即写入,新 tab/重启沿用)。
    // 模型用 LLMId 字符串形式;空串 = 没有 last_used,落回 profile 默认。
    byop_last_used_model_id: ByopLastUsedModelId {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop.last_used_model_id",
        description: "Last selected BYOP model id (picker hydrates new tabs/sessions from this).",
    }

    // OpenWarp BYOP per-(api_type, model) 思考深度记忆。
    // key = `<api_type>:<model_id>`,value = ReasoningEffortSetting。picker 切换写入。
    byop_last_used_reasoning: ByopLastUsedReasoning {
        type: BYOPLastUsedReasoningMap,
        default: BYOPLastUsedReasoningMap::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop.last_used_reasoning",
        max_table_depth: 1,
        description: "Per-(api_type, model) reasoning effort memory for BYOP picker.",
    }
]);

impl AISettings {
    pub fn register_and_subscribe_to_events(app: &mut AppContext) {
        Self::register(app);
        app.add_singleton_model(FocusedTerminalInfo::new);
        CompiledCommandsForCodingAgentToolbar::register(app);

        app.update_model(&Self::handle(app), |_me, ctx| {
            ctx.subscribe_to_model(&FocusedTerminalInfo::handle(ctx), |_me, event, ctx| {
                if matches!(event, FocusedTerminalInfoEvent::TerminalInfoUpdated) {
                    // Pipe the event so that any view that listens for settings changes will be notified.
                    ctx.emit(AISettingsChangedEvent::IsAnyAIEnabled {
                        change_event_reason: ChangeEventReason::LocalChange,
                    });
                }
            });
        });
    }

    pub fn is_any_ai_enabled(&self) -> bool {
        *self.is_any_ai_enabled
    }

    pub fn default_session_mode(&self) -> DefaultSessionMode {
        *self.default_session_mode_internal.value()
    }

    /// Returns the stored default tab config path (only meaningful when mode is `TabConfig`).
    pub fn default_tab_config_path(&self) -> &str {
        &self.default_tab_config_path
    }

    /// Looks up the `TabConfig` matching the stored `default_tab_config_path`.
    /// Returns `None` if the path is empty or no loaded config matches.
    pub fn resolved_default_tab_config(
        &self,
        app: &AppContext,
    ) -> Option<crate::tab_configs::TabConfig> {
        let path_str = self.default_tab_config_path.as_str();
        if path_str.is_empty() {
            return None;
        }
        let path = std::path::Path::new(path_str);
        crate::user_config::WarpConfig::as_ref(app)
            .tab_configs()
            .iter()
            .find(|config| config.source_path.as_deref().is_some_and(|p| p == path))
            .cloned()
    }

    pub fn is_active_ai_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled()
            && *self.is_active_ai_enabled_internal
            && AppExecutionMode::as_ref(app).allows_active_ai()
    }

    pub fn is_intelligent_autosuggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.intelligent_autosuggestions_enabled_internal
    }

    pub fn add_cli_agent_footer_enabled_command(
        &mut self,
        command: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        if self
            .cli_agent_footer_enabled_commands
            .value()
            .contains_key(command)
        {
            return;
        }

        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        map.insert(command.to_string(), String::new());
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    pub fn remove_cli_agent_footer_enabled_command(
        &mut self,
        command: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        let command = command.trim();
        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        map.shift_remove(command);
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    pub fn set_cli_agent_for_command(
        &mut self,
        pattern: &str,
        agent: Option<CLIAgent>,
        ctx: &mut ModelContext<Self>,
    ) {
        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        if !map.contains_key(pattern) {
            return;
        }
        let value = agent.map(|a| a.to_serialized_name()).unwrap_or_default();
        map.insert(pattern.to_string(), value);
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    /// Whether the plugin install chip was dismissed for the given agent/host.
    pub fn is_plugin_install_chip_dismissed(&self, key: &str) -> bool {
        self.plugin_install_chip_dismissed_map
            .get(key)
            .copied()
            .unwrap_or(false)
    }

    /// Mark the plugin install chip as dismissed for the given agent/host.
    pub fn dismiss_plugin_install_chip(&mut self, key: &str, ctx: &mut ModelContext<Self>) {
        let mut map = self.plugin_install_chip_dismissed_map.clone();
        map.insert(key.to_owned(), true);
        report_if_error!(self.plugin_install_chip_dismissed_map.set_value(map, ctx));
    }

    /// Returns the minimum plugin version for which the update chip was dismissed
    /// for the given agent/host, or an empty string if not dismissed.
    pub fn plugin_update_chip_dismissed_version(&self, key: &str) -> &str {
        self.plugin_update_chip_dismissed_for_version_map
            .get(key)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Record that the user dismissed the update chip for the given agent/host at
    /// the specified minimum version.
    pub fn dismiss_plugin_update_chip(
        &mut self,
        key: &str,
        version: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let mut map = self.plugin_update_chip_dismissed_for_version_map.clone();
        map.insert(key.to_owned(), version);
        report_if_error!(self
            .plugin_update_chip_dismissed_for_version_map
            .set_value(map, ctx));
    }
}

/// Singleton model that caches compiled regexes for the `cli_agent_footer_enabled_commands`
/// setting. Each entry pairs a compiled regex with the CLI agent it maps to.
pub struct CompiledCommandsForCodingAgentToolbar {
    regexes: Vec<(Regex, CLIAgent)>,
}

impl CompiledCommandsForCodingAgentToolbar {
    fn parse(app: &AppContext) -> Vec<(Regex, CLIAgent)> {
        AISettings::as_ref(app)
            .cli_agent_footer_enabled_commands
            .value()
            .iter()
            .filter_map(|(pattern, agent_name)| {
                let regex = Regex::new(pattern).ok()?;
                let agent = CLIAgent::from_serialized_name(agent_name);
                Some((regex, agent))
            })
            .collect()
    }

    fn register(app: &mut AppContext) {
        let handle = app.add_singleton_model(|ctx| Self {
            regexes: Self::parse(ctx),
        });
        let ai_settings = AISettings::handle(app);
        app.subscribe_to_model(&ai_settings, move |_, event, ctx| {
            if matches!(
                event,
                AISettingsChangedEvent::CLIAgentToolbarEnabledCommands { .. }
            ) {
                let regexes = Self::parse(ctx);
                handle.update(ctx, |me, _| {
                    me.regexes = regexes;
                });
            }
        });
    }

    /// Returns the CLI agent assigned to the first matching pattern, or `None`
    /// if no pattern matches the command.
    pub fn matched_agent(app: &AppContext, command: &str) -> Option<CLIAgent> {
        Self::as_ref(app)
            .regexes
            .iter()
            .find(|(regex, _)| regex.is_match(command))
            .map(|(_, agent)| *agent)
    }
}

impl Entity for CompiledCommandsForCodingAgentToolbar {
    type Event = ();
}

impl SingletonEntity for CompiledCommandsForCodingAgentToolbar {}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
