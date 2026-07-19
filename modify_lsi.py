import re

with open('src/LiveServerInfo.rs', 'r') as f:
    content = f.read()

# 1. Update signature of query_server_with_retry
content = content.replace(
    'pub async fn query_server_with_retry(ip: &str, port: u16) -> Result<L4D2ServerInfo, Status> {',
    'pub async fn query_server_with_retry(ip: &str, port: u16, force_fetch: bool) -> Result<L4D2ServerInfo, Status> {\n\tlet parsed_ip: IpAddr = ip.parse().map_err(|_| Status::BadRequest)?;\n\n\tif !force_fetch {\n\t\tif let Ok(cache) = get_cache().read() {\n\t\t\tif let Some((cached_info, cached_time)) = cache.get(&(parsed_ip, port)) {\n\t\t\t\tif std::time::Instant::now().duration_since(*cached_time).as_secs() < 15 {\n\t\t\t\t\treturn Ok(cached_info.clone());\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}'
)
content = content.replace(
    'let parsed_ip: IpAddr = ip.parse().map_err(|_| Status::BadRequest)?;\n\t\n\t// Try to query with retries\n\tfor attempt in 1..=MAX_RETRIES {',
    '// Try to query with retries\n\tfor attempt in 1..=MAX_RETRIES {'
)

# 2. Update usages in LiveServerInfo.rs
content = content.replace(
    'query_server_with_retry(&ip, port).await.map(Json)',
    'query_server_with_retry(&ip, port, false).await.map(Json)'
)
content = content.replace(
    'query_server_with_retry(config.server_ip(), config.server_port()).await',
    'query_server_with_retry(config.server_ip(), config.server_port(), false).await'
)
content = content.replace(
    'query_server_with_retry(config.server2_ip(), config.server2_port()).await',
    'query_server_with_retry(config.server2_ip(), config.server2_port(), false).await'
)

# 3. Add start_background_queries function at the end
bg_queries_code = """
pub fn start_background_queries(config_handle: ConfigHandle) {
    tokio::spawn(async move {
        loop {
            let (ip1, port1, ip2, port2) = {
                if let Ok(guard) = config_handle.read() {
                    (guard.server_ip().to_string(), guard.server_port(), guard.server2_ip().to_string(), guard.server2_port())
                } else {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };
            
            // Query first server
            let _ = query_server_with_retry(&ip1, port1, true).await;
            // Query second server
            let _ = query_server_with_retry(&ip2, port2, true).await;
            
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}
"""
content += bg_queries_code

with open('src/LiveServerInfo.rs', 'w') as f:
    f.write(content)

