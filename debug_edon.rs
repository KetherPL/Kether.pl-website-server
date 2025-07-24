use std::ffi::CString;

fn main() {
    let libnode_path = std::env::var("EDON_LIBNODE_PATH")
        .unwrap_or_else(|_| "/tmp/libnode-older/libnode.so".to_string());
    
    println!("Loading libnode from: {}", libnode_path);
    
    // Try to load the library exactly like libnode_sys does
    let lib = match unsafe { libloading::os::unix::Library::new(&libnode_path) } {
        Ok(lib) => {
            println!("✓ Successfully loaded libnode.so");
            lib
        }
        Err(e) => {
            eprintln!("✗ Failed to load libnode.so: {}", e);
            return;
        }
    };
    
    // Test all the symbols that edon/libnode_sys might need
    let critical_symbols = [
        "node_embedding_start",
        "node_embedding_stop",
        "node_embedding_init", 
        "node_embedding_teardown",
        "node_api_create_syntax_error",
        "napi_create_string_utf8",
        "napi_get_global",
        "napi_create_object",
        "napi_set_property",
        "napi_get_property",
        "napi_call_function",
        "uv_default_loop",
        "uv_run",
        "uv_loop_close",
        "uv_loop_init",
        "uv_loop_configure",
    ];
    
    println!("\nTesting critical symbols:");
    let mut missing_symbols = Vec::new();
    
    for symbol_name in &critical_symbols {
        // Test exactly like libnode_sys does it
        match unsafe { lib.get::<*const ()>(symbol_name.as_bytes()) } {
            Ok(_) => println!("✓ Found symbol: {}", symbol_name),
            Err(e) => {
                println!("✗ Missing symbol: {} ({})", symbol_name, e);
                missing_symbols.push(*symbol_name);
            }
        }
    }
    
    if missing_symbols.is_empty() {
        println!("\n✓ All critical symbols found!");
    } else {
        println!("\n✗ Missing {} critical symbols:", missing_symbols.len());
        for symbol in &missing_symbols {
            println!("  - {}", symbol);
        }
    }
    
    // Let's also try to manually call node_embedding_start to see what happens
    println!("\nTrying to manually call node_embedding_start...");
    
    type NodeEmbeddingStartFn = unsafe extern "C" fn(argc: std::ffi::c_int, argv: *const *const std::ffi::c_char);
    
    match unsafe { lib.get::<NodeEmbeddingStartFn>(b"node_embedding_start") } {
        Ok(func) => {
            println!("✓ Got node_embedding_start function pointer");
            
            // Try to call it with minimal args
            let args = vec![
                CString::new("node").unwrap(),
                CString::new("--version").unwrap(),
            ];
            let arg_ptrs: Vec<*const std::ffi::c_char> = args.iter().map(|s| s.as_ptr()).collect();
            
            println!("Attempting to call node_embedding_start...");
            unsafe {
                func(arg_ptrs.len() as std::ffi::c_int, arg_ptrs.as_ptr());
            }
            println!("✓ node_embedding_start called successfully");
        }
        Err(e) => {
            println!("✗ Failed to get node_embedding_start: {}", e);
        }
    }
}
