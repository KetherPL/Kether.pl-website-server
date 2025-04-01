use diesel::prelude::*;
use rocket::serde::{Serialize, Deserialize};

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::bind_suggestions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BindSuggestion {
	pub id: i32,
	pub author: String,
	pub text: String,
	pub proposed_by: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::bind_suggestions)]
pub struct NewBindSuggestion<'a> {
	pub author: &'a str,
	pub text: &'a str,
	pub proposed_by: &'a str,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::bind_suggestions)]
pub struct DelBindSuggestion {
	pub id: i32,
}