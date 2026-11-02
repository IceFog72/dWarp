use super::agent_providers_widget::AgentProvidersWidget;
use super::settings_page::{
    build_sub_header, build_toggle_element, render_body_item_label, MatchData, PageType,
    SettingsPageMeta, SettingsPageViewHandle, SettingsWidget, ToggleState, HEADER_PADDING,
};
use super::SettingsSection;
use crate::ai::llms::{LLMId, LLMPreferences};
use crate::appearance::Appearance;
use crate::menu::{MenuItem, MenuItemFields};
use crate::report_if_error;
use crate::settings::AISettings;
use crate::terminal::cli_agent_input::footer::editor::AgentToolbarInlineEditor;
use crate::terminal::CLIAgent;
use crate::view_components::dropdown::DropdownAction;
use crate::view_components::{
    Dropdown, DropdownItem, SubmittableTextInput, SubmittableTextInputEvent,
};
use enum_iterator::all;
use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use regex::Regex;
use settings::{Setting, ToggleableSetting};
use warp_core::features::FeatureFlag;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    ChildView, Container, CornerRadius, CrossAxisAlignment, Fill, Flex, FormattedTextElement,
    HighlightedHyperlink, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Radius,
    Shrinkable,
};
use warpui::keymap::ContextPredicate;
use warpui::ui_components::{
    components::{Coords, UiComponent, UiComponentStyles},
    switch::SwitchStateHandle,
};
use warpui::{
    Action, AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

const CONTENT_FONT_SIZE: f32 = 12.;

/// Identifies which subpage of the AI settings the user is viewing.
/// When `None`, the page shows all widgets (legacy/full view).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AISubpage {
    /// 自定义 AI 提供商(BYOP) 配置子页。
    Providers,
    /// Third-party CLI agent settings.
    ThirdPartyCLIAgents,
}

impl AISubpage {
    pub fn from_section(section: SettingsSection) -> Option<Self> {
        match section {
            SettingsSection::AgentProviders => Some(Self::Providers),
            SettingsSection::ThirdPartyCLIAgents => Some(Self::ThirdPartyCLIAgents),
            _ => None,
        }
    }
}
pub struct AISettingsPageView {
    page: PageType<Self>,
    active_subpage: Option<AISubpage>,

