use edon::Nodejs;

fn main() {
    println!("Testing edon initialization...");
    
    // Set debug environment variables
    std::env::set_var("RUST_LOG", "debug");
    std::env::set_var("EDON_DEBUG", "1");
    
    let libnode_path = std::env::var("EDON_LIBNODE_PATH")
        .unwrap_or_else(|_| "/tmp/libnode-older/libnode.so".to_string());
    
    println!("Using EDON_LIBNODE_PATH: {}", libnode_path);
    
    match Nodejs::load_auto() {
        Ok(nodejs) => {
            println!("✓ Successfully initialized edon/Nodejs");
            
            // Try to spawn a context
            match nodejs.spawn_context() {
                Ok(context) => {
                    println!("✓ Successfully spawned Node.js context");
                    
                    // Try a simple eval
                    match context.eval("console.log('Hello from Node.js'); 'success'") {
                        Ok(result) => println!("✓ Eval successful: {:?}", result),
                        Err(e) => println!("✗ Eval failed: {:?}", e),
                    }
                }
                Err(e) => println!("✗ Failed to spawn context: {:?}", e),
            }
        }
        Err(e) => {
            println!("✗ Failed to initialize edon: {:?}", e);
            
            // Let's try to understand what went wrong
            println!("\nDebugging the failure...");
            
            // Check if the file exists and is readable
            if std::path::Path::new(&libnode_path).exists() {
                println!("✓ libnode.so file exists");
                
                // Check permissions
                match std::fs::metadata(&libnode_path) {
                    Ok(metadata) => {
                        println!("✓ File metadata accessible");
                        println!("  File size: {} bytes", metadata.len());
                        println!("  Permissions: {:?}", metadata.permissions());
                    }
                    Err(e) => println!("✗ Cannot read file metadata: {}", e),
                }
            } else {
                println!("✗ libnode.so file does not exist at: {}", libnode_path);
            }
        }
    }
}
