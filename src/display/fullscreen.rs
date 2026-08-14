//! Fullscreen / DJ-mode viewport helpers via egui.

use egui::{Context, ViewportCommand};

pub fn toggle_fullscreen(ctx: &Context, enable: bool) {
    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(enable));
}

pub fn is_fullscreen(ctx: &Context) -> bool {
    ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
}
