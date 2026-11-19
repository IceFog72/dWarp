use std::collections::{HashMap, HashSet};
use std::time::Duration;

use settings::Setting as _;
use sysinfo::{CpuRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use warpui::{AppContext, SingletonEntity, ViewContext};

use super::{
    resolve_vertical_tabs_mode, TerminalPane, VerticalTabsResolvedMode, Workspace,
    CLI_USAGE_OPTIONS,
};
use crate::workspace::tab_settings::{
    TabSettings, VerticalTabsCompactSubtitle, VerticalTabsViewMode,
};
use crate::FeatureFlag;

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct CliUsage {
    by_shell_pid: HashMap<u32, ProcessUsage>,
    refreshing: bool,
}

struct ProcessUsage {
    cpu_percent: Option<f32>,
    resident_bytes: u64,
}

impl CliUsage {
    pub(super) fn subtitle(&self, pid: u32, mode: VerticalTabsCompactSubtitle) -> String {
        let usage = self.by_shell_pid.get(&pid);
        let cpu = || match usage.and_then(|usage| usage.cpu_percent) {
            Some(percent) => format!("CPU {percent:.1}%"),
            None => "CPU —".into(),
        };
        let ram = || match usage {
            Some(usage) => format!("RAM {:.1} MiB", usage.resident_bytes as f64 / 1_048_576.),
            None => "RAM —".into(),
        };

        match mode {
            VerticalTabsCompactSubtitle::Cpu => cpu(),
            VerticalTabsCompactSubtitle::Ram => ram(),
            VerticalTabsCompactSubtitle::CpuAndRam
            | VerticalTabsCompactSubtitle::CommandCpuAndRam => format!("{} · {}", cpu(), ram()),
            VerticalTabsCompactSubtitle::Branch
            | VerticalTabsCompactSubtitle::WorkingDirectory
            | VerticalTabsCompactSubtitle::Command => unreachable!(),
        }
    }
}

impl Workspace {
    pub(in crate::workspace::view) fn sync_vertical_tabs_cli_usage(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.should_sample_cli_usage(ctx) {
            self.vertical_tabs_panel.cli_usage.by_shell_pid.clear();
        } else if !self.vertical_tabs_panel.cli_usage.refreshing {
            self.refresh_cli_usage(System::new(), ctx);
        }
    }

    fn should_sample_cli_usage(&self, app: &AppContext) -> bool {
        let settings = TabSettings::as_ref(app);
        self.vertical_tabs_panel_open
            && FeatureFlag::VerticalTabs.is_enabled()
            && *settings.use_vertical_tabs
            && *settings.vertical_tabs_view_mode.value() == VerticalTabsViewMode::Compact
            && CLI_USAGE_OPTIONS.contains(settings.vertical_tabs_compact_subtitle.value())
            && !matches!(
                resolve_vertical_tabs_mode(app),
                VerticalTabsResolvedMode::Summary
            )
    }

    fn refresh_cli_usage(&mut self, mut system: System, ctx: &mut ViewContext<Self>) {
        if !self.should_sample_cli_usage(ctx) {
            self.vertical_tabs_panel.cli_usage.refreshing = false;
            return;
        }
        self.vertical_tabs_panel.cli_usage.refreshing = true;

        let app: &AppContext = ctx;
        let shell_pids: Vec<u32> = self
            .tabs
            .iter()
            .flat_map(|tab| {
                let group = tab.pane_group.as_ref(app);
                group.visible_pane_ids().into_iter().filter_map(move |id| {
                    let pane = group.downcast_pane_by_id::<TerminalPane>(id)?;
                    pane.terminal_view(app).as_ref(app).local_shell_pid
                })
            })
            .collect();

        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || {
                    let usage = sample_process_usage(&mut system, &shell_pids);
                    (system, usage)
                })
                .await
            },
            |workspace, result, ctx| {
                if !workspace.should_sample_cli_usage(ctx) {
                    workspace.vertical_tabs_panel.cli_usage.refreshing = false;
                    return;
                }
                let (system, usage) = result.unwrap_or_default();
                workspace.vertical_tabs_panel.cli_usage.by_shell_pid = usage;
                ctx.notify();
                ctx.spawn(
                    async { warpui::r#async::Timer::after(REFRESH_INTERVAL).await },
                    move |workspace, _, ctx| workspace.refresh_cli_usage(system, ctx),
                );
            },
        );
    }
}

fn sample_process_usage(system: &mut System, shell_pids: &[u32]) -> HashMap<u32, ProcessUsage> {
    if shell_pids.is_empty() {
        return HashMap::new();
    }

    if system.cpus().is_empty() {
        system.refresh_cpu_specifics(CpuRefreshKind::nothing());
    }

    // sysinfo computes CPU usage from two snapshots. Linux tasks are threads, so including
    // them would count the same process memory more than once.
    let cpu_ready = !system.processes().is_empty();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .without_tasks(),
    );
    let cpu_count = system.cpus().len();
    let processes = system.processes();
    let mut children = HashMap::<Pid, Vec<Pid>>::new();
    for (&pid, process) in processes {
        if let Some(parent) = process.parent().filter(|parent| {
            processes
                .get(parent)
                .is_some_and(|parent| parent.start_time() <= process.start_time())
        }) {
            children.entry(parent).or_default().push(pid);
        }
    }

    shell_pids
        .iter()
        .filter_map(|&shell_pid| {
            let root = Pid::from_u32(shell_pid);
            processes.get(&root)?;
            let mut pending = vec![root];
            let mut seen = HashSet::new();
            let mut cpu_percent = 0.;
            let mut resident_bytes = 0;
            while let Some(pid) = pending.pop() {
                if !seen.insert(pid) {
                    continue;
                }
                let process = &processes[&pid];
                cpu_percent += process.cpu_usage();
                resident_bytes += process.memory();
                if let Some(children) = children.get(&pid) {
                    pending.extend(children);
                }
            }
            Some((
                shell_pid,
                ProcessUsage {
                    // sysinfo reports 100% per logical CPU. Display a share of total capacity.
                    cpu_percent: (cpu_ready && cpu_count > 0)
                        .then(|| (cpu_percent / cpu_count as f32).clamp(0., 100.)),
                    resident_bytes,
                },
            ))
        })
        .collect()
}
