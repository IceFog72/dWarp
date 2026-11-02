use serde::{de, Deserialize, Serialize};
use settings::Setting as _;
use std::collections::HashMap;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

pub use ai::LLMId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisableReason {
    AdminDisabled,
    OutOfRequests,
    ProviderOutage,
    RequiresUpgrade,
    Unavailable,
}

impl DisableReason {}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LLMSpec {
    pub cost: f32,
    pub quality: f32,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LLMProvider {
    OpenAI,
    Anthropic,
    Google,
    Xai,
    Unknown,
}

impl LLMProvider {}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLMContextWindow {
    #[serde(default)]
    pub is_configurable: bool,
    #[serde(default)]
    pub min: u32,
    #[serde(default)]
    pub max: u32,
    #[serde(default)]
    pub default_max: u32,
}

/// Metadata about an LLM.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LLMInfo {
    pub display_name: String,
    pub base_model_name: String,
    pub id: LLMId,
    pub reasoning_level: Option<String>,
    pub description: Option<String>,
    pub disable_reason: Option<DisableReason>,
    pub vision_supported: bool,
    pub spec: Option<LLMSpec>,
    pub provider: LLMProvider,
    pub context_window: LLMContextWindow,
}

impl<'de> Deserialize<'de> for LLMInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireLLMInfo {
            display_name: String,
            #[serde(default)]
            base_model_name: Option<String>,
            id: LLMId,
            #[serde(default)]
            reasoning_level: Option<String>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            disable_reason: Option<DisableReason>,
            #[serde(default)]
            vision_supported: bool,
            #[serde(default)]
            spec: Option<LLMSpec>,
            provider: LLMProvider,
            #[serde(default)]
            context_window: LLMContextWindow,
        }

        let wire = WireLLMInfo::deserialize(deserializer)?;
        Ok(Self {
            base_model_name: wire
                .base_model_name
                .unwrap_or_else(|| wire.display_name.clone()),
            vision_supported: wire.vision_supported,
            provider: wire.provider,
            display_name: wire.display_name,
            id: wire.id,
            reasoning_level: wire.reasoning_level,
            description: wire.description,
            disable_reason: wire.disable_reason,
            spec: wire.spec,
            context_window: wire.context_window,
        })
    }
}

impl LLMInfo {
    /// Returns the display name for the LLM, to be used in the LLM selector menu.
    pub fn menu_display_name(&self) -> String {
        // Base label includes optional description in parentheses
        match &self.description {
            // This is a temporary implementation that won't scale well for longer
            // descriptions. We should implement a better approach for displaying
            // model descriptions, maybe through subtext.
            Some(desc) => format!("{} ({})", self.display_name, desc),
            None => self.display_name.clone(),
        }
    }
}

/// The set of LLMs available for a feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvailableLLMs {
    /// The Warp "default" LLM.
    default_id: LLMId,
    choices: Vec<LLMInfo>,

    #[serde(default)]
    preferred_codex_model_id: Option<LLMId>,
}

impl AvailableLLMs {
    /// Constructs an `AvailableLLMs` instance from the given default ID and choices.
    ///
    /// If choices is empty, returns an error.
    ///
    /// If default_id is not a valid ID present in `choices`, takes the first choice in `choices
    /// and uses it as the default.
    pub fn new<T: Into<LLMInfo>>(
        mut default_id: LLMId,
        choices: impl IntoIterator<Item = T>,
        preferred_codex_model_id: Option<LLMId>,
    ) -> Result<Self, anyhow::Error> {
        let choices: Vec<LLMInfo> = choices.into_iter().map(Into::into).collect();
        if choices.is_empty() {
            return Err(anyhow::anyhow!(
                "Tried to create AvailableLLMs with empty`choices`.",
            ));
        } else if !choices.iter().any(|info| info.id == default_id) {
            let fallback_default = choices
                .first()
                .ok_or_else(|| anyhow::anyhow!("Choices should not be empty"))?;
            log::error!(
                "Default LLM ID {} not present in choices, falling back to first choice {}",
                default_id,
                fallback_default.display_name
            );
            default_id = fallback_default.id.clone();
        }

        Ok(Self {
            default_id,
            choices: choices.into_iter().collect(),
            preferred_codex_model_id,
        })
    }