    cli_agent_footer_command_editor: ViewHandle<SubmittableTextInput>,
    cli_agent_footer_command_mouse_state_handles: Vec<MouseStateHandle>,
    cli_agent_footer_command_agent_dropdowns: Vec<ViewHandle<Dropdown<AISettingsPageAction>>>,
    cli_agent_toolbar_inline_editor: ViewHandle<AgentToolbarInlineEditor>,
    command_suggestion_model_dropdown: ViewHandle<Dropdown<AISettingsPageAction>>,
}
impl AISettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let cli_agent_footer_command_editor = ctx.add_typed_action_view(|ctx| {
            let mut input =
                SubmittableTextInput::new(ctx).validate_on_edit(|s| Regex::new(s).is_ok());
            input.set_placeholder_text(
                crate::t!("settings-ai-command-supports-regex-placeholder"),
                ctx,
            );
            input
        });
        ctx.subscribe_to_view(
            &cli_agent_footer_command_editor,
            |_, _, event, ctx| match event {
                SubmittableTextInputEvent::Submit(command) => {
                    AISettings::handle(ctx).update(ctx, |settings, ctx| {
                        settings.add_cli_agent_footer_enabled_command(command, ctx);
                    });
                }
                SubmittableTextInputEvent::Escape => ctx.emit(AISettingsPageEvent::FocusModal),
            },
        );

        let command_suggestion_model_dropdown = ctx.add_typed_action_view(Dropdown::new);
        Self::refresh_command_suggestion_models(&command_suggestion_model_dropdown, ctx);
        ctx.subscribe_to_model(&LLMPreferences::handle(ctx), |me, _, _, ctx| {
            Self::refresh_command_suggestion_models(&me.command_suggestion_model_dropdown, ctx);
        });
        ctx.subscribe_to_model(&AISettings::handle(ctx), |me, _, _, ctx| {
            me.cli_agent_footer_command_mouse_state_handles = AISettings::as_ref(ctx)
                .cli_agent_footer_enabled_commands
                .value()
                .keys()
                .map(|_| Default::default())
                .collect();
            me.cli_agent_footer_command_agent_dropdowns = Self::create_cli_agent_dropdowns(ctx);
            ctx.notify();
        });
        Self {
            page: Self::build_page(None, ctx),
            active_subpage: None,

            cli_agent_footer_command_editor,
            cli_agent_footer_command_mouse_state_handles: AISettings::as_ref(ctx)
                .cli_agent_footer_enabled_commands
                .value()
                .keys()
                .map(|_| Default::default())
                .collect(),
            cli_agent_footer_command_agent_dropdowns: Self::create_cli_agent_dropdowns(ctx),
            cli_agent_toolbar_inline_editor: ctx
                .add_typed_action_view(AgentToolbarInlineEditor::new),
            command_suggestion_model_dropdown,
        }
    }
    fn refresh_command_suggestion_models(
        menu: &ViewHandle<Dropdown<AISettingsPageAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        menu.update(ctx, |menu, ctx| {
            let preferences = LLMPreferences::as_ref(ctx);
            let items = preferences
                .get_command_suggestion_llm_choices()
                .map(|llm| {
                    DropdownItem::new(
                        llm.menu_display_name(),
                        AISettingsPageAction::SetCommandSuggestionModel(llm.id.clone()),
                    )
                })
                .collect();
            let selected = preferences.get_command_suggestion_model(ctx).id.clone();
            menu.set_items(items, ctx);
            menu.set_selected_by_action(
                AISettingsPageAction::SetCommandSuggestionModel(selected),
                ctx,
            );
        });
        ctx.notify();
    }
    /// Set the active subpage and rebuild the widget list to show only relevant widgets.
    pub fn set_active_subpage(&mut self, subpage: Option<AISubpage>, ctx: &mut ViewContext<Self>) {
        if self.active_subpage != subpage {
            self.active_subpage = subpage;
            self.page = Self::build_page(subpage, ctx);
            ctx.notify();
        }
    }

    /// 重建当前 subpage 的 widget 列表。
    /// 用于 widget 的内部状态依赖 `AISettings` 中的复杂集合(例如自定义 Agent
    /// Provider 列表),在集合大小变化时需要重新创建 widget 持有的 ViewHandle。
    pub fn rebuild_current_page(&mut self, ctx: &mut ViewContext<Self>) {
        // 复用旧 page 的滚动 handle,避免重建后跳回顶部。
        let preserved_scroll = self.page.scroll_states();
        self.page = Self::build_page(self.active_subpage, ctx);
        if let Some((v, h)) = preserved_scroll {
            self.page.replace_scroll_states(v, h);
        }
        ctx.notify();
    }

    fn build_page(subpage: Option<AISubpage>, ctx: &mut ViewContext<Self>) -> PageType<Self> {
        let widgets: Vec<Box<dyn SettingsWidget<View = Self>>> =
            if subpage == Some(AISubpage::ThirdPartyCLIAgents) {
                vec![Box::<CLIAgentWidget>::default()]
            } else {
                vec![
                    Box::<CommandSuggestionsWidget>::default(),
                    Box::new(AgentProvidersWidget::new(ctx)),
                ]
            };
        PageType::new_uncategorized(widgets, None)
    }
    fn create_cli_agent_dropdowns(
        ctx: &mut ViewContext<Self>,
    ) -> Vec<ViewHandle<Dropdown<AISettingsPageAction>>> {
        let entries: Vec<(String, CLIAgent)> = AISettings::as_ref(ctx)
            .cli_agent_footer_enabled_commands
            .value()
            .iter()
            .map(|(pattern, agent_value)| {
                (pattern.clone(), CLIAgent::from_serialized_name(agent_value))
            })
            .collect();

        entries
            .into_iter()
            .map(|(pattern_clone, current_agent)| {
                ctx.add_typed_action_view(move |ctx| {
                    let mut dropdown = Dropdown::new(ctx);
                    dropdown.set_top_bar_max_width(160.);
                    dropdown.set_menu_width(180., ctx);
                    dropdown.set_main_axis_size(MainAxisSize::Min, ctx);

                    let mut items: Vec<MenuItem<DropdownAction<AISettingsPageAction>>> = Vec::new();

                    for agent in all::<CLIAgent>() {
                        if matches!(agent, CLIAgent::Unknown) {
                            continue;
                        }
                        let icon = agent.icon();
                        let mut fields = MenuItemFields::new(agent.display_name())
                            .with_on_select_action(DropdownAction::SelectActionAndClose(
                                AISettingsPageAction::SetCLIAgentForCommand {
                                    pattern: pattern_clone.clone(),
                                    agent: Some(agent),
                                },
                            ));
                        if let Some(icon) = icon {
                            fields = fields.with_icon(icon);
                        }
                        items.push(fields.into_item());
                    }

                    let other_label = crate::t!("settings-ai-coding-agent-other");
                    items.push(
                        MenuItemFields::new(other_label.clone())
                            .with_on_select_action(DropdownAction::SelectActionAndClose(
                                AISettingsPageAction::SetCLIAgentForCommand {
                                    pattern: pattern_clone.clone(),
                                    agent: None,
                                },
                            ))
                            .into_item(),
                    );

                    dropdown.set_rich_items(items, ctx);

                    let other_label_for_override = other_label.clone();
                    dropdown.set_menu_header_text_override(move |label| {
                        if label == other_label_for_override {
                            crate::t!("settings-ai-coding-agent-select-header")
                        } else {
                            label.to_string()
                        }
                    });

                    let selected_name: String = if matches!(current_agent, CLIAgent::Unknown) {
                        other_label
                    } else {
                        current_agent.display_name().to_string()
                    };
                    dropdown.set_selected_by_name(selected_name, ctx);

                    dropdown
                })
            })
            .collect()
    }
}

impl View for AISettingsPageView {
    fn ui_name() -> &'static str {
        "AISettingsPage"
    }

    fn render(&self, app: &warpui::AppContext) -> Box<dyn warpui::Element> {
        self.page.render(self, app)
    }
}

