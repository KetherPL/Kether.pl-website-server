// SPDX-License-Identifier: GPL-3.0-only

use rocket::response::Redirect;
use rocket::{catch, Catcher, catchers, get, routes};

// --- Redirect the root path to our frontend
#[get("/")]
pub async fn redirect_to_kether() -> Redirect {
    Redirect::to("https://kether.pl")
}

#[catch(404)]
pub async fn not_found() -> Redirect {
    Redirect::to("https://kether.pl")
}

pub fn mount_sat_specific_routes() -> Vec<rocket::Route> {
    routes![
		redirect_to_kether,
    ]
}

pub fn mount_sat_specific_catchers() -> Vec<Catcher> {
    catchers![
        not_found,
    ]
}