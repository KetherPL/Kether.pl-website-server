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
	request: Json<CreateBindSuggestionRequest>,
) -> Result<Json<BindSuggestion>, Status> {
	let proposed_by = user.0.to_string();
	db.create_bind_suggestion(
		request.author.clone(),
		request.text.clone(),
		proposed_by,
	).await
	.map(Json)
	.map_err(|e| storage_error_status("creating bind suggestion", &e, &[], &["already exists"]))
}

#[post("/bind_suggestions/deleteBindSuggestion", data = "<request>")]
pub async fn delete_bind_suggestion(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	request: Json<DeleteBindSuggestionRequest>,
) -> Result<Json<usize>, Status> {
	db.delete_bind_suggestion(request.id).await
		.map(|_| Json(1))
		.map_err(|e| storage_error_status("deleting bind suggestion", &e, &["not found"], &[]))
}

#[post("/bind_suggestions/updateBindSuggestion", data = "<bind_suggestion_to_update>")]
pub async fn update_bind_suggestion(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
	db.update_bind_suggestion(
		bind_suggestion_to_update.id,
		bind_suggestion_to_update.author.clone(),
		bind_suggestion_to_update.text.clone(),
		bind_suggestion_to_update.proposed_by.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating bind suggestion", &e, &["not found"], &["already exists"]))
}

#[put("/bind_suggestions/<bind_suggestion_id>", data = "<bind_suggestion_to_update>")]
pub async fn update_bind_suggestion_by_id(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	bind_suggestion_id: i32,
	bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
	db.update_bind_suggestion(
		bind_suggestion_id,
		bind_suggestion_to_update.author.clone(),
		bind_suggestion_to_update.text.clone(),
		bind_suggestion_to_update.proposed_by.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating bind suggestion by id", &e, &["not found"], &["already exists"]))
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

