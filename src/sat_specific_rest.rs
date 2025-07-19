// SPDX-License-Identifier: GPL-3.0-only

use rocket::response::Redirect;
use rocket::{get, routes};

// --- Redirect the root path to our frontend
#[get("/")]
pub fn redirect_to_kether() -> Redirect {
    Redirect::to("https://kether.pl")
}

pub fn mount_sat_specific_routes() -> Vec<rocket::Route> {
    routes![
		redirect_to_kether,
    ]
}
