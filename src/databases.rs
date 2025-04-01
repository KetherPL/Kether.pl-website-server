// SPDX-License-Identifier: GPL-3.0-only
use crate::db::database::DbPool;
use crate::models::bind::{Bind, NewBind, BindVoting, NewBindVoting};
use crate::models::bind_suggestion::{BindSuggestion, NewBindSuggestion};
use crate::models::command::{Command, NewCommand};
use diesel::prelude::*;
use diesel::result::Error;
use diesel::sql_types::BigInt;

pub fn get_bind(pool: &DbPool, bind_id: i32) -> Result<Bind, Error> {
	use crate::schema::binds::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	binds.filter(id.eq(bind_id)).first(&mut conn)
}

pub fn get_binds(pool: &DbPool) -> Result<Vec<Bind>, Error> {
	use crate::schema::binds::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	binds.load::<Bind>(&mut conn)
}

pub fn create_bind(pool: &DbPool, new_bind: NewBind) -> Result<Bind, Error> {
	use crate::schema::binds::{self, dsl::*};
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::insert_into(binds::table)
		.values(&new_bind)
		.execute(&mut conn)?;
		
    // Get the last inserted ID using raw SQL
    let last_id: i64 = diesel::select(diesel::dsl::sql::<BigInt>("last_insert_rowid()"))
        .get_result(&mut conn)?;

    // Retrieve the inserted bind
    binds.filter(id.eq(last_id as i32)).first(&mut conn)
}

pub fn delete_bind(pool: &DbPool, bind_id: i32) -> Result<usize, Error> {
	use crate::schema::binds::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::delete(binds.filter(id.eq(bind_id)))
		.execute(&mut conn)
}

pub fn update_bind(pool: &DbPool, bind_to_update: Bind) -> Result<Bind, Error> {
	use crate::schema::binds::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(binds.filter(id.eq(bind_to_update.id)))
		.set((author.eq(bind_to_update.author), text.eq(bind_to_update.text)))
		.execute(&mut conn)?;

		// Retrieve the updated bind
		binds.filter(id.eq(bind_to_update.id)).first(&mut conn)
}

pub fn get_bind_suggestion(pool: &DbPool, bind_suggestion_id: i32) -> Result<BindSuggestion, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	bind_suggestions.filter(id.eq(bind_suggestion_id)).first(&mut conn)
}

pub fn get_bind_suggestions(pool: &DbPool) -> Result<Vec<BindSuggestion>, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	bind_suggestions.load::<BindSuggestion>(&mut conn)
}

pub fn create_bind_suggestion(pool: &DbPool, new_bind_suggestion: NewBindSuggestion) -> Result<BindSuggestion, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::insert_into(bind_suggestions)
		.values(&new_bind_suggestion)
		.execute(&mut conn)?;

    let last_id: i64 = diesel::select(diesel::dsl::sql::<BigInt>("last_insert_rowid()"))
        .get_result(&mut conn)?;
    bind_suggestions.filter(id.eq(last_id as i32)).first(&mut conn)
}

pub fn delete_bind_suggestion(pool: &DbPool, bind_suggestion_id: i32) -> Result<usize, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::delete(bind_suggestions.filter(id.eq(bind_suggestion_id)))
		.execute(&mut conn)
}

pub fn update_bind_suggestion(pool: &DbPool, bind_suggestion_to_update: BindSuggestion) -> Result<BindSuggestion, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(bind_suggestions.filter(id.eq(bind_suggestion_to_update.id)))
		.set((author.eq(bind_suggestion_to_update.author), text.eq(bind_suggestion_to_update.text), proposed_by.eq(bind_suggestion_to_update.proposed_by)))
		.execute(&mut conn)?;

    bind_suggestions.filter(id.eq(bind_suggestion_to_update.id)).first(&mut conn)
}

pub fn get_command(pool: &DbPool, command_id: i32) -> Result<Command, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	commands.filter(id.eq(command_id)).first(&mut conn)
}

