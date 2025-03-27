// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenv::dotenv;
use gamedig::games::l4d2;
use std::{fmt, path::PathBuf, thread};

mod databases;
mod databases_rest;
mod db;
mod LiveServerInfo;
mod models;
mod REST;
mod schema;

static DATABASE_PATH: &str = "kether.sqlite"; // Hardcoded in case if DB_PATH env var would be unavailable
static DATABASE_RELATIVE_DIR: bool = true; /* Is the database in the same dir as the executable?
                                        If yes, just put a file NAME in the DATABASE_PATH */

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
    dotenv().ok();
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

fn exe_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    match std::env::current_exe() {
        Ok(exe_path) => {
            if let Some(exe_dir) = exe_path.parent() {
                Ok(exe_dir.to_path_buf())
            } else {
                let err = format!("Could not determine the parent directory of the executable.");
                eprintln!("{} {}", "Error:".red(), err);
                return Err(Box::new(QuietErr(Some(err))));
            }
        }
        Err(e) => {
            let err = format!("Failed to get current executable path:\n {}", e);
            eprintln!("{} {}", "Error:".red(), err);
            return Err(Box::new(QuietErr(Some(err))));
        }
    }
}

#[derive(Debug)]
pub struct QuietErr(Option<String>);
impl fmt::Display for QuietErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref msg) = self.0 {
            write!(f, "{}", msg)
        } else {
            write!(f, "")
        }
    }
}
impl std::error::Error for QuietErr {}