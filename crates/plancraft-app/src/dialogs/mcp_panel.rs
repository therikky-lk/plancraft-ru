//! The "AI Agents (MCP)" panel: start/stop the live MCP server and show how
//! to connect Claude Code or other MCP clients.
//!
//! The server requires a bearer token that changes on every start. Claude Code connects through
//! `plancraft mcp --attach`, which reads the current URL and token from an owner-only session
//! file, so its configuration keeps working across restarts; other HTTP clients get the token.

use egui::{Context, RichText};

use crate::mcp_host::McpHost;
use crate::settings::AppSettings;
use crate::theme;

/// A command or value in a code box with a copy button; `shown` may mask secrets in `text`.
fn code_line(ui: &mut egui::Ui, shown: &str, text: &str) {
    ui.horizontal(|ui| {
        egui::Frame::new().fill(theme::pal().HEADER_BG).corner_radius(4.0).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(shown).monospace()).wrap());
        });
        if ui.small_button("📋").on_hover_text(crate::tr("Copy")).clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    });
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).strong());
}

/// The running executable, quoted for a shell command line.
fn exe() -> String {
    format!("\"{}\"", exe_path(std::env::var_os("APPIMAGE"), std::env::current_exe().ok()))
}

/// The program to start PlanCraft with: the AppImage file when running from one (the AppImage
/// runtime names it in `$APPIMAGE`; the executable itself sits in a temporary mount that is gone
/// once PlanCraft closes), else the executable.
fn exe_path(appimage: Option<std::ffi::OsString>, current: Option<std::path::PathBuf>) -> String {
    match (appimage.filter(|a| !a.is_empty()), current) {
        (Some(a), _) => std::path::PathBuf::from(a).display().to_string(),
        (None, Some(p)) => p.display().to_string(),
        (None, None) => "plancraft".into(),
    }
}

