// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
use gamedig::games::l4d2;
use std::thread;

mod LiveServerInfo;
mod REST;
mod config;
mod databases;
mod databases_rest;
mod db;
mod models;
mod schema;
mod steam_rest;
mod sat_specific_rest;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Start the RESTful server service
	#[arg(short, long)]
	service: bool,
	#[command(subcommand)]
	command: Option<Commands>,
}

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

fn main() {
	let args = Args::parse();

	if args.service {
		println!("Starting internal services server service");
		// :: Start the internal services server ::
		// Start the LiveServerInfo RESTful service
		thread::spawn(move || {
			REST::main();
			std::process::exit(0);
		});
		// Keep the main thread alive so the server thread can run
		// Without this, the main thread exits immediately, killing the server thread.
		loop {
			thread::sleep(std::time::Duration::from_secs(1));
		}

		//... TODO
	}

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

fn query_l4d2_server(ip: &str, port: u16) {

	// Get the IP address and port from command line arguments

	let response = l4d2::query(&ip.parse().unwrap(), Some(port));
	// None is the default port (which is 27015), could also be Some(27015)

	match response { // Result type, must check what it is...
		Err(error) => println!("Couldn't query, error: {}", error),
		Ok(r) => println!("{:#?}", r)
	}
}
