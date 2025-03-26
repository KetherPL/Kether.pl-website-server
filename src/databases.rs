// SPDX-License-Identifier: GPL-3.0-only
use crate::db::database::DbPool;
use crate::models::bind::{Bind, NewBind, BindVoting, NewBindVoting};
use crate::models::bind_suggestion::{BindSuggestion, NewBindSuggestion};
use crate::models::command::{Command, NewCommand};
use diesel::prelude::*;
use diesel::result::Error;

pub fn get_binds(pool: &DbPool) -> Result<Vec<Bind>, Error> {
    use crate::schema::binds::dsl::*;
    let mut conn = pool.get().unwrap();
    binds.load::<Bind>(&mut conn)
}

pub fn create_bind(pool: &DbPool, new_bind: NewBind) -> Result<Bind, Error> {
    use crate::schema::binds;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(binds::table)
        .values(&new_bind)
        .get_result(&mut conn)
}

pub fn get_bind_suggestions(pool: &DbPool) -> Result<Vec<BindSuggestion>, Error> {
    use crate::schema::bind_suggestions::dsl::*;
    let mut conn = pool.get().unwrap();
    bind_suggestions.load::<BindSuggestion>(&mut conn)
}

pub fn create_bind_suggestion(pool: &DbPool, new_bind_suggestion: NewBindSuggestion) -> Result<BindSuggestion, Error> {
    use crate::schema::bind_suggestions;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(bind_suggestions::table)
        .values(&new_bind_suggestion)
        .get_result(&mut conn)
}

pub fn get_commands(pool: &DbPool) -> Result<Vec<Command>, Error> {
    use crate::schema::commands::dsl::*;
    let mut conn = pool.get().unwrap();
    commands.load::<Command>(&mut conn)
}

pub fn create_command(pool: &DbPool, new_command: NewCommand) -> Result<Command, Error> {
    use crate::schema::commands;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(commands::table)
        .values(&new_command)
        .get_result(&mut conn)
}

pub fn get_bind_votings(pool: &DbPool) -> Result<Vec<BindVoting>, Error> {
    use crate::schema::bind_votings::dsl::*;
    let mut conn = pool.get().unwrap();
    bind_votings.load::<BindVoting>(&mut conn)
}

pub fn create_bind_voting(pool: &DbPool, new_bind_voting: NewBindVoting) -> Result<BindVoting, Error> {
    use crate::schema::bind_votings;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(bind_votings::table)
        .values(&new_bind_voting)
        .get_result(&mut conn)
}
