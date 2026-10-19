//! Keeps the webview on the app's own pages. A link in previewed content
//! (e.g. `target="_top"`) or any other navigation to an outside site is
//! refused, so remote content can never run with access to the app's IPC.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Runtime, Url};

pub fn guard<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("navigation-guard")
        .on_navigation(|_webview, url| {
            let allowed = is_allowed(url);
            if !allowed {
                // Only the scheme and host: the rest may carry signed parameters.
                tracing::warn!(
                    scheme = url.scheme(),
                    host = url.host_str().unwrap_or(""),
                    "blocked navigation away from the app"
                );
            }
            allowed
        })
        .build()
}

fn is_allowed(url: &Url) -> bool {
    match url.scheme() {
        // macOS / Linux production origin, and in-app previews.
        "tauri" | "blob" => true,
        "about" => url.as_str() == "about:blank",
        // Windows production origin; the Vite dev server in debug builds.
        "http" | "https" => match url.host_str() {
            Some("tauri.localhost") => true,
            Some("localhost") => cfg!(debug_assertions) && url.port() == Some(1420),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(url: &str) -> bool {
        is_allowed(&Url::parse(url).unwrap())
    }

    #[test]
    fn allows_only_the_app_itself() {
        assert!(allowed("tauri://localhost/#/about"));
        assert!(allowed("http://tauri.localhost/index.html"));
        assert!(allowed("https://tauri.localhost/"));
        assert!(allowed("blob:tauri://localhost/2b6c0f6e"));
        assert!(allowed("about:blank"));

        assert!(!allowed("https://evil.example/phish"));
        assert!(!allowed("http://tauri.localhost.evil.example/"));
        assert!(!allowed("file:///etc/passwd"));
        assert!(!allowed("javascript:alert(1)"));
        assert!(!allowed("http://localhost:8080/"));
    }
}
