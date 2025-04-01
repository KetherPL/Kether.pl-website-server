use diesel::prelude::*;
use rocket::serde::{Serialize, Deserialize};

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::commands)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Command {
    pub id: i32,
    pub command: String,
    pub description: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::commands)]
pub struct NewCommand<'a> {
    pub command: &'a str,
    pub description: &'a str,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::commands)]
pub struct DelCommand {
    pub id: i32,
}