pub fn get_commands(pool: &DbPool) -> Result<Vec<Command>, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	commands.load::<Command>(&mut conn)
}

pub fn create_command(pool: &DbPool, new_command: NewCommand) -> Result<Command, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::insert_into(commands)
		.values(&new_command)
		.execute(&mut conn)?;

    let last_id: i64 = diesel::select(diesel::dsl::sql::<BigInt>("last_insert_rowid()"))
        .get_result(&mut conn)?;
    commands.filter(id.eq(last_id as i32)).first(&mut conn)
}

pub fn delete_command(pool: &DbPool, command_id: i32) -> Result<usize, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::delete(commands.filter(id.eq(command_id)))
		.execute(&mut conn)
		
	
}

pub fn update_command(pool: &DbPool, command_to_update: Command) -> Result<Command, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(commands.filter(id.eq(command_to_update.id)))
		.set((command.eq(command_to_update.command), description.eq(command_to_update.description)))
		.execute(&mut conn)?;

    commands.filter(id.eq(command_to_update.id)).first(&mut conn)
}

pub fn get_bind_voting(pool: &DbPool, bind_voting_id: i32) -> Result<BindVoting, Error> {
	use crate::schema::bind_votings::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	bind_votings.filter(id.eq(bind_voting_id)).first(&mut conn)
}

pub fn get_bind_votings(pool: &DbPool) -> Result<Vec<BindVoting>, Error> {
	use crate::schema::bind_votings::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	bind_votings.load::<BindVoting>(&mut conn)
}

pub fn create_bind_voting(pool: &DbPool, new_bind_voting: NewBindVoting) -> Result<BindVoting, Error> {
	use crate::schema::bind_votings::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::insert_into(bind_votings)
		.values(&new_bind_voting)
		.execute(&mut conn)?;

    let last_id: i64 = diesel::select(diesel::dsl::sql::<BigInt>("last_insert_rowid()"))
        .get_result(&mut conn)?;
    bind_votings.filter(id.eq(last_id as i32)).first(&mut conn)
}

pub fn delete_bind_voting(pool: &DbPool, bind_voting_id: i32) -> Result<usize, Error> {
	use crate::schema::bind_votings::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::delete(bind_votings.filter(id.eq(bind_voting_id)))
		.execute(&mut conn)
}

// By ID
pub fn update_bind_by_id(pool: &DbPool, bind_id: i32, bind_to_update: Bind) -> Result<Bind, Error> {
	use crate::schema::binds::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(binds.filter(id.eq(bind_id)))
		.set((author.eq(bind_to_update.author), text.eq(bind_to_update.text)))
		.execute(&mut conn)?;

    binds.filter(id.eq(bind_id)).first(&mut conn)
}

pub fn update_bind_suggestion_by_id(pool: &DbPool, bind_suggestion_id: i32, bind_suggestion_to_update: BindSuggestion) -> Result<BindSuggestion, Error> {
	use crate::schema::bind_suggestions::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(bind_suggestions.filter(id.eq(bind_suggestion_id)))
		.set((author.eq(bind_suggestion_to_update.author), text.eq(bind_suggestion_to_update.text), proposed_by.eq(bind_suggestion_to_update.proposed_by)))
		.execute(&mut conn)?;

    bind_suggestions.filter(id.eq(bind_suggestion_id)).first(&mut conn)
}

pub fn update_command_by_id(pool: &DbPool, command_id: i32, command_to_update: Command) -> Result<Command, Error> {
	use crate::schema::commands::dsl::*;
	let mut conn = match pool.get() {
	Ok(conn) => conn,
	Err(e) => {
		eprintln!("Error getting database connection: {}", e);
		return Err(Error::DatabaseError(diesel::result::DatabaseErrorKind::UnableToSendCommand, Box::new(e.to_string()))); // Or a more specific error
	}
};
	diesel::update(commands.filter(id.eq(command_id)))
		.set((command.eq(command_to_update.command), description.eq(command_to_update.description)))
		.execute(&mut conn)?;

    commands.filter(id.eq(command_id)).first(&mut conn)
}