pub fn show(
    ctx: &Context,
    open: &mut bool,
    host: &mut McpHost,
    settings: &mut AppSettings,
    ws: &plancraft_core::workspace::SharedWorkspace,
) {
    super::window(ctx, crate::tr("AI Agents — MCP Server")).id(egui::Id::new("mcp_panel"))
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0)).open(open).default_width(600.0).show(ctx, |ui| {
        ui.label(crate::tr("PlanCraft includes a Model Context Protocol (MCP) server. AI agents such as Claude Code can read and edit every part of your projects: tasks, milestones, dependencies, resources, calendars, time tracking, workload and leveling."));
        ui.add_space(8.0);
        ui.label(RichText::new(crate::tr("Live connection to this window")).strong().size(15.0));
        ui.label(crate::tr("Agents edit the documents open here. Changes appear immediately and can be undone like your own edits (marked ⚡ in the undo history)."));
        ui.horizontal(|ui| {
            ui.label(crate::tr("Port:"));
            let mut port = settings.mcp_port.to_string();
            if ui.add_enabled(!host.running(), egui::TextEdit::singleline(&mut port).desired_width(60.0)).changed()
                && let Ok(p) = port.parse()
            {
                settings.mcp_port = p;
            }
            if host.running() {
                if ui.button(crate::tr("■ Stop server")).clicked() {
                    host.stop();
                }
                ui.label(RichText::new(crate::tr("⚡ running")).color(theme::pal().OK));
            } else if ui.add(egui::Button::new(RichText::new(crate::tr("▶ Start server")).color(egui::Color32::WHITE)).fill(theme::pal().ACCENT)).clicked() {
                host.start(ws.clone(), ctx.clone(), settings.mcp_port);
            }
            let mut auto = settings.mcp_autostart;
            if ui.checkbox(&mut auto, crate::tr("Start automatically")).changed() {
                settings.mcp_autostart = auto;
                settings.save();
            }
        });
        if let Some(e) = &host.error {
            ui.colored_label(theme::pal().ERROR, e);
        }
        let exe = exe();
        heading(ui, crate::tr("Claude Code"));
        let attach = format!("claude mcp add plancraft -- {exe} mcp --attach");
        code_line(ui, &attach, &attach);
        ui.label(
            RichText::new(crate::tr("Add it once: it connects to this window whenever the server is running, even after PlanCraft restarts."))
                .small()
                .color(theme::pal().TEXT_WEAK),
        );
        heading(ui, "Codex");
        let codex = format!("codex mcp add plancraft -- {exe} mcp --attach");
        code_line(ui, &codex, &codex);
        ui.label(RichText::new(crate::tr("Run in a terminal, then restart Codex. Use `codex mcp list` to check the registration. Start the PlanCraft server and ask Codex to use the plancraft MCP tools.")).small().color(theme::pal().TEXT_WEAK));
        heading(ui, "GitHub Copilot (VS Code / CLI)");
        let executable = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "plancraft".into());
        let config = serde_json::json!({"mcpServers": {"plancraft": {"type": "stdio", "command": executable, "args": ["mcp", "--attach"]}}}).to_string();
        code_line(ui, &config, &config);
        ui.label(RichText::new(crate::tr("Add this JSON to ~/.copilot/mcp-config.json (merge mcpServers if the file already exists). In VS Code, select the Copilot agent and enable the server in Chat: Open Customizations > MCP Servers. Restart the agent, start the PlanCraft server, and ask it to use the plancraft MCP tools.")).small().color(theme::pal().TEXT_WEAK));
        if let (Some(url), Some(token)) = (host.url(), host.token()) {
            heading(ui, crate::tr("Other HTTP MCP clients"));
            code_line(ui, &url, &url);
            let id = egui::Id::new("mcp_show_token");
            let mut show_token = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
            let header = format!("--header \"Authorization: Bearer {token}\"");
            let masked = format!("--header \"Authorization: Bearer {}\"", "•".repeat(16));
            ui.horizontal(|ui| {
                code_line(ui, if show_token { &header } else { &masked }, &header);
                let label = if show_token { crate::tr("Hide") } else { crate::tr("Show") };
                if ui.toggle_value(&mut show_token, label).on_hover_text(crate::tr("Show the bearer token")).changed() {
                    ctx.data_mut(|d| d.insert_temp(id, show_token));
                }
            });
            ui.label(
                RichText::new(crate::tr("Clients must send this bearer token. It changes each time the server starts.")).small().color(theme::pal().TEXT_WEAK),
            );
            ui.add_space(2.0);
            ui.label(RichText::new(crate::tr_dyn(&format!("Changes made by agents this session: {}", host.changes()))).small().color(theme::pal().TEXT_WEAK));
        }
        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new(crate::tr("Headless (stdio) mode")).strong().size(15.0));
        ui.label(crate::tr("Agents can also run PlanCraft without the window, working directly on files:"));
        let headless = format!("claude mcp add plancraft-files -- {exe} mcp path/to/project.plancraft");
        code_line(ui, &headless, &headless);
        ui.label(RichText::new(crate::tr("Only connect agents you trust: they can change and save your project files.")).small().color(theme::pal().TEXT_WEAK));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: from an AppImage the panel offered the temporary mount's path, which stops
    /// existing when PlanCraft closes, so the agent setup command it showed never worked.
    #[test]
    fn the_command_names_the_appimage_when_there_is_one() {
        let mounted = Some(std::path::PathBuf::from("/tmp/.mount_PlanCrAbc/usr/bin/plancraft"));
        assert_eq!(exe_path(Some("/home/a/PlanCraft.AppImage".into()), mounted.clone()), "/home/a/PlanCraft.AppImage");
        assert_eq!(exe_path(None, mounted.clone()), "/tmp/.mount_PlanCrAbc/usr/bin/plancraft");
        assert_eq!(exe_path(Some("".into()), mounted), "/tmp/.mount_PlanCrAbc/usr/bin/plancraft");
        assert_eq!(exe_path(None, None), "plancraft");
    }
}
