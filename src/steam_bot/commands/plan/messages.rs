// SPDX-License-Identifier: GPL-3.0-only

pub(super) struct PlanMessageContext {
    pub time_str: String,
    pub actor_mention: Option<String>,
    pub server_id: Option<u8>,
    pub server_ip: Option<String>,
    pub server_port: Option<u16>,
    pub server_name: Option<String>,
    pub requested_map: Option<String>,
    pub is_replan: bool,
}

pub(super) enum ClearResult {
    TargetedCleared { server_id: u8 },
    TargetedNotFound { server_id: u8, global_active: bool },
    GlobalCleared,
}

pub(super) fn format_already_planned(time_str: &str) -> String {
    format!("Lobby is already planned at {}", time_str)
}

pub(super) fn format_clear_response(actor_mention: Option<&str>, result: ClearResult) -> String {
    match result {
        ClearResult::TargetedCleared { server_id } => match actor_mention {
            Some(actor_mention) => {
                format!("{} cleared the reservation for server {}.", actor_mention, server_id)
            }
            None => format!("Reservation cleared for server {}.", server_id),
        },
        ClearResult::TargetedNotFound {
            server_id,
            global_active,
        } => {
            if global_active {
                format!(
                    "No targeted reservation for server {}. A global reservation is active; use !plan clear to clear all.",
                    server_id
                )
            } else {
                format!("No targeted reservation for server {}.", server_id)
            }
        }
        ClearResult::GlobalCleared => match actor_mention {
            Some(actor_mention) => format!("{} cleared the reservation.", actor_mention),
            None => "Reservation cleared.".to_string(),
        },
    }
}

fn format_plan_body(ctx: &PlanMessageContext) -> String {
    if let Some(server_id) = ctx.server_id {
        let server_ip = ctx
            .server_ip
            .as_deref()
            .expect("server_ip required when server_id is set");
        let server_port = ctx
            .server_port
            .expect("server_port required when server_id is set");

        if let Some(actor_mention) = ctx.actor_mention.as_deref() {
            if ctx.is_replan {
                return match ctx.server_name.as_deref() {
                    Some(server_name) => format!(
                        "{} re-planned lobby time at {} for server {}\nServer: {} | IP: {}:{}",
                        actor_mention, ctx.time_str, server_id, server_name, server_ip, server_port
                    ),
                    None => format!(
                        "{} re-planned lobby time at {} for server {}\nServer IP: {}:{}",
                        actor_mention, ctx.time_str, server_id, server_ip, server_port
                    ),
                };
            }

            return match ctx.server_name.as_deref() {
                Some(server_name) => format!(
                    "{} planned lobby time at {} for server {} [mention=all]@all[/mention]\nServer: {} | IP: {}:{}",
                    actor_mention, ctx.time_str, server_id, server_name, server_ip, server_port
                ),
                None => format!(
                    "{} planned lobby time at {} for server {} [mention=all]@all[/mention]\nServer IP: {}:{}",
                    actor_mention, ctx.time_str, server_id, server_ip, server_port
                ),
            };
        }

        if ctx.is_replan {
            return match ctx.server_name.as_deref() {
                Some(server_name) => format!(
                    "Re-planned lobby time for server {}: {}\nServer: {} | IP: {}:{}",
                    server_id, ctx.time_str, server_name, server_ip, server_port
                ),
                None => format!(
                    "Re-planned lobby time for server {}: {}\nServer IP: {}:{}",
                    server_id, ctx.time_str, server_ip, server_port
                ),
            };
        }

        return match ctx.server_name.as_deref() {
            Some(server_name) => format!(
                "Planned lobby time for server {}: {} [mention=all]@all[/mention]\nServer: {} | IP: {}:{}",
                server_id, ctx.time_str, server_name, server_ip, server_port
            ),
            None => format!(
                "Planned lobby time for server {}: {} [mention=all]@all[/mention]\nServer IP: {}:{}",
                server_id, ctx.time_str, server_ip, server_port
            ),
        };
    }

    if let Some(actor_mention) = ctx.actor_mention.as_deref() {
        if ctx.is_replan {
            format!("{} re-planned lobby time at {}", actor_mention, ctx.time_str)
        } else {
            format!(
                "{} planned lobby time at {} [mention=all]@all[/mention]",
                actor_mention, ctx.time_str
            )
        }
    } else if ctx.is_replan {
        format!("Re-planned lobby time: {}", ctx.time_str)
    } else {
        format!(
            "Planned lobby time: {} [mention=all]@all[/mention]",
            ctx.time_str
        )
    }
}

pub(super) fn format_plan_response(ctx: PlanMessageContext) -> String {
    let mut response = format_plan_body(&ctx);
    if let Some(requested_map) = ctx.requested_map {
        response.push('\n');
        response.push_str("Requested map: ");
        response.push_str(&requested_map);
    }
    response
}
