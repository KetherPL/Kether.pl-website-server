// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
#[cfg(feature = "server_query")]
use gamedig::games::l4d2;
#[cfg(feature = "rest_call_for_sub")]
use std::sync::Arc;
#[cfg(feature = "rest_call_for_sub")]
use edon::Nodejs;

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
		
		// Initialize Node.js instance and Steam bot
		#[cfg(feature = "rest_call_for_sub")]
		{
			let nodejs_instance = match node().await {
				Ok(node) => node,
				Err(e) => {
					eprintln!("Failed to initialize Node.js and Steam bot: {}", e);
					std::process::exit(1);
				}
			};
			
			// Start REST server with Node.js instance - don't spawn thread, run directly
			if let Err(e) = REST::main_with_nodejs(nodejs_instance).await {
				eprintln!("REST server failed: {}", e);
				std::process::exit(1);
			}
		}
		
		#[cfg(not(feature = "rest_call_for_sub"))]
		{
			if let Err(e) = REST::main().await {
				eprintln!("REST server failed: {}", e);
				std::process::exit(1);
			}
		}
	}

	#[cfg(feature = "server_query")]
	match &args.command {
		Some(Commands::Query { ip, port }) => {
			query_l4d2_server(&ip, *port);
		}
		None => {
			if !args.service {
				eprintln!("No command nor argument specified!");
			}
		}
	}
}

#[cfg(feature = "server_query")]
fn query_l4d2_server(ip: &str, port: u16) {

	// Get the IP address and port from command line arguments

	let response = l4d2::query(&ip.parse().unwrap(), Some(port));
	// None is the default port (which is 27015), could also be Some(27015)

	match response { // Result type, must check what it is...
		Err(error) => println!("Couldn't query, error: {}", error),
		Ok(r) => println!("{:#?}", r)
	}
}


#[cfg(feature = "rest_call_for_sub")]
pub async fn node() -> Result<Arc<Nodejs>, Box<dyn std::error::Error>> {
    // Create Node.js instance
	let node: Nodejs;
	
	// let node_sys: std::path::PathBuf = "/lib/libnode.so".into();
	// if node_sys.exists() {
	// 	node = Nodejs::load("/lib/libnode.so")?;
	// }
	// else {
   		node = Nodejs::load_auto()?;
	// }
	// Start a Nodejs context
	let node_arc = Arc::new(node);
	// Initialize Steam bot
	let node_context = node_arc.spawn_context()?;
	
	// Spawn thread to initialize Steam bot
	println!("Initializing Steam bot...");
	
	// Initialize bot in Node.js with proper error handling
	let init_result = node_context.eval(r#"
		(async () => {
			try {
				// Import the steamBot - adjust path as needed
				const { steamBot } = require('./nodejs/dist/src/steam/steamBot');
				global.steamBot = steamBot;
				
				console.log('Logging into Steam...');
				await steamBot.loginAsync();
				console.log('Steam bot logged in successfully');
				
				// Set up global sendMessage function
				global.sendMessage = async (msg) => {
					console.log('Sending message:', msg);
					return await steamBot.sendMessageAsync(msg);
				};
				
				return 'Steam bot initialized successfully';
			} catch (error) {
				console.error('Steam bot initialization failed:', error);
				throw error;
			}
		})();
	"#);
	
	match init_result {
        Ok(_) => {
            println!("✓ Steam bot initialization completed successfully");
            Ok(node_arc)
        }
        Err(e) => {
            eprintln!("✗ Failed to initialize Steam bot: {}", e);
            Err(e.into())
        }
    }
}