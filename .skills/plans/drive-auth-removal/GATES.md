# Gates: Drive and auth removal

OWNS: app/**, crates/**, Cargo.toml, Cargo.lock, .skills/plans/drive-auth-removal/**

Scope: Finish removal of Warp Drive, login/signup, and Warp-server features while preserving third-party CLI agents, BYOP suggestions, SSH, and local terminal behavior.

- [ ] G1: Drive and cloud object panes, actions, search results, and persistence callers are removed coherently.
  EVIDENCE: pending source review and compiler validation.

- [ ] G2: Login/signup and Warp-server callers, telemetry, and background synchronization are removed.
  EVIDENCE: pending source and endpoint review.

- [ ] G3: Third-party CLI agents, BYOP suggestions, SSH, and local history retain their implementations and settings.
  EVIDENCE: pending source review and behavioral validation.

- [ ] G4: The application compiles and relevant checks pass.
  EVIDENCE: pending cargo check, formatting, clippy, and relevant tests.

- [ ] G5: The application starts and supports local commands and session restore.
  EVIDENCE: pending runtime validation.

Decision: Deleted cloud modules cannot own live local state. Remove their callers at each domain owner. Preserve local file notebooks, workflows, and third-party CLI functionality where current code still uses them. A compiler pass alone cannot prove absence of network traffic or singleton panics, so source and runtime checks remain separate gates.

Current validation notes (2026-11-01):
- Direct user scope now includes removing the Privacy page and unused references. Direct user timing: run tests after code, then commit all workspace changes.
- Single-agent work only. Existing staged and unstaged work is preserved.
- All-feature application/test compilation passed in /tmp/warp-drive-check-tests-v13.jsonl before the final orphan GraphQL-operation cleanup.
- Final workspace nextest run is in progress in /tmp/warp-drive-workspace-tests-v3.log.
- Compatibility-only saved pane, toolbar, theme, and history identifiers remain where old local state needs them. GraphQL transport, operations, subscriptions, backend configuration, Firebase, managed cloud secrets, referral model, reward modal, network console, and Privacy UI have been removed.
- SSH uses OS-account authentication and a deployed remote server. Backend automatic installation has been removed. Localhost has no SSH listener for a full connection smoke test.
- The existing application-wide dead_code suppression was temporarily removed to audit orphaned code. This exposed additional unrelated and test-only unused APIs. It remains removed, so lint results must report those warnings honestly.
