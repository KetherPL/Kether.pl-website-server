// SPDX-License-Identifier: GPL-3.0-only

use rocket::{catch, catchers, get, http::{ContentType, Status}, response::{self, Responder, Response}, routes, Catcher, Route, Request};
use std::fs;
use std::path::{Path, PathBuf};
use rocket::serde::Serialize;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: Option<u64>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct DirectoryListing {
    path: String,
    parent_path: Option<String>,
    entries: Vec<FileEntry>,
}

struct HtmlResponse(String);

impl<'r> Responder<'r, 'static> for HtmlResponse {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(ContentType::HTML)
            .sized_body(self.0.len(), std::io::Cursor::new(self.0))
            .ok()
    }
}

fn render_directory_listing_html(listing: &DirectoryListing) -> String {
    let mut html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Index of /fastdl/{}</title>
    <style>
        body {{
            font-family: monospace;
            margin: 20px;
            background-color: #f5f5f5;
        }}
        h1 {{
            border-bottom: 2px solid #ccc;
            padding-bottom: 10px;
        }}
        table {{
            border-collapse: collapse;
            width: 100%;
            background-color: white;
        }}
        th, td {{
            text-align: left;
            padding: 8px 12px;
            border-bottom: 1px solid #ddd;
        }}
        th {{
            background-color: #f0f0f0;
            font-weight: bold;
        }}
        tr:hover {{
            background-color: #f9f9f9;
        }}
        .dir {{
            font-weight: bold;
        }}
        .dir::before {{
            content: "📁 ";
        }}
        .file::before {{
            content: "📄 ";
        }}
        .size {{
            text-align: right;
        }}
        a {{
            text-decoration: none;
            color: #0066cc;
        }}
        a:hover {{
            text-decoration: underline;
        }}
        .parent {{
            font-weight: bold;
        }}
        .parent::before {{
            content: "⬆️ ";
        }}
    </style>
</head>
<body>
    <h1>Index of /fastdl/{}</h1>
    
    <table>
        <thead>
            <tr>
                <th>Name</th>
                <th>Size</th>
            </tr>
        </thead>
        <tbody>"#, listing.path, listing.path);

    // Add parent directory link if not at root
    if let Some(parent) = &listing.parent_path {
        html.push_str(&format!(r#"
            <tr>
                <td><a href="/fastdl/{}" class="parent">Parent Directory</a></td>
                <td class="size">-</td>
            </tr>"#, parent));
    }

    // Add directory and file entries
    for entry in &listing.entries {
        let link_path = if listing.path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", listing.path, entry.name)
        };

        let size_display = if let Some(size) = entry.size {
            if size > 1048576 {
                format!("{}M", size / 1048576)
            } else if size > 1024 {
                format!("{}K", size / 1024)
            } else {
                size.to_string()
            }
        } else {
            "-".to_string()
        };

        if entry.is_dir {
            html.push_str(&format!(r#"
            <tr>
                <td>
                    <a href="/fastdl/{}/" class="dir">{}/</a>
                </td>
                <td class="size">{}</td>
            </tr>"#, link_path, entry.name, size_display));
        } else {
            html.push_str(&format!(r#"
            <tr>
                <td>
                    <a href="/fastdl/{}" class="file">{}</a>
                </td>
                <td class="size">{}</td>
            </tr>"#, link_path, entry.name, size_display));
        }
    }

    html.push_str(r#"
        </tbody>
    </table>
    
    <hr>
    <small>Kether FastDL Server</small>
</body>
</html>"#);

    html
}

/// Implementation of directory listing logic
async fn directory_listing_impl(path: PathBuf) -> Result<HtmlResponse, rocket::http::Status> {
    let fastdl_root = Path::new("./fastdl");
    let full_path = fastdl_root.join(&path);
    
    // Security check: ensure the path is within fastdl directory
    if !full_path.starts_with(fastdl_root) {
        return Err(rocket::http::Status::Forbidden);
    }
    
    // Check if path exists
    if !full_path.exists() {
        return Err(rocket::http::Status::NotFound);
    }
    
    // If it's a file, let FileServer handle it
    if full_path.is_file() {
        return Err(rocket::http::Status::NotFound);
    }
    
    // If it's not a directory, return 404
    if !full_path.is_dir() {
        return Err(rocket::http::Status::NotFound);
    }
    
    // Check if index.html exists (let FileServer handle it)
    let index_path = full_path.join("index.html");
    if index_path.exists() {
        return Err(rocket::http::Status::NotFound); // Let FileServer handle this
    }
    
    // Read directory contents
    let mut entries = Vec::new();
    
    if let Ok(dir_entries) = fs::read_dir(&full_path) {
        for entry in dir_entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = metadata.is_dir();
                let size = if is_dir { None } else { Some(metadata.len()) };
                
                entries.push(FileEntry {
                    name,
                    is_dir,
                    size,
                });
            }
        }
    }
    
    // Sort entries: directories first, then files, both alphabetically
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
    
    let path_str = path.to_string_lossy().to_string();
    let parent_path = if path_str.is_empty() {
        None
    } else {
        path.parent().map(|p| p.to_string_lossy().to_string())
    };
    
    let listing = DirectoryListing {
        path: path_str,
        parent_path,
        entries,
    };
    
    Ok(HtmlResponse(render_directory_listing_html(&listing)))
}

/// Custom 404 catcher for FastDL that provides directory listings
#[catch(404)]
pub async fn fastdl_not_found(req: &Request<'_>) -> Option<HtmlResponse> {
    let uri_path = req.uri().path().as_str();
    
    // Only handle requests that start with /fastdl/
    if !uri_path.starts_with("/fastdl/") {
        return None; // Let other catchers handle it
    }
    
    // Extract the path after /fastdl/
    let fastdl_path = &uri_path[8..]; // Remove "/fastdl/" prefix
    let path = PathBuf::from(fastdl_path);
    
    // Check if this could be a directory request
    let fastdl_root = Path::new("./fastdl");
    let full_path = fastdl_root.join(&path);
    
    // Security check
    if !full_path.starts_with(fastdl_root) {
        return None;
    }
    
    // If it's a directory without index.html, serve directory listing
    if full_path.exists() && full_path.is_dir() {
        let index_path = full_path.join("index.html");
        if !index_path.exists() {
            match directory_listing_impl(path).await {
                Ok(response) => return Some(response),
                Err(_) => return None,
            }
        }
    }
    
    None // Let the global catcher handle it
}

/// Mount FastDL catchers
pub fn mount_fastdl_catchers() -> Vec<Catcher> {
    catchers![fastdl_not_found]
}

/// Mount FastDL routes (empty now, using catcher approach)
pub fn mount_fastdl_routes() -> Vec<Route> {
    routes![]
}