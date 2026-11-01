//! BYOP command suggestions from local terminal context.

use minijinja::{context, Environment};
use serde::Serialize;
use std::sync::OnceLock;

use super::oneshot::{
    byop_oneshot_completion, resolve_command_suggestion_oneshot, OneshotConfig, OneshotOptions,
};

pub mod parsing;

// ---------------------------------------------------------------------------
// 模板
// ---------------------------------------------------------------------------

static ENV: OnceLock<Environment<'static>> = OnceLock::new();

fn build_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template(
        "next_command_system.j2",
        include_str!("../prompts/active_ai/next_command_system.j2"),
    )
    .expect("next_command_system parses");
    env.add_template(
        "next_command_user.j2",
        include_str!("../prompts/active_ai/next_command_user.j2"),
    )
    .expect("next_command_user parses");
    env
}

fn env() -> &'static Environment<'static> {
    ENV.get_or_init(build_env)
}

fn render(template: &str, ctx: minijinja::Value) -> String {
    env()
        .get_template(template)
        .and_then(|t| t.render(ctx))
        .unwrap_or_else(|e| {
            log::warn!("[active_ai] render {template} failed: {e}");
            String::new()
        })
}

// ---------------------------------------------------------------------------
// 公共上下文片段
// ---------------------------------------------------------------------------

/// 单条已完成命令块的精简上下文(供 next_command 消费)。
#[derive(Debug, Clone, Serialize, Default)]
pub struct BlockSnippet {
    pub command: String,
    pub output_summary: String,
    pub exit_code: i32,
    pub pwd: String,
}

pub mod next_command {
    use super::*;
    use warpui::AppContext;

    pub struct Input {
        pub recent_blocks: Vec<BlockSnippet>,
        /// 已在 client 端从历史 DB 选出的相似命令上下文(可选)。
        pub history_context: String,
        pub system_context: Option<String>,
        /// 用户已输入的前缀(必须用作输出前缀)。
        pub prefix: Option<String>,
        /// 之前已 reject 的建议(避免重复)。
        pub rejected_suggestions: Vec<String>,
    }

    /// Pre-spawn:解 BYOP 配置(需要 `&AppContext`)。`None` ⇒ 静默 no-op。
    pub fn resolve(app: &AppContext) -> Option<OneshotConfig> {
        resolve_command_suggestion_oneshot(app)
    }

    /// In-spawn:用 cfg + Input 渲染 prompt 并发请求。
    /// 模板渲染不依赖 AppContext,可在 spawn 内同步调用。
    pub async fn run_with(cfg: OneshotConfig, input: Input) -> Option<String> {
        let system = render("next_command_system.j2", context! {});
        let user = render(
            "next_command_user.j2",
            context! {
                recent_blocks => input.recent_blocks,
                history_context => input.history_context,
                system_context => input.system_context,
                prefix => input.prefix,
                rejected_suggestions => input.rejected_suggestions,
            },
        );
        let opts = OneshotOptions {
            response_format_json: false,
            max_chars: Some(8000),
            ..Default::default()
        };
        let raw = match byop_oneshot_completion(&cfg, &system, &user, &opts).await {
            Ok(s) => s,
            Err(e) => {
                log::debug!("[active_ai] next_command oneshot failed: {e:#}");
                return None;
            }
        };
        log::info!(
            "[active_ai] next_command raw response ({} chars): {raw:?}",
            raw.len()
        );
        let sanitized = parsing::sanitize_predict(&raw);
        if sanitized.is_none() && !raw.trim().is_empty() {
            log::warn!("[active_ai] next_command sanitize REJECTED raw response");
        }
        sanitized
    }
}
