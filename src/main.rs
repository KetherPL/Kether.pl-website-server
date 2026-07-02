// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
use tokio::{task::spawn, time::{sleep, Duration}};
#[cfg(feature = "rest_api")]
use tokio::sync::{broadcast, mpsc};
#[cfg(feature = "rest_api")]
use tokio::task::JoinHandle;
#[cfg(feature = "server_query")]
use gamedig::games::l4d2::query;

#[cfg(any(feature = "server_query", feature = "rest_steam"))]
mod utils;

#[cfg(all(feature = "server_query", feature = "rest_api"))]
mod LiveServerInfo;

#[cfg(feature = "rest_api")]
mod REST;

#[cfg(feature = "cfg")]
mod config;

#[cfg(feature = "hot_reload")]
mod config_watch;

#[cfg(feature = "rest_steam")]
mod steam_rest;

#[cfg(feature = "sat")]
mod sat_specific_rest;

#[cfg(feature = "fastdl")]
mod fastdl_rest;

#[cfg(feature = "rest_call_for_sub")]
mod call_for_sub_rest;
#[cfg(feature = "rest_call_for_sub")]
mod steam_bot;

#[cfg(feature = "rest_json_db")]
mod json_cmds_binds_rest;
#[cfg(feature = "rest_json_db")]
mod json_api;
#[cfg(feature = "rest_json_db")]
mod json_storage;

#[cfg(feature = "rest_api")]
mod plan_ws;

#[cfg(feature = "auth")]
mod auth;

#[cfg(feature = "maps_bridge")]
mod maps_bridge;

mod repl;

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

/// Waits for service tasks to finish after a shutdown signal, with a timeout.
#[cfg(feature = "rest_api")]
async fn await_service_shutdown(
	rest_handle: JoinHandle<()>,
	#[cfg(feature = "rest_call_for_sub")] bot_handle: Option<JoinHandle<()>>,
	#[cfg(not(feature = "rest_call_for_sub"))] _bot_handle: Option<JoinHandle<()>>,
) {
	let _ = tokio::time::timeout(Duration::from_secs(30), async {
		if let Err(e) = rest_handle.await {
			eprintln!("REST task join error: {:?}", e);
		}
		#[cfg(feature = "rest_call_for_sub")]
		if let Some(bot) = bot_handle {
			if let Err(e) = bot.await {
				eprintln!("SteamBot task join error: {:?}", e);
			}
		}
	})
	.await;
	sleep(Duration::from_millis(500)).await;
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
		let (daemon_command_tx, mut daemon_command_rx) =
			mpsc::unbounded_channel::<repl::DaemonCommand>();
		#[cfg(feature = "hot_reload")]
		let _config_watcher = {
			let handle = steam_bot::registry::config_handle();
			let config_path = match config::exe_dir() {
				Ok(dir) => dir.join(config::CONF_FILE_NAME),
				Err(e) => {
					eprintln!("Failed to build config watcher path: {}", e);
					return;
				}
			};
			match config_watch::spawn_config_watcher(handle, config_path) {
				Ok(watcher) => watcher,
				Err(e) => {
					eprintln!("Failed to start config watcher: {}", e);
					return;
				}
			}
		};
		// Start the REPL key listener (activates on 'C' key press)
		let daemon_command_tx_for_repl = daemon_command_tx.clone();
		spawn(async {
			if let Err(e) = repl::start_key_listener(daemon_command_tx_for_repl).await {
				eprintln!("REPL key listener error: {}", e);
			}
		});

		let mut running = true;
		while running {
			let (service_shutdown_tx, _) = broadcast::channel::<()>(1);
			let rest_shutdown = service_shutdown_tx.subscribe();
			#[cfg(feature = "rest_call_for_sub")]
			let bot_shutdown = service_shutdown_tx.subscribe();

			#[cfg(feature = "rest_api")]
			let rest_handle = spawn(async move {
				if let Err(e) = REST::run(rest_shutdown).await {
					eprintln!("REST server error: {}", e);
				}
			});

			#[cfg(feature = "rest_call_for_sub")]
			let bot_handle: JoinHandle<()> = spawn(async move {
				if let Err(e) = steam_bot::run(bot_shutdown).await {
					eprintln!("SteamBot error: {}", e);
				}
			});

			loop {
				tokio::select! {
					_ = sleep(Duration::from_secs(1)) => {}
					Some(cmd) = daemon_command_rx.recv() => {
						match cmd {
							repl::DaemonCommand::Restart => {
								println!("Restart requested via REPL — restarting services in-process...");
								let _ = service_shutdown_tx.send(());
								await_service_shutdown(
									rest_handle,
									{
										#[cfg(feature = "rest_call_for_sub")]
										{
											Some(bot_handle)
										}
										#[cfg(not(feature = "rest_call_for_sub"))]
										{
											None
										}
									},
								)
								.await;
								println!("Services stopped. Starting fresh...");
								break;
							}
							repl::DaemonCommand::Stop => {
								println!("Stop requested via REPL. Shutting down daemon.");
								let _ = service_shutdown_tx.send(());
								await_service_shutdown(
									rest_handle,
									{
										#[cfg(feature = "rest_call_for_sub")]
										{
											Some(bot_handle)
										}
										#[cfg(not(feature = "rest_call_for_sub"))]
										{
											None
										}
									},
								)
								.await;
								running = false;
								break;
							}
						}
					}
				}
				if !running {
					break;
				}
			}
		}

		println!("Daemon exited.");
		std::process::exit(0);
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
