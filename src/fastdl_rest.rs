// SPDX-License-Identifier: GPL-3.0-only

use rocket::{catch, catchers, http::{ContentType, Header, Status}, response::{self, Responder, Response}, Catcher, Request, fairing::{Fairing, Info, Kind}};
use smol::{fs, stream::StreamExt};
use once_cell::sync::OnceCell;
use std::path::{Component, Path, PathBuf};
use rocket::serde::Serialize;

static CANONICAL_FASTDL_ROOT: OnceCell<PathBuf> = OnceCell::new();

fn fastdl_root_rel() -> &'static Path {
	Path::new("./fastdl")
}

fn canonical_fastdl_root() -> Result<&'static Path, Status> {
	CANONICAL_FASTDL_ROOT
		.get_or_try_init(|| {
			std::fs::canonicalize(fastdl_root_rel()).map_err(|_| Status::InternalServerError)
		})
		.map(|p| p.as_path())
}

/// Resolve a request-relative path under `root`, rejecting traversal attempts.
fn resolve_fastdl_path_under(root: &Path, rel: &Path) -> Result<PathBuf, Status> {
	if rel.is_absolute() {
		return Err(Status::Forbidden);
	}

	for component in rel.components() {
		if matches!(component, Component::ParentDir) {
			return Err(Status::Forbidden);
		}
	}

	let joined = root.join(rel);

	if joined.exists() {
		let canonical = std::fs::canonicalize(&joined).map_err(|_| Status::NotFound)?;
		if !canonical.starts_with(root) {
			return Err(Status::Forbidden);
		}
		return Ok(canonical);
	}

	let mut current = joined.as_path();
	while !current.exists() {
		match current.parent() {
			Some(parent) if parent.starts_with(root) => current = parent,
			Some(parent) if parent == root => current = parent,
			_ => return Err(Status::NotFound),
		}
	}

	let canonical_ancestor = std::fs::canonicalize(current).map_err(|_| Status::NotFound)?;
	if !canonical_ancestor.starts_with(root) {
		return Err(Status::Forbidden);
	}

	Ok(joined)
}

/// Resolve a request-relative path under the FastDL root, rejecting traversal attempts.
fn resolve_fastdl_path(rel: &Path) -> Result<PathBuf, Status> {
	resolve_fastdl_path_under(canonical_fastdl_root()?, rel)
}

enum FastdlCatchResponse {
	Html(HtmlResponse),
	Status(Status),
}

