// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
use tokio::{task::{spawn, spawn_blocking}, time::{sleep, Duration}};
#[cfg(feature = "server_query")]
use gamedig::games::l4d2::query;

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

#[cfg(feature = "fastdl")]
mod fastdl_rest;

#[cfg(feature = "rest_call_for_sub")]
mod call_for_sub_rest;
#[cfg(feature = "rest_call_for_sub")]
mod SteamBot;

#[cfg(feature = "rest_json_db")]
mod json_cmds_binds_rest;
#[cfg(feature = "rest_json_db")]
mod json_storage;

/// Command line arguments parser for the Kether Internal Services Server
/// 
/// This struct defines the available command line options and subcommands
/// for the application. It uses clap for argument parsing and validation.
/// 
/// # Features
/// * `rest_api` - Enables the `--service` flag to start the REST API server
/// * `server_query` - Enables the `Query` subcommand for L4D2 server queries
/// 
/// # Example
/// ```bash
/// # Start the REST API service
/// ./Kether_Internal_Services_Server --service
/// 
/// # Query an L4D2 server
/// ./Kether_Internal_Services_Server query -i 192.168.1.100 -p 27015
/// ```
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Start the RESTful server service
	/// 
	/// When this flag is provided, the application starts the REST API server
	/// on port 3001 and initializes all configured services including:
	/// * LiveServerInfo RESTful service
	/// * SteamBot for Steam chat integration
	/// * Database REST endpoints (if enabled)
	/// * Steam REST endpoints (if enabled)
	#[cfg(feature = "rest_api")]
	#[arg(short, long)]
	service: bool,
	
	/// Server query subcommands
	/// 
	/// Available subcommands for querying L4D2 servers and other game servers.
	/// Only available when the `server_query` feature is enabled.
	#[cfg(feature = "server_query")]
	#[command(subcommand)]
	command: Option<Commands>,
}

/// Available subcommands for server querying functionality
/// 
/// This enum defines the available subcommands that can be used with the
/// main application. Currently supports L4D2 server queries.
#[cfg(feature = "server_query")]
#[derive(Subcommand)]
enum Commands {
	/// Query an L4D2 server for information
	/// 
	/// This command queries a Left 4 Dead 2 server at the specified IP address
	/// and port to retrieve server information such as player count, map name,
	/// server name, and other game-specific data.
	/// 
	/// # Arguments
	/// * `ip` - The IP address of the L4D2 server to query
	/// * `port` - The port number of the L4D2 server (default: 27015)
	/// 
	/// # Example
	/// ```bash
	/// ./Kether_Internal_Services_Server query -i 192.168.1.100 -p 27015
	/// ```
	Query {
		/// IP address of the L4D2 server to query
		#[arg(short, long, value_name = "L4D2 SERVER IP ADDRESS")]
		ip: String,
		/// Port number of the L4D2 server
		#[arg(short, long, value_name = "PORT")]
		port: u16,
	}
}

/// Main entry point for the Kether Internal Services Server
/// 
/// This function serves as the primary entry point for the application.
/// It parses command line arguments and routes to the appropriate functionality
/// based on the provided arguments and enabled features.
/// 
/// # Features
/// * `rest_api` - Starts the REST API server when `--service` flag is used
/// * `server_query` - Enables L4D2 server querying functionality
/// 
/// # Behavior
/// 1. Parses command line arguments using clap
/// 2. If `--service` flag is provided, starts the REST API server
/// 3. If `query` subcommand is provided, queries the specified L4D2 server
/// 4. If no valid arguments are provided, displays help information
/// 
/// # Example
/// ```rust
/// #[tokio::main]
/// async fn main() {
///     // This function is automatically called by tokio::main
/// }
/// ```
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
		// Start the SteamBot
		spawn(async {
			if let Err(e) = SteamBot::main().await {
				eprintln!("SteamBot error: {}", e);
			}
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

/// Queries a Left 4 Dead 2 server for information
/// 
/// This function queries an L4D2 server at the specified IP address and port
/// to retrieve server information such as player count, map name, server name,
/// and other game-specific data.
/// 
/// # Arguments
/// * `ip` - The IP address of the L4D2 server as a string
/// * `port` - The port number of the L4D2 server
/// 
/// # Returns
/// * `Ok(())` - If the query is successful
/// * `Err(String)` - If the query fails (invalid IP, network error, etc.)
/// 
/// # Example
/// ```rust
/// let result = query_l4d2_server("192.168.1.100", 27015).await;
/// match result {
///     Ok(_) => println!("Query successful"),
///     Err(e) => eprintln!("Query failed: {}", e),
/// }
/// ```
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
