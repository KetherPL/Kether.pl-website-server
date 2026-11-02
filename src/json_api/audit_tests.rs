// SPDX-License-Identifier: GPL-3.0-only

use rocket::http::{ContentType, Header, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{Value, json};

use crate::admin_actions::{ActionChoice, AdminActionLog};
use crate::config::Config;
use crate::json_storage::JsonDatabase;
use crate::steam_bot::registry::ConfigHandle;

const ADMIN_ID: i64 = 76561198000000001;
const USER_ID: i64 = 76561198000000002;

async fn client(enabled: bool) -> (Client, Config) {
	let config = Config::from_toml_str(&format!(
		r#"
frontend_admins = [{ADMIN_ID}]
admin_action_logging = {enabled}
[auth]
session_secret = "test-audit-secret-that-is-long-enough-for-hs256"
csrf_allowed_origins = ["https://kether.pl"]
"#
	))
	.expect("config");
	let handle: ConfigHandle =
		std::sync::Arc::new(std::sync::RwLock::new(std::sync::Arc::new(config.clone())));
	let directory = tempfile::tempdir().expect("directory");
	let rocket = rocket::build()
		.manage(handle)
		.manage(JsonDatabase::in_memory_for_tests())
		.manage(AdminActionLog::new(
			directory.path().join("log_admin_actions.json"),
		))
		.manage(directory)
		.mount("/api", crate::json_cmds_binds_rest::mount_json_routes());
	(Client::tracked(rocket).await.expect("client"), config)
}

async fn post<'a>(
	client: &'a Client,
	config: &Config,
	user: i64,
	path: &str,
	body: Value,
) -> rocket::local::asynchronous::LocalResponse<'a> {
	let token = crate::auth::mint_session(user, config).expect("token");
	client
		.post(path.to_string())
		.header(Header::new("Authorization", format!("Bearer {token}")))
		.header(Header::new("Origin", "https://kether.pl"))
		.header(ContentType::JSON)
		.body(body.to_string())
		.dispatch()
		.await
}

#[rocket::async_test]
async fn admin_bind_edits_and_deletion_preserve_authoritative_full_versions() {
	let (client, config) = client(true).await;
	let created = post(
		&client,
		&config,
		ADMIN_ID,
		"/api/binds/addBind",
		json!({"author":"Original author", "text":"Original text"}),
	)
	.await;
	assert_eq!(created.status(), Status::Ok);
	let bind: Value = created.into_json().await.expect("bind");
	let id = bind["id"].as_i64().expect("id");
	let updated = post(
		&client,
		&config,
		ADMIN_ID,
		"/api/binds/updateBind",
		json!({"id":id,"author":"Edited author","text":"Edited text","upvote":[],"downvote":[]}),
	)
	.await;
	assert_eq!(updated.status(), Status::Ok);
	let deleted = post(
		&client,
		&config,
		ADMIN_ID,
		"/api/binds/deleteBind",
		json!({"id":id,"author":"Forged author","text":"Forged text"}),
	)
	.await;
	assert_eq!(deleted.status(), Status::Ok);
	let entries = client
		.rocket()
		.state::<AdminActionLog>()
		.expect("log")
		.history(15)
		.await
		.expect("history");
	assert_eq!(entries.len(), 3);
	assert_eq!(entries[0].action_id, 5);
	assert_eq!(entries[0].choice, ActionChoice::Delete);
	let deleted_data = entries[0]
		.additional_data
		.as_ref()
		.expect("deleted snapshot");
	assert_eq!(deleted_data["before"]["author"], "Edited author");
	assert_eq!(deleted_data["before"]["text"], "Edited text");
	assert_eq!(deleted_data["after"], Value::Null);
	let edit_data = entries[1].additional_data.as_ref().expect("edit snapshot");
	assert_eq!(edit_data["before"]["author"], "Original author");
	assert_eq!(edit_data["before"]["text"], "Original text");
	assert_eq!(edit_data["after"]["text"], "Edited text");
}

