# Tasks

One file per task: the plan, written before any code changes and approved
by the user (see "Workflow" in `agent.md`), and afterwards what was built
differently from it. Open ideas without a plan are in `todo.md`.

## Status

| Task | Issue | Status |
|---|---|---|
| [RMS view in the plot area](3-rms-view.md) | [#3](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/3) | done, 0.7.0 |
| [Faster movement in the channel list and overlays](7-channel-list-jump-keys.md) | [#7](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/7) | done, 0.7.0 |
| [`fault` command](fault-command.md) | - | done, 0.7.0 |
| [`locate` command](locate-command.md) | - | done, 0.7.0 |
| [`dc-offset` command](dc-offset-command.md) | - | done, 0.7.0 |
| [`compare` command](compare-command.md) | - | done, 0.7.0 |
| [`compare --trend` and `compare --plot`](compare-trend-plot.md) | - | done, 0.7.0 |
| [`trend`: clearer names for the symmetrical components](trend-sequence-names.md) | [#1](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/1) | done, 0.7.0 |
| [`trend`: a steadier frequency from the strongest signal](trend-frequency.md) | [#2](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/2) | done, 0.7.0 |
| [TUI: a harmonics view over time](tui-harmonics-view.md) | [#4](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/4) | done, 0.7.0 |
| [TUI: impedance trajectories](6-tui-impedance-view.md) | [#6](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/6) | done, 0.8.0 |
| [TUI: optional grouping of the analog traces](5-tui-analog-groups.md) | [#5](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/5) | done, 0.8.0 |
| [TUI: show the plot views as tabs](tui-view-tabs.md) | - | done, unreleased |
| [TUI: named color themes from TOML files, text styles](tui-themes.md) | - | done, unreleased |
| [`compare --plot --difference`: B − A per pair](compare-difference-plot.md) | - | done, unreleased |
| [Command line refactoring: broken pipe, shared options, helpers, exit codes, JSON](cli-refactoring.md) | - | in progress (steps 1–5 of 6) |
| [Desktop GUI with Tauri and Leptos, workspace with shared crates](gui-tauri-leptos.md) | - | planned, waiting for approval; open questions: app name, tabs |

Status values:

- **planned:** the plan is written and waits for the user's approval.
- **in progress:** approved; being built on a branch.
- **done, X.Y.Z:** built, merged into `main` and released in version X.Y.Z;
  done but not released yet: **done, unreleased**.
- **dropped:** not built; the file says why.

## A task file

- File name: `<issue>-<short-name>.md` for a GitLab issue, otherwise
  `<short-name>.md`.
- Header: the title, then `- **Issue:**` (link and title, or "none, from
  `todo.md`") and `- **Status:**` (one of the values above, then what was
  built differently from the plan).
- Sections: Goal or Problem, what was checked (background, prototypes),
  "Decisions to confirm" (a table with the proposal and the alternative),
  Output (examples), Implementation plan (library, command or TUI, tests,
  docs, checks).
- A later change to a built task gets its own section in the same file
  ("Follow-up: …") with its own status line.

When a task is added or its status changes, update its file and the table
above in the same commit.
