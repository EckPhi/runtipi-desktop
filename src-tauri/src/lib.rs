use serde::{Deserialize, Serialize};
use std::{fs, sync::Mutex};
use tauri::{Emitter, Manager, Webview, WebviewUrl};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instance {
    id: Uuid,
    name: String,
    url: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    instances: Vec<Instance>,
    default_instance: Option<Uuid>,
}

struct State(Mutex<Settings>);

fn shell(webview: &Webview) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("Only the local application shell may perform this operation".into());
    }
    Ok(())
}

fn http_url(value: &str) -> Result<tauri::Url, String> {
    let url = tauri::Url::parse(value).map_err(|_| "Enter a complete http:// or https:// URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS URLs are supported".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Sign in through the dashboard instead of putting credentials in URLs".into());
    }
    Ok(url)
}

#[tauri::command]
fn load_settings(webview: Webview, state: tauri::State<State>) -> Result<Settings, String> {
    shell(&webview)?;
    Ok(state.0.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn save_settings(
    webview: Webview,
    app: tauri::AppHandle,
    state: tauri::State<State>,
    settings: Settings,
) -> Result<(), String> {
    shell(&webview)?;
    let mut ids = std::collections::HashSet::new();
    for instance in &settings.instances {
        if instance.name.trim().is_empty() {
            return Err("An instance needs a name".into());
        }
        if !ids.insert(instance.id) {
            return Err("Duplicate instance identifier".into());
        }
        http_url(&instance.url)?;
    }
    if settings
        .default_instance
        .is_some_and(|id| !ids.contains(&id))
    {
        return Err("Default instance must exist".into());
    }
    let mut stored = state.0.lock().map_err(|e| e.to_string())?;
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join("instances.json"),
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    *stored = settings;
    Ok(())
}

#[tauri::command]
async fn create_tab(
    webview: Webview,
    app: tauri::AppHandle,
    instance_id: Uuid,
    tab_id: Uuid,
    url: String,
    top: f64,
) -> Result<(), String> {
    shell(&webview)?;
    let url = http_url(&url)?;
    if !top.is_finite() || top < 0.0 {
        return Err("Invalid browser bounds".into());
    }
    {
        let state = app.state::<State>();
        if !state
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .instances
            .iter()
            .any(|i| i.id == instance_id)
        {
            return Err("Unknown instance".into());
        }
    }
    let label = format!("tab-{tab_id}");
    let events = app.clone();
    let popup_label = label.clone();
    let title_events = app.clone();
    let title_label = label.clone();
    let navigation_events = app.clone();
    let navigation_label = label.clone();
    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(url))
        .on_navigation(move |url| {
            if http_url(url.as_str()).is_err() {
                return false;
            }
            let _ = navigation_events.emit_to(
                "main",
                "tab-url",
                serde_json::json!({"label": navigation_label, "url": url.as_str()}),
            );
            true
        })
        .on_document_title_changed(move |_, title| {
            let _ = title_events.emit_to(
                "main",
                "tab-title",
                serde_json::json!({"label": title_label, "title": title}),
            );
        })
        .on_new_window(move |url, _| {
            if http_url(url.as_str()).is_ok() {
                let _ = events.emit_to(
                    "main",
                    "tab-popup",
                    serde_json::json!({"label": popup_label, "url": url.as_str()}),
                );
            }
            tauri::webview::NewWindowResponse::Deny
        });
    #[cfg(not(target_os = "macos"))]
    let builder = builder.data_directory(
        app.path()
            .app_local_data_dir()
            .map_err(|e| e.to_string())?
            .join("profiles")
            .join(instance_id.to_string()),
    );
    #[cfg(target_os = "macos")]
    let builder = builder.data_store_identifier(*instance_id.as_bytes());
    let window = app.get_window("main").ok_or("Missing main window")?;
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(window.scale_factor().map_err(|e| e.to_string())?);
    window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, top),
            tauri::LogicalSize::new(size.width, (size.height - top).max(1.0)),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn control_tab(
    webview: Webview,
    app: tauri::AppHandle,
    tab_id: Uuid,
    action: String,
    top: Option<f64>,
) -> Result<(), String> {
    shell(&webview)?;
    let tab = app
        .get_webview(&format!("tab-{tab_id}"))
        .ok_or("Tab is no longer available")?;
    let result = match action.as_str() {
        "show" => tab.show(),
        "hide" => tab.hide(),
        "close" => tab.close(),
        "back" => tab.eval("history.back()"),
        "forward" => tab.eval("history.forward()"),
        "reload" => tab.reload(),
        "resize" => {
            let top = top.ok_or("Missing browser bounds")?;
            if !top.is_finite() || top < 0.0 {
                return Err("Invalid browser bounds".into());
            }
            let window = app.get_window("main").ok_or("Missing main window")?;
            let size = window
                .inner_size()
                .map_err(|e| e.to_string())?
                .to_logical::<f64>(window.scale_factor().map_err(|e| e.to_string())?);
            // Update the rectangle together; separate position/size updates
            // read native coordinates back between calls on macOS.
            tab.set_bounds(tauri::Rect {
                position: tauri::LogicalPosition::new(0.0, top).into(),
                size: tauri::LogicalSize::new(size.width, (size.height - top).max(1.0)).into(),
            })
        }
        "external" => {
            let url = tab.url().map_err(|e| e.to_string())?;
            http_url(url.as_str())?;
            return app
                .opener()
                .open_url(url.as_str(), None::<&str>)
                .map_err(|e| e.to_string());
        }
        _ => return Err("Unknown tab action".into()),
    };
    result.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("instances.json");
            let settings = if path.exists() {
                serde_json::from_slice(&fs::read(path)?)?
            } else {
                Settings::default()
            };
            app.manage(State(Mutex::new(settings)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            create_tab,
            control_tab
        ])
        .run(tauri::generate_context!())
        .expect("error while running Runtipi Desktop");
}

#[cfg(test)]
mod tests {
    use super::http_url;
    #[test]
    fn browser_urls_reject_local_protocols_and_embedded_credentials() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:password@host",
            "not a URL",
        ] {
            assert!(http_url(url).is_err(), "accepted {url}");
        }
        for url in ["http://192.168.1.20:8080", "https://tipi.example.com"] {
            assert!(http_url(url).is_ok(), "rejected {url}");
        }
    }
}