    fn info_for_id(&self, id: &LLMId) -> Option<&LLMInfo> {
        self.choices.iter().find(|info| info.id == *id)
    }

    fn default_llm_info(&self) -> &LLMInfo {
        self.info_for_id(&self.default_id)
            .expect("Default LLM ID must be present in choices")
    }
}

/// The set of models available to the client, grouped by the feature they support.
/// This is fetched from the server and cached.
///
/// Currently, if a model is available for multiple features,
/// it will appear denormalized in each of the feature's
/// [`AvailableLLMs`]. While this denormalization doesn't add much value today,
/// it eventually lets us add feature-specific properties to an [`LLMInfo`].
///
/// NOTE: This used to include a `planning` field; this was removed after planning via subagent was
/// deprecated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelsByFeature {
    pub agent_mode: AvailableLLMs,
    pub coding: AvailableLLMs,
    /// The set of LLMs available for CLI agent.
    /// This field is optional during deserialization, as older clients might not have this field.
    #[serde(default)]
    pub cli_agent: Option<AvailableLLMs>,
    /// The set of LLMs available for computer use agent.
    /// This field is optional during deserialization, as older clients might not have this field.
    #[serde(default)]
    pub computer_use: Option<AvailableLLMs>,
}

/// Returns the default AvailableLLMs for computer use.
/// Used both in `ModelsByFeature::default()` and as a fallback in `get_computer_use_available()`.
fn default_computer_use_llms() -> AvailableLLMs {
    AvailableLLMs {
        default_id: "computer-use-agent-auto".to_owned().into(),
        choices: vec![LLMInfo {
            display_name: "auto".to_owned(),
            base_model_name: "auto".to_owned(),
            id: "computer-use-agent-auto".to_owned().into(),
            reasoning_level: None,
            description: None,
            disable_reason: None,
            vision_supported: true,
            spec: None,
            provider: LLMProvider::Unknown,
            context_window: LLMContextWindow::default(),
        }],
        preferred_codex_model_id: None,
    }
}

impl Default for ModelsByFeature {
    fn default() -> Self {
        Self {
            agent_mode: AvailableLLMs {
                default_id: "auto".to_owned().into(),
                choices: vec![LLMInfo {
                    display_name: "auto (cost-efficient)".to_owned(),
                    base_model_name: "auto (cost-efficient)".to_owned(),
                    id: "auto".to_owned().into(),
                    reasoning_level: None,
                    description: None,
                    disable_reason: None,
                    vision_supported: true,
                    spec: None,
                    provider: LLMProvider::Unknown,
                    context_window: LLMContextWindow::default(),
                }],
                preferred_codex_model_id: None,
            },
            coding: AvailableLLMs {
                default_id: "auto".to_owned().into(),
                choices: vec![LLMInfo {
                    display_name: "auto (responsive)".to_owned(),
                    base_model_name: "auto (responsive)".to_owned(),
                    id: "auto".to_owned().into(),
                    reasoning_level: None,
                    description: None,
                    disable_reason: None,
                    vision_supported: true,
                    spec: None,
                    provider: LLMProvider::Unknown,
                    context_window: LLMContextWindow::default(),
                }],
                preferred_codex_model_id: None,
            },
            cli_agent: Some(AvailableLLMs {
                default_id: "cli-agent-auto".to_owned().into(),
                choices: vec![LLMInfo {
                    display_name: "auto".to_owned(),
                    base_model_name: "auto".to_owned(),
                    id: "cli-agent-auto".to_owned().into(),
                    reasoning_level: None,
                    description: None,
                    disable_reason: None,
                    vision_supported: false,
                    spec: None,
                    provider: LLMProvider::Unknown,
                    context_window: LLMContextWindow::default(),
                }],
                preferred_codex_model_id: None,
            }),
            computer_use: Some(default_computer_use_llms()),
        }
    }
}

/// Singleton model holding the BYOP LLMs available for AI command suggestions and the
/// user's per-model reasoning effort memory.
pub struct LLMPreferences {
    models_by_feature: ModelsByFeature,
    /// Last reasoning effort used per (api_type, model), hydrated from settings.
    last_used_reasoning: HashMap<
        (crate::settings::AgentProviderApiType, String),
        crate::settings::ReasoningEffortSetting,
    >,
}