pub enum AISettingsPageEvent {
    FocusModal,
}
impl Entity for AISettingsPageView {
    type Event = AISettingsPageEvent;
}
#[derive(Debug, Clone, PartialEq)]
pub enum AISettingsPageAction {
    ToggleGlobalAI,
    ToggleIntelligentAutosuggestions,
    ToggleCLIAgentToolbar,
    ToggleAutoToggleRichInput,
    ToggleAutoOpenRichInputOnCLIAgentStart,
    ToggleAutoDismissRichInputAfterSubmit,
    RemoveCLIAgentToolbarEnabledCommand(String),
    SetCLIAgentForCommand {
        pattern: String,
        agent: Option<CLIAgent>,
    },
    SetCommandSuggestionModel(LLMId),
    AddAgentProvider,
    RemoveAgentProvider {
        provider_id: String,
    },
    UpdateAgentProviderName {
        provider_id: String,
        name: String,
    },
    UpdateAgentProviderBaseUrl {
        provider_id: String,
        base_url: String,
    },
    /// 显式设置 provider 的 API 协议类型(OpenAI / OpenAI-Response / Gemini / Anthropic / Ollama)。
    /// chat_stream 据此显式绑定 genai AdapterKind,绕过模型名识别。
    SetAgentProviderApiType {
        provider_id: String,
        api_type: crate::settings::AgentProviderApiType,
    },
    UpdateAgentProviderApiKey {
        provider_id: String,
        api_key: String,
    },
    UpdateAgentProviderModels {
        provider_id: String,
        models: Vec<crate::settings::AgentProviderModel>,
    },
    AddAgentProviderModel {
        provider_id: String,
    },
    RemoveAgentProviderModel {
        provider_id: String,
        model_index: usize,
    },
    UpdateAgentProviderModelName {
        provider_id: String,
        model_index: usize,
        name: String,
    },
    UpdateAgentProviderModelId {
        provider_id: String,
        model_index: usize,
        id: String,
    },
    /// 更新单条模型的 context_window(tokens),0 = 未指定。
    UpdateAgentProviderModelContextWindow {
        provider_id: String,
        model_index: usize,
        context_window: u32,
    },
    /// 更新单条模型的 max_output_tokens,0 = 未指定。
    UpdateAgentProviderModelMaxOutput {
        provider_id: String,
        model_index: usize,
        max_output_tokens: u32,
    },
    FetchAgentProviderModels {
        provider_id: String,
    },
    /// 触发一次 models.dev 目录加载(磁盘缓存 + 必要时网络刷新)。Providers 子页打开即触发。
    EnsureModelsDevLoaded,
    /// 强制刷新 models.dev 目录(忽略 TTL)。"刷新" 按钮触发。
    RefreshModelsDev,
    /// 从 models.dev 目录创建一个新 provider:回填 name/base_url/全部模型(含 context)。
    AddProviderFromModelsDev {
        catalog_provider_id: String,
    },
    /// 把现有 provider 的模型列表与 models.dev 同步(按 base_url 匹配),
    /// 用 catalog 提供的 context_window / reasoning / tool_call 等元数据填充本地条目。
    SyncProviderModelsFromModelsDev {
        provider_id: String,
    },
    /// 折叠/展开 "快速添加" chip 行。
    ToggleModelsDevChipsExpanded,
    /// 设置 "快速添加" chip 行的搜索 query(子串过滤 provider name/id)。
    SetModelsDevSearchQuery(String),

