import re

with open('src/json_storage.rs', 'r') as f:
    content = f.read()

# 1. Update struct JsonDatabase
content = re.sub(
    r'pub struct JsonDatabase \{\s*inner: Arc<RwLock<JsonDatabaseInternal>>,\s*base_path: PathBuf,\s*\}',
    'pub struct JsonDatabase {\n    inner: Arc<RwLock<JsonDatabaseInternal>>,\n    save_tx: tokio::sync::mpsc::Sender<()>,\n}',
    content
)

# 2. Update load() function
load_replacement = """        let inner = Arc::new(RwLock::new(internal));
        let (save_tx, mut save_rx) = tokio::sync::mpsc::channel(100);
        
        let bg_inner = inner.clone();
        let bg_base_path = base_path.clone();
        tokio::spawn(async move {
            loop {
                if save_rx.recv().await.is_none() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                while let Ok(_) = save_rx.try_recv() {}
                
                let (commands_clone, binds_clone, suggestions_clone) = {
                    if let Ok(db) = bg_inner.read() {
                        (db.commands.clone(), db.binds.clone(), db.bind_suggestions.clone())
                    } else {
                        continue;
                    }
                };
                
                let _ = Self::save_json_file(&bg_base_path.join(COMMANDS_FILE), &commands_clone).await;
                let _ = Self::save_json_file(&bg_base_path.join(BINDS_FILE), &binds_clone).await;
                let _ = Self::save_json_file(&bg_base_path.join(BIND_SUGGESTIONS_FILE), &suggestions_clone).await;
            }
        });
        
        Ok(JsonDatabase {
            inner,
            save_tx,
        })"""
content = re.sub(
    r'Ok\(JsonDatabase \{\s*inner: Arc::new\(RwLock::new\(internal\)\),\s*base_path,\s*\}\)',
    load_replacement,
    content
)

# 3. Replace saving calls
content = re.sub(
    r'Self::save_json_file\(&self\.base_path\.join\([A-Z_]+\), &[a-z_]+_clone\)\.await\?;',
    'let _ = self.save_tx.send(()).await;',
    content
)

with open('src/json_storage.rs', 'w') as f:
    f.write(content)

