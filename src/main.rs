// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
#[cfg(feature = "server_query")]
use gamedig::games::l4d2::query;
use tokio::{task::{spawn, spawn_blocking}, time::{sleep, Duration}};

#[cfg(all(feature = "server_query", feature = "rest_api"))]
mod LiveServerInfo;

#[cfg(feature = "rest_api")]
mod REST;

#[cfg(feature = "cfg")]
mod config;

#[cfg(feature = "rest_sqlite")]
mod databases;
#[cfg(feature = "rest_sqlite")]
mod databases_rest;
#[cfg(feature = "rest_sqlite")]
mod db;
#[cfg(feature = "rest_sqlite")]
mod models;
#[cfg(feature = "rest_sqlite")]
mod schema;

#[cfg(feature = "rest_steam")]
mod steam_rest;

#[cfg(feature = "sat")]
mod sat_specific_rest;

#[cfg(feature = "rest_call_for_sub")]
mod call_for_sub_rest;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Start the RESTful server service
	#[cfg(feature = "rest_api")]
	#[arg(short, long)]
	service: bool,
	#[cfg(feature = "server_query")]
	#[command(subcommand)]
	command: Option<Commands>,
}

#[cfg(feature = "server_query")]
#[derive(Subcommand)]
enum Commands {
	/// -i <IP address> -p <PORT>   Prints selected server query to the CLI/terminal
	Query {
		#[arg(short, long, value_name = "L4D2 SERVER IP ADDRESS")]
		ip: String,
		#[arg(short, long, value_name = "PORT")]
		port: u16,
	}
}

#[tokio::main]
async fn main() {
	let args = Args::parse();

	#[cfg(feature = "rest_api")]
	if args.service {
		println!("Starting internal services server service");
		// :: Start the internal services server ::
		// Start the LiveServerInfo RESTful service
		spawn(async {
			let _ = spawn_blocking(move || REST::main()).await;
			std::process::exit(0);
		});
		// Keep the main thread alive so the server thread can run
		// Without this, the main thread exits immediately, killing the server thread.
		loop {
			sleep(Duration::from_secs(1)).await;
		}

		//... TODO
	}

	#[cfg(feature = "server_query")]
	match &args.command {
		Some(Commands::Query {ip, port}) => {
			match query_l4d2_server(&ip, *port).await {
                Ok(response) => {
                    println!("Query successful: {:?}", response);
                },
                Err(e) => {
                    eprintln!("Error querying server: {}", e);
                }
            }
		}
		None => {
			if !args.service {
				eprintln!("No command nor argument specified! \n Please use -h or --help for more information.");
			}
		}
	}
}

#[cfg(feature = "server_query")]
async fn query_l4d2_server(ip: &str, port: u16) -> Result<(), String> {
	// Get the IP address and port from command line arguments

	// Attempt to parse the IP address. If it fails, return an error.
    let parsed_ip = match ip.parse() {
        Ok(addr) => addr,
        Err(_) => return Err(format!("Invalid IP address: {}", ip)),
    };

    // Query the L4D2 server using the parsed IP and specified port.
    let response = query(&parsed_ip, Some(port));
	// None is the default port (which is 27015), could also be Some(27015)

    if let Err(error) = response {
        return Err(format!("Couldn't query, error: {}", error));
    }

    if let Ok(r) = response {
        println!("{:#?}", r);
    }

    Ok(())
}