    // ----- 单条模型条目 detail panel -----
    /// 切换单条模型的 detail panel 展开/折叠状态。
    ToggleAgentProviderModelExpanded {
        provider_id: String,
        model_index: usize,
    },
    /// 三态循环切换单条模型的某个多模态 capability(image/pdf/audio)。
    /// `None → Some(true) → Some(false) → None`。
    CycleAgentProviderModelCapability {
        provider_id: String,
        model_index: usize,
        kind: ModelCapabilityKind,
    },
    /// 切换单条模型的 reasoning 标志(普通 bool 字段,不是三态)。
    ToggleAgentProviderModelReasoning {
        provider_id: String,
        model_index: usize,
    },
    /// 切换单条模型的 tool_call 标志。
    ToggleAgentProviderModelToolCall {
        provider_id: String,
        model_index: usize,
    },
}
/// model detail panel 三态 capability chip 的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCapabilityKind {
    Image,
    Pdf,
    Audio,
}
impl TypedActionView for AISettingsPageView {
    type Action = AISettingsPageAction;
    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            AISettingsPageAction::SetCommandSuggestionModel(id) => {
                LLMPreferences::handle(ctx).update(ctx, |preferences, ctx| {
                    preferences.set_command_suggestion_model(id, ctx)
                });
                Self::refresh_command_suggestion_models(
                    &self.command_suggestion_model_dropdown,
                    ctx,
                );
            }
            AISettingsPageAction::ToggleGlobalAI => {
                if let Err(e) = AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.is_any_ai_enabled.toggle_and_save_value(ctx)
                }) {
                    log::warn!("Failed to set value for Global AI setting: {e:?}");
                }
                ctx.notify();
            }
            AISettingsPageAction::ToggleIntelligentAutosuggestions => {
                if let Err(e) = AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings
                        .intelligent_autosuggestions_enabled_internal
                        .toggle_and_save_value(ctx)
                }) {
                    log::warn!("Failed to set value for Next Command setting: {e:?}");
                }
                ctx.notify();
            }
            AISettingsPageAction::ToggleCLIAgentToolbar => {
                if let Err(e) = AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings
                        .should_render_cli_agent_footer
                        .toggle_and_save_value(ctx)
                }) {
                    log::warn!("Failed to set value for CLI Agent Footer setting: {e:?}");
                }
                ctx.notify();
            }
            AISettingsPageAction::ToggleAutoToggleRichInput => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.auto_toggle_rich_input.toggle_and_save_value(ctx));
                });
                ctx.notify();
            }
            AISettingsPageAction::ToggleAutoOpenRichInputOnCLIAgentStart => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings
                        .auto_open_rich_input_on_cli_agent_start
                        .toggle_and_save_value(ctx));
                });
                ctx.notify();
            }
            AISettingsPageAction::ToggleAutoDismissRichInputAfterSubmit => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings
                        .auto_dismiss_rich_input_after_submit
                        .toggle_and_save_value(ctx));
                });
                ctx.notify();
            }
            AISettingsPageAction::RemoveCLIAgentToolbarEnabledCommand(command) => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.remove_cli_agent_footer_enabled_command(command, ctx);
                });
            }
            AISettingsPageAction::SetCLIAgentForCommand { pattern, agent } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.set_cli_agent_for_command(pattern, *agent, ctx);
                });
            }
            AISettingsPageAction::AddAgentProvider => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    providers.push(crate::settings::AgentProvider::new_empty());
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::RemoveAgentProvider { provider_id } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    providers.retain(|p| p.id != *provider_id);
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                crate::ai::agent_providers::AgentProviderSecrets::handle(ctx).update(
                    ctx,
                    |secrets, ctx| {
                        secrets.remove(provider_id, ctx);
                    },
                );
                super::agent_providers_widget::clear_expanded_models_for_provider(provider_id);
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::UpdateAgentProviderName { provider_id, name } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        p.name = name.clone();
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::UpdateAgentProviderBaseUrl {
                provider_id,
                base_url,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        p.base_url = base_url.clone();
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::SetAgentProviderApiType {
                provider_id,
                api_type,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        p.api_type = *api_type;
                        // 若 base_url 为空,顺手填该类型的默认 endpoint(便于新手)。
                        // 用户已自填 base_url 时不动。
                        if p.base_url.trim().is_empty() {
                            p.base_url = api_type.default_base_url().to_owned();
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::UpdateAgentProviderApiKey {
                provider_id,
                api_key,
            } => {
                crate::ai::agent_providers::AgentProviderSecrets::handle(ctx).update(
                    ctx,
                    |secrets, ctx| {
                        secrets.set(provider_id, api_key.clone(), ctx);
                    },
                );
                ctx.notify();
            }
            AISettingsPageAction::UpdateAgentProviderModels {
                provider_id,
                models,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        p.models = models.clone();
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::AddAgentProviderModel { provider_id } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        p.models
                            .push(crate::settings::AgentProviderModel::from_id(String::new()));
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                // 行级 add 需要新建 EditorView,所以走 rebuild;rebuild_current_page 已保留滚动。
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::RemoveAgentProviderModel {
                provider_id,
                model_index,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if *model_index < p.models.len() {
                            p.models.remove(*model_index);
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                // 删一条会让后续 index 漂移,清掉这个 provider 的全部展开记录避免误展开。
                super::agent_providers_widget::clear_expanded_models_for_provider(provider_id);
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::UpdateAgentProviderModelName {
                provider_id,
                model_index,
                name,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.name = name.clone();
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::UpdateAgentProviderModelId {
                provider_id,
                model_index,
                id,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.id = id.clone();
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::UpdateAgentProviderModelContextWindow {
                provider_id,
                model_index,
                context_window,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.context_window = *context_window;
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::UpdateAgentProviderModelMaxOutput {
                provider_id,
                model_index,
                max_output_tokens,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.max_output_tokens = *max_output_tokens;
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                ctx.notify();
            }
            AISettingsPageAction::FetchAgentProviderModels { provider_id } => {
                let provider_id = provider_id.clone();
                let providers = AISettings::as_ref(ctx).agent_providers.value().clone();
                let Some(provider) = providers.into_iter().find(|p| p.id == provider_id) else {
                    return;
                };
                let api_key = crate::ai::agent_providers::AgentProviderSecrets::as_ref(ctx)
                    .get(&provider_id)
                    .map(str::to_owned);
                let client = http_client::Client::new();
                let provider_id_for_handler = provider_id.clone();
                ctx.spawn(
                    async move {
                        crate::ai::agent_providers::fetch_openai_compatible_models(
                            client,
                            &provider.base_url,
                            api_key.as_deref(),
                        )
                        .await
                    },
                    move |view, result, ctx| match result {
                        Ok(fetched) => {
                            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                                let mut providers = settings.agent_providers.value().clone();
                                if let Some(p) = providers
                                    .iter_mut()
                                    .find(|p| p.id == provider_id_for_handler)
                                {
                                    // 合并保留: 已存在的 id 保留用户改过的 name,新 id 追加,
                                    // 本地多余的 id 不删(用户手动 ×)。
                                    let existing: std::collections::HashSet<String> =
                                        p.models.iter().map(|m| m.id.clone()).collect();
                                    for m in fetched {
                                        if !existing.contains(&m.id) {
                                            p.models.push(
                                                crate::settings::AgentProviderModel::from_id(m.id),
                                            );
                                        }
                                    }
                                }
                                let _ = settings.agent_providers.set_value(providers, ctx);
                            });
                            // 模型行数可能变了,需要 rebuild widget rows。
                            view.rebuild_current_page(ctx);
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to fetch models for provider {provider_id_for_handler}: {e}"
                            );
                            ctx.notify();
                        }
                    },
                );
            }
            AISettingsPageAction::EnsureModelsDevLoaded => {
                use crate::ai::agent_providers::models_dev;
                let had_disk = models_dev::load_from_disk();
                if !had_disk || models_dev::is_stale() {
                    let client = http_client::Client::new();
                    ctx.spawn(
                        async move { models_dev::fetch_and_cache(client).await },
                        |view, result, ctx| {
                            if let Err(e) = result {
                                log::warn!("[models.dev] 拉取失败: {e}");
                            }
                            view.rebuild_current_page(ctx);
                        },
                    );
                } else {
                    ctx.notify();
                }
            }
            AISettingsPageAction::RefreshModelsDev => {
                use crate::ai::agent_providers::models_dev;
                let client = http_client::Client::new();
                ctx.spawn(
                    async move { models_dev::fetch_and_cache(client).await },
                    |view, result, ctx| {
                        if let Err(e) = result {
                            log::warn!("[models.dev] 刷新失败: {e}");
                        }
                        view.rebuild_current_page(ctx);
                    },
                );
            }
            AISettingsPageAction::AddProviderFromModelsDev {
                catalog_provider_id,
            } => {
                use crate::ai::agent_providers::models_dev;
                let Some(catalog) = models_dev::cached() else {
                    log::warn!("[models.dev] 目录尚未加载,无法添加 {catalog_provider_id}");
                    return;
                };
                let Some(cat_provider) = catalog.get(catalog_provider_id) else {
                    log::warn!("[models.dev] 目录中无 provider id: {catalog_provider_id}");
                    return;
                };
                let mut new_provider = crate::settings::AgentProvider::new_empty();
                new_provider.name = if cat_provider.name.is_empty() {
                    catalog_provider_id.clone()
                } else {
                    cat_provider.name.clone()
                };
                if let Some(api) = &cat_provider.api {
                    new_provider.base_url = api.clone();
                }
                new_provider.models = cat_provider
                    .models
                    .values()
                    .map(models_dev::into_agent_provider_model)
                    .collect();
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    providers.push(new_provider);
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::SyncProviderModelsFromModelsDev { provider_id } => {
                use crate::ai::agent_providers::models_dev;
                let Some(catalog) = models_dev::cached() else {
                    log::warn!("[models.dev] 目录未加载,无法同步 {provider_id}");
                    return;
                };
                let providers_snapshot = AISettings::as_ref(ctx).agent_providers.value().clone();
                let Some(local) = providers_snapshot.iter().find(|p| p.id == *provider_id) else {
                    return;
                };
                // 匹配策略:先按 base_url 完全相等 / 包含;否则按 name (大小写无关) 匹配 catalog provider id 或 name。
                let target_url = local.base_url.trim().trim_end_matches('/').to_lowercase();
                let target_name = local.name.trim().to_lowercase();
                let cat_provider = catalog.iter().find(|(_, p)| {
                    if let Some(api) = &p.api {
                        let api_norm = api.trim().trim_end_matches('/').to_lowercase();
                        if !target_url.is_empty()
                            && (api_norm == target_url
                                || api_norm.contains(&target_url)
                                || target_url.contains(&api_norm))
                        {
                            return true;
                        }
                    }
                    !target_name.is_empty()
                        && (p.name.to_lowercase() == target_name
                            || p.id.to_lowercase() == target_name)
                });
                let Some((_, cat_provider)) = cat_provider else {
                    log::warn!(
                        "[models.dev] 未在目录中找到匹配 (base_url={}, name={})",
                        local.base_url,
                        local.name
                    );
                    return;
                };
                let cat_models = cat_provider.models.clone();
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        // 既有 id 用 catalog 元数据覆盖;catalog 多出的追加;本地多出的(用户自定义)保留。
                        for local_model in p.models.iter_mut() {
                            if let Some(cat_m) = cat_models.get(&local_model.id) {
                                let merged = models_dev::into_agent_provider_model(cat_m);
                                local_model.context_window = merged.context_window;
                                local_model.max_output_tokens = merged.max_output_tokens;
                                local_model.reasoning = merged.reasoning;
                                local_model.tool_call = merged.tool_call;
                                if local_model.name.trim().is_empty() {
                                    local_model.name = merged.name;
                                }
                                // 多模态 capability:**只填 None 槽位**,Some(_) 视为用户
                                // 已显式覆盖,sync 不动。这样:
                                // - 首次 sync(用户没碰过) → 全部写入 catalog 推断结果
                                // - 用户手动 cycle 到 Some(true/false) 后再 sync → 保留覆盖
                                // - 用户三态循环回 None(Auto) → 下次 sync 又会被填上
                                if local_model.image.is_none() {
                                    local_model.image = merged.image;
                                }
                                if local_model.pdf.is_none() {
                                    local_model.pdf = merged.pdf;
                                }
                                if local_model.audio.is_none() {
                                    local_model.audio = merged.audio;
                                }
                            }
                        }
                        let existing: std::collections::HashSet<String> =
                            p.models.iter().map(|m| m.id.clone()).collect();
                        for cat_m in cat_models.values() {
                            if !existing.contains(&cat_m.id) {
                                p.models.push(models_dev::into_agent_provider_model(cat_m));
                            }
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::ToggleModelsDevChipsExpanded => {
                use crate::ai::agent_providers::models_dev;
                models_dev::toggle_chips_expanded();
                ctx.notify();
            }
            AISettingsPageAction::SetModelsDevSearchQuery(q) => {
                use crate::ai::agent_providers::models_dev;
                models_dev::set_search_query(q.clone());
                ctx.notify();
            }
            AISettingsPageAction::ToggleAgentProviderModelExpanded {
                provider_id,
                model_index,
            } => {
                super::agent_providers_widget::toggle_model_expanded(provider_id, *model_index);
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::CycleAgentProviderModelCapability {
                provider_id,
                model_index,
                kind,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            let slot = match kind {
                                ModelCapabilityKind::Image => &mut m.image,
                                ModelCapabilityKind::Pdf => &mut m.pdf,
                                ModelCapabilityKind::Audio => &mut m.audio,
                            };
                            // 三态循环:None → Some(true) → Some(false) → None。
                            *slot = match *slot {
                                None => Some(true),
                                Some(true) => Some(false),
                                Some(false) => None,
                            };
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::ToggleAgentProviderModelReasoning {
                provider_id,
                model_index,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.reasoning = !m.reasoning;
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
            AISettingsPageAction::ToggleAgentProviderModelToolCall {
                provider_id,
                model_index,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let mut providers = settings.agent_providers.value().clone();
                    if let Some(p) = providers.iter_mut().find(|p| p.id == *provider_id) {
                        if let Some(m) = p.models.get_mut(*model_index) {
                            m.tool_call = !m.tool_call;
                        }
                    }
                    let _ = settings.agent_providers.set_value(providers, ctx);
                });
                self.rebuild_current_page(ctx);
            }
        }
    }
}
impl SettingsPageMeta for AISettingsPageView {
    fn should_render(&self, app: &AppContext) -> bool {
        let _ = app;
        true
    }
    fn section() -> SettingsSection {
        SettingsSection::AI
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<AISettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<AISettingsPageView>) -> Self {
        SettingsPageViewHandle::AI(view_handle)
    }
}
fn render_ai_setting_toggle(
    label: impl Into<String>,
    action: AISettingsPageAction,
    is_setting_enabled: bool,
    is_setting_toggleable: bool,
    switch_state: SwitchStateHandle,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    build_toggle_element(
        render_body_item_label::<AISettingsPageAction>(
            label.into(),
            Some(styles::header_font_color(is_setting_toggleable, app)),
            None,
            ToggleState::Enabled,
            appearance,
        ),
        render_ai_feature_switch(
            switch_state,
            is_setting_enabled,
            is_setting_toggleable,
            action,
            app,
        ),
        appearance,
        None,
    )
}

fn render_toolbar_layout_editor(
    editor: &ViewHandle<AgentToolbarInlineEditor>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = Container::new(
        appearance
            .ui_builder()
            .span(crate::t!("settings-ai-toolbar-layout"))
            .with_style(UiComponentStyles {
                font_size: Some(CONTENT_FONT_SIZE),
                ..Default::default()
            })
            .build()
            .finish(),
    )
    .with_margin_bottom(4.)
    .finish();
    let editor = Container::new(ChildView::new(editor).finish())
        .with_margin_bottom(16.)
        .finish();

    Flex::column().with_child(label).with_child(editor).finish()
}

fn render_ai_feature_switch(
    state_handle: SwitchStateHandle,
    is_setting_enabled: bool,
    is_setting_toggleable: bool,
    toggle_action: AISettingsPageAction,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let ui_builder = appearance.ui_builder();
    ui_builder
        .switch(state_handle)
        .check(is_setting_enabled)
        .with_disabled(!is_setting_toggleable)
        .with_disabled_styles(UiComponentStyles {
            background: Some(Fill::Solid(internal_colors::neutral_4(appearance.theme()))),
            foreground: Some(Fill::Solid(internal_colors::neutral_5(appearance.theme()))),
            ..Default::default()
        })
        .build()
        .on_click(move |ctx, _, _| {
            if !is_setting_toggleable {
                return;
            }
            ctx.dispatch_typed_action(toggle_action.clone());
        })
        .finish()
}

#[derive(Default)]
struct CommandSuggestionsWidget {
    enabled: SwitchStateHandle,
    suggestions: SwitchStateHandle,
}
impl SettingsWidget for CommandSuggestionsWidget {
    type View = AISettingsPageView;
    fn search_terms(&self) -> &str {
        "command suggestions AI model provider"
    }
    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let settings = AISettings::as_ref(app);
        Flex::column()
            .with_child(build_sub_header(appearance, "Command suggestions", None).finish())
            .with_child(render_ai_setting_toggle(
                "Enable AI command suggestions",
                AISettingsPageAction::ToggleGlobalAI,
                *settings.is_any_ai_enabled,
                true,
                self.enabled.clone(),
                app,
            ))
            .with_child(render_ai_setting_toggle(
                "Suggest the next command",
                AISettingsPageAction::ToggleIntelligentAutosuggestions,
                *settings.intelligent_autosuggestions_enabled_internal,
                *settings.is_any_ai_enabled,
                self.suggestions.clone(),
                app,
            ))
            .with_child(ChildView::new(&view.command_suggestion_model_dropdown).finish())
            .finish()
    }
}
pub(crate) fn cli_agent_settings_widget_id() -> &'static str {
    CLIAgentWidget::static_widget_id()
}

#[derive(Default)]
struct CLIAgentWidget {
    cli_agent_footer_toggle: SwitchStateHandle,
    auto_toggle_rich_input_toggle: SwitchStateHandle,
    auto_toggle_rich_input_info_tooltip: MouseStateHandle,
    auto_open_rich_input_on_cli_agent_start_toggle: SwitchStateHandle,
    auto_dismiss_rich_input_toggle: SwitchStateHandle,
}

impl SettingsWidget for CLIAgentWidget {
    type View = AISettingsPageView;

    fn search_terms(&self) -> &str {
        "third party cli coding agent claude codex gemini toolbar footer layout chip chips rearrange re-arrange bar command regex auto show rich input dismiss"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let ai_settings = AISettings::as_ref(app);

        // The Coding Agents section is always enabled, independent of the
        // global AI toggle, because these settings control third-party coding
        // agents (Claude Code, Codex, Gemini CLI) rather than Warp's own AI.
        let cli_agent_footer_toggle = render_ai_setting_toggle(
            crate::t!("settings-ai-show-coding-agent-toolbar"),
            AISettingsPageAction::ToggleCLIAgentToolbar,
            *ai_settings.should_render_cli_agent_footer,
            true,
            self.cli_agent_footer_toggle.clone(),
            app,
        );

        let description_fragments = vec![
            FormattedTextFragment::plain_text(
                "Show a toolbar with quick actions when running coding agents like ",
            ),
            FormattedTextFragment::inline_code("claude"),
            FormattedTextFragment::plain_text(", "),
            FormattedTextFragment::inline_code("codex"),
            FormattedTextFragment::plain_text(", or "),
            FormattedTextFragment::inline_code("gemini"),
            FormattedTextFragment::plain_text("."),
        ];

        let description = FormattedTextElement::new(
            FormattedText::new([FormattedTextLine::Line(description_fragments)]),
            appearance.ui_font_size(),
            appearance.ui_font_family(),
            appearance.monospace_font_family(),
            styles::description_font_color(true, app).into(),
            HighlightedHyperlink::default(),
        );

        let is_footer_enabled = *ai_settings.should_render_cli_agent_footer;

        let mut column = Flex::column()
            .with_child(
                build_sub_header(
                    appearance,
                    crate::t!("settings-ai-third-party-cli-section"),
                    Some(styles::header_font_color(true, app)),
                )
                .with_padding_bottom(HEADER_PADDING)
                .finish(),
            )
            .with_child(cli_agent_footer_toggle)
            .with_child(
                Container::new(description.finish())
                    .with_margin_top(styles::DESCRIPTION_NEGATIVE_MARGIN_OFFSET)
                    .with_margin_bottom(styles::DESCRIPTION_MARGIN_BOTTOM)
                    .with_margin_right(styles::TOGGLE_WIDTH_MARGIN)
                    .finish(),
            );

        if is_footer_enabled {
            use super::settings_page::AdditionalInfo;

            if FeatureFlag::CLIAgentRichInput.is_enabled() {
                // Setting 1: Auto show/hide rich input based on agent status
                let auto_show_toggle_label = render_body_item_label::<AISettingsPageAction>(
                    crate::t!("settings-ai-auto-show-rich-input"),
                    Some(styles::header_font_color(true, app)),
                    Some(AdditionalInfo {
                        mouse_state: self.auto_toggle_rich_input_info_tooltip.clone(),
                        on_click_action: None,
                        secondary_text: None,
                        tooltip_override_text: Some(crate::t!(
                            "settings-ai-auto-show-rich-input-tooltip"
                        )),
                    }),
                    ToggleState::Enabled,
                    appearance,
                );
                column.add_child(build_toggle_element(
                    auto_show_toggle_label,
                    render_ai_feature_switch(
                        self.auto_toggle_rich_input_toggle.clone(),
                        *ai_settings.auto_toggle_rich_input,
                        true,
                        AISettingsPageAction::ToggleAutoToggleRichInput,
                        app,
                    ),
                    appearance,
                    None,
                ));

                column.add_child(render_ai_setting_toggle(
                    crate::t!("settings-ai-auto-open-rich-input"),
                    AISettingsPageAction::ToggleAutoOpenRichInputOnCLIAgentStart,
                    *ai_settings.auto_open_rich_input_on_cli_agent_start,
                    true,
                    self.auto_open_rich_input_on_cli_agent_start_toggle.clone(),
                    app,
                ));

                // Setting 2: Auto dismiss rich input after prompt submission
                column.add_child(render_ai_setting_toggle(
                    crate::t!("settings-ai-auto-dismiss-rich-input"),
                    AISettingsPageAction::ToggleAutoDismissRichInputAfterSubmit,
                    *ai_settings.auto_dismiss_rich_input_after_submit,
                    true,
                    self.auto_dismiss_rich_input_toggle.clone(),
                    app,
                ));
            }

            let command_list = {
                let mut list_column = Flex::column();

                list_column.add_child(
                    appearance
                        .ui_builder()
                        .span(crate::t!("settings-ai-toolbar-commands-label"))
                        .with_style(UiComponentStyles {
                            font_size: Some(CONTENT_FONT_SIZE),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                );

                list_column
                    .add_child(ChildView::new(&view.cli_agent_footer_command_editor).finish());

                let background = appearance.theme().surface_1();
                let font_color = appearance.theme().foreground();
                let items: Vec<_> = ai_settings
                    .cli_agent_footer_enabled_commands
                    .value()
                    .keys()
                    .cloned()
                    .collect();
                let len = items.len();
                for (rev_i, pattern) in items.iter().rev().enumerate() {
                    let original_i = len - 1 - rev_i;
                    let remove_action =
                        AISettingsPageAction::RemoveCLIAgentToolbarEnabledCommand(pattern.clone());
                    let mouse_state = view
                        .cli_agent_footer_command_mouse_state_handles
                        .get(original_i)
                        .cloned()
                        .unwrap_or_default();

                    let remove_button = appearance
                        .ui_builder()
                        .close_button(16., mouse_state)
                        .build()
                        .on_click(move |ctx, _, _| {
                            ctx.dispatch_typed_action(remove_action.clone());
                        })
                        .finish();

                    let label = appearance
                        .ui_builder()
                        .wrappable_text(pattern.clone(), true)
                        .with_style(UiComponentStyles {
                            font_color: Some(font_color.into_solid()),
                            font_family_id: Some(appearance.monospace_font_family()),
                            font_size: Some(appearance.ui_font_size()),
                            ..Default::default()
                        })
                        .build()
                        .finish();

                    let mut right_side =
                        Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
                    if let Some(dropdown_handle) = view
                        .cli_agent_footer_command_agent_dropdowns
                        .get(original_i)
                    {
                        right_side.add_child(
                            Container::new(ChildView::new(dropdown_handle).finish())
                                .with_margin_right(8.)
                                .finish(),
                        );
                    }
                    right_side.add_child(remove_button);

                    let row = Container::new(
                        Flex::row()
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                            .with_children([
                                Shrinkable::new(1., label).finish(),
                                right_side.finish(),
                            ])
                            .finish(),
                    )
                    .with_background(background)
                    .with_horizontal_padding(8.)
                    .with_vertical_padding(4.)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                    .with_margin_bottom(4.)
                    .finish();

                    list_column.add_child(row);
                }

                list_column.finish()
            };
            let command_list_description = appearance
                .ui_builder()
                .paragraph(crate::t!("settings-ai-toolbar-commands-description"))
                .with_style(UiComponentStyles {
                    font_size: Some(appearance.ui_font_size()),
                    font_color: Some(styles::description_font_color(true, app).into()),
                    margin: Some(
                        Coords::default()
                            .top(4.)
                            .bottom(styles::DESCRIPTION_MARGIN_BOTTOM)
                            .right(styles::TOGGLE_WIDTH_MARGIN),
                    ),
                    ..Default::default()
                })
                .build()
                .finish();

            column.add_child(command_list);
            column.add_child(command_list_description);

            if FeatureFlag::AgentToolbarEditor.is_enabled() {
                column.add_child(render_toolbar_layout_editor(
                    &view.cli_agent_toolbar_inline_editor,
                    appearance,
                ));
            }
        }

        column.finish()
    }
}

mod styles {
    use warp_core::ui::{appearance::Appearance, theme::Fill};
    use warpui::{AppContext, SingletonEntity};

    // Apply a negative margin to the description text so it appears closer to the main
    // settings option text.
    pub const DESCRIPTION_NEGATIVE_MARGIN_OFFSET: f32 = -12.;

    /// The space between a description and the next toggle.
    pub const DESCRIPTION_MARGIN_BOTTOM: f32 = 12.;

    /// Margin to leave for switch toggle to the right of the description subtext.
    pub const TOGGLE_WIDTH_MARGIN: f32 = 48.;

    pub fn header_font_color(is_enabled_setting: bool, app: &AppContext) -> Fill {
        let appearance = Appearance::as_ref(app);
        if is_enabled_setting {
            appearance
                .theme()
                .main_text_color(appearance.theme().surface_2())
        } else {
            appearance.theme().disabled_ui_text_color()
        }
    }

    pub fn description_font_color(is_enabled_setting: bool, app: &AppContext) -> Fill {
        let appearance = Appearance::as_ref(app);
        if is_enabled_setting {
            appearance
                .theme()
                .sub_text_color(appearance.theme().surface_1())
        } else {
            appearance.theme().disabled_ui_text_color()
        }
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(super::SettingsAction) -> T,
) {
    super::ToggleSettingActionPair::add_toggle_setting_action_pairs_as_bindings(
        vec![super::ToggleSettingActionPair::new(
            "Toggle AI command suggestions",
            builder(super::SettingsAction::AI(
                AISettingsPageAction::ToggleGlobalAI,
            )),
            context,
            super::flags::IS_ANY_AI_ENABLED,
        )],
        app,
    );
}
