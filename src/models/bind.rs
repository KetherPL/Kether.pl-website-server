use diesel::prelude::*;
use rocket::serde::{Serialize, Deserialize};

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::binds)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bind {
    pub id: i32,
    pub author: String,
    pub text: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::binds)]
pub struct NewBind<'a> {
    pub author: &'a str,
    pub text: &'a str,
}

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::bind_votings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BindVoting {
    pub id: i32,
    pub voter_steam_id: String,
    pub voted_bind_id: String,
    pub vote: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = crate::schema::bind_votings)]
pub struct NewBindVoting<'a> {
    pub voter_steam_id: &'a str,
    pub voted_bind_id: &'a str,
    pub vote: &'a str,
}