impl<'r> Responder<'r, 'static> for FastdlCatchResponse {
	fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
		match self {
			Self::Html(html) => html.respond_to(req),
			Self::Status(status) => status.respond_to(req),
		}
	}
}

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
            // .header(Header::new("Cache-Control", "max-age=31536000")) // Cache for 1 year
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
            font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
            margin: 20px;
            background-color: #1a1a1a;
            color: #e0e0e0;
        }}
        h1 {{
            border-bottom: 2px solid #444;
            padding-bottom: 10px;
            color: #fff;
            text-shadow: 0 0 10px #4a9eff;
        }}
        table {{
            border-collapse: collapse;
            width: 100%;
            background-color: #2d2d2d;
            border: 1px solid #444;
            border-radius: 8px;
            overflow: hidden;
        }}
        th, td {{
            text-align: left;
            padding: 12px 16px;
            border-bottom: 1px solid #444;
        }}
        th {{
            background-color: #3a3a3a;
            font-weight: bold;
            color: #fff;
            text-transform: uppercase;
            font-size: 0.9em;
            letter-spacing: 1px;
        }}
        tr:hover {{
            background-color: #404040;
            transition: background-color 0.2s ease;
        }}
        .dir {{
            font-weight: bold;
            color: #4a9eff;
        }}
        .dir::before {{
            content: "📁 ";
            filter: brightness(1.2);
        }}
        .file::before {{
            content: "📄 ";
            filter: brightness(1.2);
        }}
        .size {{
            text-align: right;
            color: #aaa;
            font-family: monospace;
        }}
        a {{
            text-decoration: none;
            color: #4a9eff;
            transition: color 0.2s ease;
        }}
        a:hover {{
            color: #66b3ff;
            text-decoration: underline;
            text-shadow: 0 0 5px #4a9eff;
        }}
        .parent {{
            font-weight: bold;
            color: #ff9500;
        }}
        .parent::before {{
            content: "⬆️ ";
        }}
        .parent:hover {{
            color: #ffb84d;
            text-shadow: 0 0 5px #ff9500;
        }}
        hr {{
            border: none;
            border-top: 1px solid #444;
            margin: 20px 0;
        }}
        small {{
            color: #888;
            font-style: italic;
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
    <small>Kether FastDL Server</small><br>
    <small>Part of the <a href="https://github.com/KetherPL/Kether.pl-website-server">Kether Internal Services Server</a></small><br>
    <small>Powered by <a href="https://www.rust-lang.org/">Rust</a>, <a href="https://github.com/SergioBenitez/Rocket">Rocket</a> and <a href="https://github.com/smol-rs/smol">Smol</a></small>
</body>
</html>"#);

    html
}

/// Implementation of directory listing logic
async fn directory_listing_impl(path: PathBuf) -> Result<HtmlResponse, Status> {
    let full_path = resolve_fastdl_path(&path)?;
    
    // Check if path exists and get metadata
    let metadata = match fs::metadata(&full_path).await {
        Ok(meta) => meta,
        Err(_) => return Err(Status::NotFound),
    };
    
    // If it's a file, let FileServer handle it
    if metadata.is_file() {
        return Err(Status::NotFound);
    }
    
    // If it's not a directory, return 404
    if !metadata.is_dir() {
        return Err(Status::NotFound);
    }
    
    // Check if index.html exists (let FileServer handle it)
    let index_path = full_path.join("index.html");
    if fs::metadata(&index_path).await.is_ok() {
        return Err(Status::NotFound); // Let FileServer handle this
    }
    
    // Read directory contents
    let mut entries = Vec::new();
    
    if let Ok(dir_entries) = fs::read_dir(&full_path).await {
        let entries_stream: Vec<_> = dir_entries.collect().await;
        for entry_result in entries_stream {
            if let Ok(entry) = entry_result
                && let Ok(metadata) = entry.metadata().await
            {
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
async fn fastdl_not_found(req: &Request<'_>) -> Option<FastdlCatchResponse> {
    let uri_path = req.uri().path().as_str();
    
    // Only handle requests that start with /fastdl/
    if !uri_path.starts_with("/fastdl/") {
        return None; // Let other catchers handle it
    }
    
    // Extract the path after /fastdl/
    let fastdl_path = &uri_path[8..]; // Remove "/fastdl/" prefix
    let path = PathBuf::from(fastdl_path);

    let full_path = match resolve_fastdl_path(&path) {
        Ok(p) => p,
        Err(e) if e.code == Status::Forbidden.code => {
            return Some(FastdlCatchResponse::Status(Status::Forbidden));
        }
        Err(_) => return None,
    };
    
    // If it's a directory without index.html, serve directory listing
    if let Ok(metadata) = fs::metadata(&full_path).await
        && metadata.is_dir()
    {
        let index_path = full_path.join("index.html");
        if fs::metadata(&index_path).await.is_err() {
            match directory_listing_impl(path).await {
                Ok(response) => return Some(FastdlCatchResponse::Html(response)),
                Err(e) if e.code == Status::Forbidden.code => {
                    return Some(FastdlCatchResponse::Status(Status::Forbidden));
                }
                Err(_) => return None,
            }
        }
    }
    
    None // Let the global catcher handle it
}

// Custom fairing to add Cache-Control headers to FastDL responses
pub struct FastDLCacheHeaders;

#[rocket::async_trait]
impl Fairing for FastDLCacheHeaders {
    fn info(&self) -> Info {
        Info {
            name: "FastDL Cache Headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        // Only add cache headers to FastDL requests
        if req.uri().path().as_str().starts_with("/fastdl/l4d2_kether/resource/") {
            // Add long cache for static files (1 year)
            res.set_header(Header::new("Cache-Control", "public, max-age=31536000, immutable"));
            // Add ETag for better caching
            if let Some(content_length) = res.headers().get_one("Content-Length") {
                let etag = format!("\"{}\"", content_length); // Simple ETag based on content length
                res.set_header(Header::new("ETag", etag));
            }
        }
    }
}

/// Mount FastDL catchers
pub fn mount_fastdl_catchers() -> Vec<Catcher> {
    catchers![fastdl_not_found]
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;

	#[test]
	fn resolve_valid_nested_path() {
		let temp = tempfile::TempDir::new().expect("tempdir");
		let root = temp.path().join("fastdl");
		fs::create_dir_all(root.join("foo/bar")).expect("create dirs");
		fs::write(root.join("foo/bar/file.txt"), b"ok").expect("write file");
		let canonical_root = root.canonicalize().expect("canonicalize");

		let rel = Path::new("foo/bar");
		let resolved = resolve_fastdl_path_under(&canonical_root, rel).expect("valid path");
		assert!(resolved.starts_with(&canonical_root));
		assert!(resolved.ends_with("foo/bar"));
	}

	#[test]
	fn reject_parent_dir_traversal() {
		let temp = tempfile::TempDir::new().expect("tempdir");
		let root = temp.path().join("fastdl");
		fs::create_dir_all(&root).expect("create root");
		let canonical_root = root.canonicalize().expect("canonicalize");

		assert_eq!(
			resolve_fastdl_path_under(&canonical_root, Path::new("../etc/passwd")),
			Err(Status::Forbidden)
		);
		assert_eq!(
			resolve_fastdl_path_under(&canonical_root, Path::new("foo/../../outside")),
			Err(Status::Forbidden)
		);
	}

	#[test]
	fn reject_absolute_path() {
		let temp = tempfile::TempDir::new().expect("tempdir");
		let root = temp.path().join("fastdl");
		fs::create_dir_all(&root).expect("create root");
		let canonical_root = root.canonicalize().expect("canonicalize");

		assert_eq!(
			resolve_fastdl_path_under(&canonical_root, Path::new("/etc/passwd")),
			Err(Status::Forbidden)
		);
	}
}