impl LLMPreferences {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        // BYOP-only: the model list is rebuilt from the configured agent_providers.
        let models_by_feature = crate::ai::agent_providers::build_byop_models_by_feature(&*ctx);

        ctx.subscribe_to_model(
            &crate::settings::AISettings::handle(ctx),
            |me, _event, ctx| {
                me.refresh_byop_models(ctx);
            },
        );
        // API key changes affect which providers are usable.
        ctx.subscribe_to_model(
            &crate::ai::agent_providers::AgentProviderSecrets::handle(ctx),
            |me, _event, ctx| {
                me.refresh_byop_models(ctx);
            },
        );

        let last_used_reasoning = {
            use crate::settings::AISettings;
            let s = AISettings::as_ref(&*ctx);
            let mut map = HashMap::new();
            for (key, effort) in s.byop_last_used_reasoning.iter() {
                if let Some((api_type_str, model_id)) = key.split_once(':') {
                    if let Some(api_type) =
                        crate::settings::AgentProviderApiType::from_debug_str(api_type_str)
                    {
                        map.insert((api_type, model_id.to_owned()), *effort);
                    }
                }
            }
            map
        };

        Self {
            models_by_feature,
            last_used_reasoning,
        }
    }

    /// Returns the BYOP models the user can pick for AI command suggestions.
    pub fn get_command_suggestion_llm_choices(&self) -> impl Iterator<Item = &LLMInfo> {
        self.models_by_feature
            .agent_mode
            .choices
            .iter()
            .filter(|llm| !matches!(llm.disable_reason, Some(DisableReason::AdminDisabled)))
    }

    /// Returns the model used for AI command suggestions and workflow autofill.
    ///
    /// Uses the model stored in `AISettings.byop_last_used_model_id`, falling back to the
    /// first configured BYOP model.
    pub fn get_command_suggestion_model<'a>(&'a self, app: &'a AppContext) -> &'a LLMInfo {
        let selected = crate::settings::AISettings::as_ref(app)
            .byop_last_used_model_id
            .to_string();
        if !selected.is_empty() {
            let llm_id: LLMId = selected.into();
            if let Some(llm_info) = self.models_by_feature.agent_mode.info_for_id(&llm_id) {
                return llm_info;
            }
        }
        self.models_by_feature.agent_mode.default_llm_info()
    }

    /// Persists the model used for AI command suggestions.
    pub fn set_command_suggestion_model(&self, llm_id: &LLMId, ctx: &mut ModelContext<Self>) {
        use warp_core::errors::report_if_error;
        let llm_id_str = llm_id.as_str().to_owned();
        crate::settings::AISettings::handle(ctx).update(ctx, |settings, ctx| {
            if settings.byop_last_used_model_id.to_string() != llm_id_str {
                report_if_error!(settings.byop_last_used_model_id.set_value(llm_id_str, ctx));
            }
        });
    }

    /// Rebuilds the model list from settings.agent_providers + AgentProviderSecrets.
    pub fn refresh_byop_models(&mut self, ctx: &mut ModelContext<Self>) {
        let new = crate::ai::agent_providers::build_byop_models_by_feature(&*ctx);
        if new != self.models_by_feature {
            self.models_by_feature = new;
            ctx.emit(LLMPreferencesEvent::UpdatedAvailableLLMs);
        }
    }

    /// Returns the reasoning effort for a model.
    /// Priority: last used for (api_type, model) > the model's default level > Auto.
    pub fn get_reasoning_effort(
        &self,
        api_type: crate::settings::AgentProviderApiType,
        model_id: &str,
    ) -> crate::settings::ReasoningEffortSetting {
        if let Some(eff) = self
            .last_used_reasoning
            .get(&(api_type, model_id.to_owned()))
        {
            return *eff;
        }
        crate::ai::agent_providers::reasoning::default_reasoning_for(api_type, model_id)
            .unwrap_or(crate::settings::ReasoningEffortSetting::Auto)
    }
}

#[derive(Clone, Debug)]
pub enum LLMPreferencesEvent {
    UpdatedAvailableLLMs,
}

impl Entity for LLMPreferences {
    type Event = LLMPreferencesEvent;
}

impl SingletonEntity for LLMPreferences {}

#[cfg(test)]
#[path = "llms_tests.rs"]
mod tests;
