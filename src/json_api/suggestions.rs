// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	get, options, post, put, routes,
	http::Status,
	serde::json::Json,
	Route, State,
};
use rocket::serde::{Deserialize, Serialize};

use crate::json_api::models::BindSuggestion;
use crate::json_api::utils::{ok_status, storage_error_status};
use crate::json_storage::JsonDatabase;
use crate::admin_actions::{config_snapshot as audit_config_snapshot, ActionChoice, AdminAction, AdminActionLog, RecordedChange};
use rocket::serde::json::json;
use crate::auth::{AdminUser, AuthUser};
use crate::steam_bot::registry::ConfigHandle;

#[cfg(feature = "rest_steam")]
fn is_steam_id64(value: &str) -> bool {
	value.len() == 17 && value.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(feature = "rest_steam")]
fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<crate::config::Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in suggestions: {}", e);
			e.into_inner().clone()
		}
	}
}

#[cfg(feature = "rest_steam")]
async fn resolve_proposed_by_names(
	suggestions: &mut [BindSuggestion],
	config: &State<ConfigHandle>,
) {
	let config = config_snapshot(config);
	let mut unique_ids = Vec::new();
	for suggestion in suggestions.iter() {
		if is_steam_id64(&suggestion.proposed_by)
			&& let Ok(id) = suggestion.proposed_by.parse::<u64>()
			&& !unique_ids.contains(&id)
		{
			unique_ids.push(id);
		}
	}

	if unique_ids.is_empty() {
		return;
	}

	let names = crate::steam_rest::resolve_persona_names(&config, &unique_ids).await;
	for suggestion in suggestions.iter_mut() {
		if !is_steam_id64(&suggestion.proposed_by) {
			continue;
		}
		let Ok(id) = suggestion.proposed_by.parse::<u64>() else {
			continue;
		};
		if let Some(name) = names.get(&id) {
			suggestion.proposed_by = name.clone();
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct CreateBindSuggestionRequest {
	pub author: String,
	pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DeleteBindSuggestionRequest {
	pub id: i32,
}

#[get("/bind_suggestions/<bind_suggestion_id>")]
pub fn get_bind_suggestion(db: &State<JsonDatabase>, bind_suggestion_id: i32) -> Result<Json<BindSuggestion>, Status> {
	db.get_bind_suggestion(bind_suggestion_id)
		.map(Json)
		.map_err(|e| storage_error_status("getting bind suggestion", &e, &["not found"], &[]))
}

#[get("/bind_suggestions/getBindSuggestions")]
pub async fn list_bind_suggestions(
	db: &State<JsonDatabase>,
	config: &State<ConfigHandle>,
) -> Result<Json<Vec<BindSuggestion>>, Status> {
	let mut suggestions = db.get_all_bind_suggestions()
		.map_err(|e| storage_error_status("listing bind suggestions", &e, &[], &[]))?;

	#[cfg(feature = "rest_steam")]
	resolve_proposed_by_names(&mut suggestions, config).await;
	#[cfg(not(feature = "rest_steam"))]
	let _ = config;

	Ok(Json(suggestions))
}

#[post("/bind_suggestions/addBindSuggestion", data = "<request>")]
pub async fn create_bind_suggestion(
	user: AuthUser,
	db: &State<JsonDatabase>,
	config_handle: &State<ConfigHandle>,
	log: &State<AdminActionLog>,
	request: Json<CreateBindSuggestionRequest>,
) -> Result<Json<BindSuggestion>, Status> {
	let config = audit_config_snapshot(config_handle);
	let _guard = log.lock_content().await;
	let data = json!({"submitted": &request.0});
	let operation = async {
		let proposed_by = user.0.to_string();
		db.create_bind_suggestion(request.author.clone(), request.text.clone(), proposed_by)
			.await
			.map(|value| {
				let data = json!({"after": &value});
				RecordedChange::completed(Json(value), data)
			})
			.map_err(|e| {
				(
					storage_error_status("creating bind suggestion", &e, &[], &["already exists"]),
					e,
				)
			})
	};
	if config.is_admin(user.0) {
		log.record_change(
			&config,
			user.0,
			AdminAction::BindSuggestion,
			ActionChoice::Create,
			data,
			operation,
		)
		.await
		.map_err(|(status, _)| status)
	} else {
		operation
			.await
			.map(|change| change.value)
			.map_err(|(status, _)| status)
	}
}

#[post("/bind_suggestions/deleteBindSuggestion", data = "<request>")]
pub async fn delete_bind_suggestion(
	admin: AdminUser,
	db: &State<JsonDatabase>,
	config_handle: &State<ConfigHandle>,
	log: &State<AdminActionLog>,
	request: Json<DeleteBindSuggestionRequest>,
) -> Result<Json<usize>, Status> {
	let config = audit_config_snapshot(config_handle);
	let _guard = log.lock_content().await;
	let before = db.get_bind_suggestion(request.id).map_err(|error| {
		storage_error_status("deleting bind suggestion", &error, &["not found"], &[])
	})?;
	let data = json!({"id": request.id, "before": &before});
	log.record_change(
		&config,
		admin.0,
		AdminAction::BindSuggestion,
		ActionChoice::Delete,
		data,
		async {
			db.delete_bind_suggestion(request.id)
				.await
				.map(|_| RecordedChange::completed(Json(1), json!({"after": null})))
				.map_err(|e| {
					(
						storage_error_status("deleting bind suggestion", &e, &["not found"], &[]),
						e,
					)
				})
		},
	)
	.await
	.map_err(|(status, _)| status)
}

#[post("/bind_suggestions/updateBindSuggestion", data = "<bind_suggestion_to_update>")]
pub async fn update_bind_suggestion(
	admin: AdminUser,
	db: &State<JsonDatabase>,
	config_handle: &State<ConfigHandle>,
	log: &State<AdminActionLog>,
	bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
	let config = audit_config_snapshot(config_handle);
	let _guard = log.lock_content().await;
	let before = db
		.get_bind_suggestion(bind_suggestion_to_update.id)
		.map_err(|error| {
			storage_error_status("updating bind suggestion", &error, &["not found"], &[])
		})?;
	let data = json!({"id": bind_suggestion_to_update.id, "before": &before});
	log.record_change(
		&config,
		admin.0,
		AdminAction::BindSuggestion,
		ActionChoice::Update,
		data,
		async {
			db.update_bind_suggestion(
				bind_suggestion_to_update.id,
				bind_suggestion_to_update.author.clone(),
				bind_suggestion_to_update.text.clone(),
				bind_suggestion_to_update.proposed_by.clone(),
			)
			.await
			.map(|value| {
				let data = json!({"after": &value});
				RecordedChange::completed(Json(value), data)
			})
			.map_err(|e| {
				(
					storage_error_status(
						"updating bind suggestion",
						&e,
						&["not found"],
						&["already exists"],
					),
					e,
				)
			})
		},
	)
	.await
	.map_err(|(status, _)| status)
}

#[put("/bind_suggestions/<bind_suggestion_id>", data = "<bind_suggestion_to_update>")]
pub async fn update_bind_suggestion_by_id(
	admin: AdminUser,
	db: &State<JsonDatabase>,
	config_handle: &State<ConfigHandle>,
	log: &State<AdminActionLog>,
	bind_suggestion_id: i32,
	bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
	let config = audit_config_snapshot(config_handle);
	let _guard = log.lock_content().await;
	let before = db
		.get_bind_suggestion(bind_suggestion_id)
		.map_err(|error| {
			storage_error_status(
				"updating bind suggestion by id",
				&error,
				&["not found"],
				&[],
			)
		})?;
	let data = json!({"id": bind_suggestion_id, "before": &before});
	log.record_change(
		&config,
		admin.0,
		AdminAction::BindSuggestion,
		ActionChoice::Update,
		data,
		async {
			db.update_bind_suggestion(
				bind_suggestion_id,
				bind_suggestion_to_update.author.clone(),
				bind_suggestion_to_update.text.clone(),
				bind_suggestion_to_update.proposed_by.clone(),
			)
			.await
			.map(|value| {
				let data = json!({"after": &value});
				RecordedChange::completed(Json(value), data)
			})
			.map_err(|e| {
				(
					storage_error_status(
						"updating bind suggestion by id",
						&e,
						&["not found"],
						&["already exists"],
					),
					e,
				)
			})
		},
	)
	.await
	.map_err(|(status, _)| status)
}

#[options("/bind_suggestions/addBindSuggestion")]
pub fn options_create_bind_suggestion() -> Status {
	ok_status()
}

#[options("/bind_suggestions/deleteBindSuggestion")]
pub fn options_delete_bind_suggestion() -> Status {
	ok_status()
}

#[options("/bind_suggestions/updateBindSuggestion")]
pub fn options_update_bind_suggestion() -> Status {
	ok_status()
}

pub fn routes() -> Vec<Route> {
	routes![
		get_bind_suggestion,
		list_bind_suggestions,
		create_bind_suggestion,
		delete_bind_suggestion,
		update_bind_suggestion,
		update_bind_suggestion_by_id,
		options_create_bind_suggestion,
		options_delete_bind_suggestion,
		options_update_bind_suggestion,
	]
}