#[rocket::async_test]
async fn admin_command_put_uses_path_id_and_records_deleted_command_contents() {
	let (client, config) = client(true).await;
	post(
		&client,
		&config,
		ADMIN_ID,
		"/api/commands/addCommand",
		json!({"command":"!old","description":"Original description"}),
	)
	.await;
	let token = crate::auth::mint_session(ADMIN_ID, &config).expect("token");
	let updated = client
		.put("/api/commands/1")
		.header(Header::new("Authorization", format!("Bearer {token}")))
		.header(Header::new("Origin", "https://kether.pl"))
		.header(ContentType::JSON)
		.body(json!({"id":999,"command":"!new","description":"Edited description"}).to_string())
		.dispatch()
		.await;
	assert_eq!(updated.status(), Status::Ok);
	assert_eq!(
		post(
			&client,
			&config,
			ADMIN_ID,
			"/api/commands/deleteCommand",
			json!({"id":1})
		)
		.await
		.status(),
		Status::Ok
	);
	let entries = client
		.rocket()
		.state::<AdminActionLog>()
		.expect("log")
		.history(15)
		.await
		.expect("history");
	assert_eq!(entries[0].action_id, 7);
	assert_eq!(
		entries[0].additional_data.as_ref().expect("deleted")["before"]["command"],
		"!new"
	);
	assert_eq!(
		entries[0].additional_data.as_ref().expect("deleted")["before"]["description"],
		"Edited description"
	);
	assert_eq!(
		entries[1].additional_data.as_ref().expect("edited")["before"]["command"],
		"!old"
	);
	assert_eq!(
		entries[1].additional_data.as_ref().expect("edited")["after"]["id"],
		1
	);
}

#[rocket::async_test]
async fn admin_suggestion_changes_are_logged_but_normal_user_submissions_are_not() {
	let (client, config) = client(true).await;
	assert_eq!(
		post(
			&client,
			&config,
			USER_ID,
			"/api/bind_suggestions/addBindSuggestion",
			json!({"author":"Author","text":"Suggestion text"})
		)
		.await
		.status(),
		Status::Ok
	);
	let log = client.rocket().state::<AdminActionLog>().expect("log");
	assert!(log.history(15).await.expect("history").is_empty());
	assert_eq!(post(&client, &config, ADMIN_ID, "/api/bind_suggestions/updateBindSuggestion", json!({"id":1,"author":"Edited author","text":"Edited suggestion","proposed_by":USER_ID.to_string()})).await.status(), Status::Ok);
	assert_eq!(
		post(
			&client,
			&config,
			ADMIN_ID,
			"/api/bind_suggestions/deleteBindSuggestion",
			json!({"id":1})
		)
		.await
		.status(),
		Status::Ok
	);
	let entries = log.history(15).await.expect("history");
	assert_eq!(entries.len(), 2);
	assert_eq!(entries[0].action_id, 6);
	assert_eq!(
		entries[0].additional_data.as_ref().expect("snapshot")["before"]["proposed_by"],
		USER_ID.to_string()
	);
	assert_eq!(
		entries[1].additional_data.as_ref().expect("snapshot")["before"]["text"],
		"Suggestion text"
	);
}

#[rocket::async_test]
async fn disabled_logging_and_unauthorized_mutations_preserve_existing_behavior() {
	let (client, config) = client(false).await;
	assert_eq!(
		post(
			&client,
			&config,
			USER_ID,
			"/api/binds/addBind",
			json!({"author":"Author","text":"Text"})
		)
		.await
		.status(),
		Status::Forbidden
	);
	assert_eq!(
		post(
			&client,
			&config,
			ADMIN_ID,
			"/api/binds/addBind",
			json!({"author":"Author","text":"Text"})
		)
		.await
		.status(),
		Status::Ok
	);
	assert!(
		client
			.rocket()
			.state::<AdminActionLog>()
			.expect("log")
			.history(15)
			.await
			.expect("history")
			.is_empty()
	);
}

#[rocket::async_test]
async fn admin_audit_failure_prevents_content_change() {
	let (client, config) = client(true).await;
	let directory = client.rocket().state::<tempfile::TempDir>().expect("directory");
	std::fs::write(directory.path().join("log_admin_actions.json"), "corrupt history").expect("corrupt log");
	let response = post(&client, &config, ADMIN_ID, "/api/binds/addBind", json!({"author":"Author","text":"Text"})).await;
	assert_eq!(response.status(), Status::ServiceUnavailable);
	assert!(client.rocket().state::<JsonDatabase>().expect("database").get_all_binds().expect("binds").is_empty());
}
