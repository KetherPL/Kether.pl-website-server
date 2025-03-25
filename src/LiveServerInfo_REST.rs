// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use gamedig::{games::l4d2, protocols::valve::game::Player};
use rocket::{get, launch, routes, serde::{Deserialize, Serialize, json::Json}};

// Define a struct to represent the L4D2 server response
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct L4D2ServerInfo {
    name: String,
    map: String,
    players: u8, //num of players already on the server
    maxplayers: u8,
    bots: u8,
    playerdetails: Vec<PD>, //players list + score + duration
    // Add other fields as needed based on the gamedig response
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct PD { // from gamedig::protocols::valve::game::Player
    pub name: String,
    pub score: i32,
    pub duration: f32,
}

impl From<Player> for PD {
    fn from(player: Player) -> Self {
        PD {
            name: player.name,
            score: player.score,
            duration: player.duration,
        }
    }
}

#[get("/<ip>/<port>")]
fn live_server_info(ip: String, port: u16) -> Result<Json<L4D2ServerInfo>, String> {
    // Get the IP address and port from the URL parameters

    let response = l4d2::query(&ip.parse().unwrap(), Some(port));

    match response { // Result type, must check what it is...
        Err(error) => Err(format!("Couldn't query, error: {}", error)),
        Ok(r) => {
            // Extract the relevant information from the gamedig response
            let server_info = L4D2ServerInfo {
                name: r.name,
                map: r.map,
                players: r.players_online,
                maxplayers: r.players_maximum,
                bots: r.players_bots,
                playerdetails: r.players_details.into_iter().map(PD::from).collect(),
            };
            Ok(Json(server_info))
            // format!("{:?}", r)
        },
    }
}


#[get("/kether")]
fn live_server_info_kether() -> Result<Json<L4D2ServerInfo>, String> {
    /* Hardcoded for simplicity, in a real-world application, 
      this should be fetched from a configuration file or a database.
      LiveServer-hosted Kether.pl L4D2 server as of 2025.
    */
    let response = l4d2::query(&"51.83.217.86".parse().unwrap(), Some(29800));

    match response { // Result type, must check what it is...
        Err(error) => Err(format!("Couldn't query, error: {}", error)),
        Ok(r) => {
            // Extract the relevant information from the gamedig response
            let server_info = L4D2ServerInfo {
                name: r.name,
                map: r.map,
                players: r.players_online,
                maxplayers: r.players_maximum,
                bots: r.players_bots,
                playerdetails: r.players_details.into_iter().map(PD::from).collect(),
            };
            Ok(Json(server_info))
        },
    }
}

// Direct JSON from LiveServer API
// #[get("/lvs")]
// fn live_server_info_kether() -> String {
//     /* Hardcoded for simplicity, in a real-world application, 
//       this should be fetched from a configuration file or a database.
//       LiveServer-hosted Kether.pl L4D2 server as of 2025.
//     */
//     let response = l4d2::query(&"51.83.217.86".parse().unwrap(), Some(29800));

//     match response { // Result type, must check what it is...
//         Err(error) => format!("Couldn't query, error: {}", error),
//         Ok(r) => {
//             // Extract the relevant information from the gamedig response
//             let server_info = L4D2ServerInfo {
//                 name: r.name,
//                 map: r.map,
//                 players: r.players.len() as u8,
//                 max_players: r.max_players,
//                 bots: r.bots,
//             };
//             Ok(Json(server_info))
//         },
//     }
// }

#[launch]
pub fn rocket() -> _ {
    rocket::build()
    .configure(rocket::Config::figment().merge(("port", 3001))) // NodeJS / React port
    .mount("/api/LiveServerInfo", routes![live_server_info, live_server_info_kether])
}