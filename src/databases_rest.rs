// SPDX-License-Identifier: GPL-3.0-only

use crate::databases::{
    create_bind, create_bind_suggestion, create_command, get_binds, get_bind_suggestions,
    get_commands, create_bind_voting, get_bind_votings, delete_bind, delete_bind_suggestion,
    delete_command, delete_bind_voting, update_bind, update_bind_suggestion, update_command,
    update_bind_by_id, update_bind_suggestion_by_id, update_command_by_id
};
use crate::db::database::DbPool;
use crate::models::bind::{Bind, NewBind, BindVoting, NewBindVoting};
use crate::models::bind_suggestion::{BindSuggestion, NewBindSuggestion};
use crate::models::command::{Command, NewCommand};
use rocket::{get, post, delete, put, routes, serde::json::Json, State};
use rocket::http::Status;

// --- Binds ---

#[get("/binds/<bind_id>")]
pub fn get_bind_by_id(db_pool: &State<DbPool>, bind_id: i32) -> Result<Json<Bind>, Status> {
    crate::databases::get_bind(db_pool, bind_id)
        .map(Json)
        .map_err(|_| Status::NotFound) // Or InternalServerError if appropriate
}

#[get("/binds")]
pub fn get_all_binds(db_pool: &State<DbPool>) -> Result<Json<Vec<Bind>>, Status> {
    get_binds(db_pool)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[post("/binds", data = "<new_bind>")]
pub fn create_new_bind(
    db_pool: &State<DbPool>,
    new_bind: Json<NewBind>,
) -> Result<Json<Bind>, Status> {
    create_bind(db_pool, new_bind.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[delete("/binds/<bind_id>")]
pub fn delete_existing_bind(
    db_pool: &State<DbPool>,
    bind_id: i32,
) -> Result<Json<usize>, Status> {
    delete_bind(db_pool, bind_id)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[put("/binds", data = "<bind_to_update>")]
pub fn update_existing_bind(
    db_pool: &State<DbPool>,
    bind_to_update: Json<Bind>,
) -> Result<Json<Bind>, Status> {
    update_bind(db_pool, bind_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

// --- Bind Suggestions ---

#[get("/bind_suggestions/<bind_suggestion_id>")]
pub fn get_bind_suggestion_by_id(db_pool: &State<DbPool>, bind_suggestion_id: i32) -> Result<Json<BindSuggestion>, Status> {
    crate::databases::get_bind_suggestion(db_pool, bind_suggestion_id)
        .map(Json)
        .map_err(|_| Status::NotFound)
}

#[get("/bind_suggestions")]
pub fn get_all_bind_suggestions(
    db_pool: &State<DbPool>,
) -> Result<Json<Vec<BindSuggestion>>, Status> {
    get_bind_suggestions(db_pool)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[post("/bind_suggestions", data = "<new_bind_suggestion>")]
pub fn create_new_bind_suggestion(
    db_pool: &State<DbPool>,
    new_bind_suggestion: Json<NewBindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    create_bind_suggestion(db_pool, new_bind_suggestion.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[delete("/bind_suggestions/<bind_suggestion_id>")]
pub fn delete_existing_bind_suggestion(
    db_pool: &State<DbPool>,
    bind_suggestion_id: i32,
) -> Result<Json<usize>, Status> {
    delete_bind_suggestion(db_pool, bind_suggestion_id)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[put("/bind_suggestions", data = "<bind_suggestion_to_update>")]
pub fn update_existing_bind_suggestion(
    db_pool: &State<DbPool>,
    bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    update_bind_suggestion(db_pool, bind_suggestion_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

// --- Commands ---

#[get("/commands/<command_id>")]
pub fn get_command_by_id(db_pool: &State<DbPool>, command_id: i32) -> Result<Json<Command>, Status> {
    crate::databases::get_command(db_pool, command_id)
        .map(Json)
        .map_err(|_| Status::NotFound)
}

#[get("/commands")]
pub fn get_all_commands(db_pool: &State<DbPool>) -> Result<Json<Vec<Command>>, Status> {
    get_commands(db_pool)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[post("/commands", data = "<new_command>")]
pub fn create_new_command(
    db_pool: &State<DbPool>,
    new_command: Json<NewCommand>,
) -> Result<Json<Command>, Status> {
    create_command(db_pool, new_command.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[delete("/commands/<command_id>")]
pub fn delete_existing_command(
    db_pool: &State<DbPool>,
    command_id: i32,
) -> Result<Json<usize>, Status> {
    delete_command(db_pool, command_id)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[put("/commands", data = "<command_to_update>")]
pub fn update_existing_command(
    db_pool: &State<DbPool>,
    command_to_update: Json<Command>,
) -> Result<Json<Command>, Status> {
    update_command(db_pool, command_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

// --- Bind Votings ---

#[get("/bind_votings/<bind_voting_id>")]
pub fn get_bind_voting_by_id(db_pool: &State<DbPool>, bind_voting_id: i32) -> Result<Json<BindVoting>, Status> {
    crate::databases::get_bind_voting(db_pool, bind_voting_id)
        .map(Json)
        .map_err(|_| Status::NotFound)
}

#[get("/bind_votings")]
pub fn get_all_bind_votings(db_pool: &State<DbPool>) -> Result<Json<Vec<BindVoting>>, Status> {
    get_bind_votings(db_pool)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[post("/bind_votings", data = "<new_bind_voting>")]
pub fn create_new_bind_voting(
    db_pool: &State<DbPool>,
    new_bind_voting: Json<NewBindVoting>,
) -> Result<Json<BindVoting>, Status> {
    create_bind_voting(db_pool, new_bind_voting.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[delete("/bind_votings/<bind_voting_id>")]
pub fn delete_existing_bind_voting(
    db_pool: &State<DbPool>,
    bind_voting_id: i32,
) -> Result<Json<usize>, Status> {
    delete_bind_voting(db_pool, bind_voting_id)
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

//By ID
#[put("/binds/<bind_id>", data = "<bind_to_update>")]
pub fn update_existing_bind_by_id(
    db_pool: &State<DbPool>,
    bind_id: i32,
    bind_to_update: Json<Bind>,
) -> Result<Json<Bind>, Status> {
    update_bind_by_id(db_pool, bind_id, bind_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[put("/bind_suggestions/<bind_suggestion_id>", data = "<bind_suggestion_to_update>")]
pub fn update_existing_bind_suggestion_by_id(
    db_pool: &State<DbPool>,
    bind_suggestion_id: i32,
    bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    update_bind_suggestion_by_id(db_pool, bind_suggestion_id, bind_suggestion_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

#[put("/commands/<command_id>", data = "<command_to_update>")]
pub fn update_existing_command_by_id(
    db_pool: &State<DbPool>,
    command_id: i32,
    command_to_update: Json<Command>,
) -> Result<Json<Command>, Status> {
    update_command_by_id(db_pool, command_id, command_to_update.into_inner())
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}

// --- Mount the routes in REST.rs ---
pub fn mount_database_routes() -> Vec<rocket::Route> {
    routes![
        get_bind_by_id,
        get_bind_suggestion_by_id,
        get_command_by_id,
        get_bind_voting_by_id,
        get_all_binds,
        create_new_bind,
        delete_existing_bind,
        update_existing_bind,
        get_all_bind_suggestions,
        create_new_bind_suggestion,
        delete_existing_bind_suggestion,
        update_existing_bind_suggestion,
        get_all_commands,
        create_new_command,
        delete_existing_command,
        update_existing_command,
        get_all_bind_votings,
        create_new_bind_voting,
        delete_existing_bind_voting,
        update_existing_bind_by_id,
        update_existing_bind_suggestion_by_id,
        update_existing_command_by_id,
    ]
}
