use crate::{http_url, shell, State};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Manager, Webview};
use tokio::{process::Command, sync::oneshot, time::timeout};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub(crate) struct LoginSummary {
    id: String,
    share_id: String,
    title: String,
}

#[derive(Serialize)]
pub(crate) struct LoginList {
    origin: String,
    items: Vec<LoginSummary>,
}

#[derive(Deserialize)]
struct CliList {
    items: Vec<CliSummary>,
}
#[derive(Deserialize)]
struct CliSummary {
    id: String,
    share_id: String,
    #[serde(default)]
    title: String,
    item_type: String,
}

#[derive(Deserialize, Serialize)]
struct Credentials {
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    urls: Vec<String>,
}
#[derive(Deserialize)]
struct ItemView {
    item: ViewedItem,
}
#[derive(Deserialize)]
struct ViewedItem {
    id: String,
    share_id: String,
    content: ItemData,
}
#[derive(Deserialize)]
struct ItemData {
    content: ItemContent,
}
#[derive(Deserialize)]
enum ItemContent {
    Login(Credentials),
}

fn cli_path(custom: Option<&str>, app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = custom.filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_absolute() || !path.is_file() {
            return Err(
                "Choose the absolute path to an installed pass-cli executable in Settings".into(),
            );
        }
        return Ok(path);
    }
    // Finder-launched Mac apps may not inherit a shell's PATH.
    let filename = if cfg!(windows) {
        "pass-cli.exe"
    } else {
        "pass-cli"
    };
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| {
            std::env::split_paths(&value)
                .map(|p| p.join(filename))
                .collect()
        })
        .unwrap_or_default();
    if let Ok(home) = app.path().home_dir() {
        candidates.push(home.join(".local/bin").join(filename));
    }
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        candidates.push(PathBuf::from(dir).join(filename));
    }
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(||
        "Proton Pass CLI was not found. Install the official pass-cli, run pass-cli login in a terminal, then set its path in Settings if necessary".into())
}

async fn run_cli(path: PathBuf, args: Vec<String>) -> Result<Vec<u8>, String> {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // No console window.
    let output = timeout(Duration::from_secs(30), command.output())
        .await
        .map_err(|_| "Proton Pass CLI timed out. Unlock/sign in through the CLI and retry")?
        .map_err(|_| "Could not start Proton Pass CLI. Check its executable path in Settings")?;
    if !output.status.success() {
        // Never forward CLI output, which can include sensitive material.
        return Err("Proton Pass CLI failed. Run pass-cli login in a terminal and check the vault name in Settings".into());
    }
    Ok(output.stdout)
}

fn configuration(app: &tauri::AppHandle) -> Result<(PathBuf, Option<String>), String> {
    let state = app.state::<State>();
    let settings = state.0.lock().map_err(|_| "Settings are unavailable")?;
    Ok((
        cli_path(settings.proton_cli_path.as_deref(), app)?,
        settings.proton_vault.clone(),
    ))
}

fn tab_origin(tab: &Webview) -> Result<String, String> {
    let url = tab
        .url()
        .map_err(|_| "Could not read the current page address")?;
    Ok(http_url(url.as_str())?.origin().ascii_serialization())
}

fn matching_origin(urls: &[String], expected: &str) -> bool {
    urls.iter().any(|value| {
        http_url(value).is_ok_and(|url| url.origin().ascii_serialization() == expected)
    })
}

#[tauri::command]
pub(crate) async fn list_logins(
    webview: Webview,
    app: tauri::AppHandle,
    tab_id: Uuid,
) -> Result<LoginList, String> {
    shell(&webview)?;
    let tab = app
        .get_webview(&format!("tab-{tab_id}"))
        .ok_or("Tab is no longer available")?;
    let origin = tab_origin(&tab)?;
    let (path, vault) = configuration(&app)?;
    let mut args = vec![
        "item".into(),
        "list".into(),
        "--output".into(),
        "json".into(),
    ];
    if let Some(vault) = vault.filter(|s| !s.is_empty()) {
        args.push(format!("--vault-name={vault}"));
    }
    let bytes = run_cli(path, args).await?;
    let list: CliList = serde_json::from_slice(&bytes).map_err(|_| "Unsupported Proton CLI item list. Update pass-cli and configure a default vault or a vault name in Settings")?;
    let items = list
        .items
        .into_iter()
        .filter(|item| item.item_type == "login")
        .map(|item| LoginSummary {
            id: item.id,
            share_id: item.share_id,
            title: item.title,
        })
        .collect();
    Ok(LoginList { origin, items })
}

#[allow(clippy::too_many_arguments)] // Tauri injects the caller and app handles.
#[tauri::command]
pub(crate) async fn fill_login(
    webview: Webview,
    app: tauri::AppHandle,
    tab_id: Uuid,
    share_id: String,
    item_id: String,
    origin: String,
    mode: String,
) -> Result<(), String> {
    shell(&webview)?;
    if !matches!(mode.as_str(), "both" | "username" | "password") {
        return Err("Unknown fill mode".into());
    }
    let tab = app
        .get_webview(&format!("tab-{tab_id}"))
        .ok_or("Tab is no longer available")?;
    if tab_origin(&tab)? != origin {
        return Err("The page changed. Open Proton Pass again before filling".into());
    }
    let (path, _) = configuration(&app)?;
    let bytes = run_cli(
        path,
        vec![
            "item".into(),
            "view".into(),
            format!("--share-id={share_id}"),
            format!("--item-id={item_id}"),
            "--output=json".into(),
        ],
    )
    .await?;
    let item: ItemView = serde_json::from_slice(&bytes)
        .map_err(|_| "The selected item is not a supported Proton Pass login")?;
    if item.item.id != item_id || item.item.share_id != share_id {
        return Err("Proton returned a different item than requested".into());
    }
    let ItemContent::Login(credentials) = item.item.content.content;
    if !matching_origin(&credentials.urls, &origin) {
        return Err("This login's saved URLs do not match this page's scheme, host, and port. Add the exact dashboard/app URL to the login in Proton Pass".into());
    }
    if tab_origin(&tab)? != origin {
        return Err("The page changed while retrieving the login. Nothing was filled".into());
    }
    let script = format!(
        "({})({}, {}, {})",
        include_str!("autofill.js"),
        serde_json::to_string(&origin).map_err(|_| "Invalid page origin")?,
        serde_json::to_string(
            &serde_json::to_string(&serde_json::json!({"username": credentials.username, "email": credentials.email, "password": credentials.password}))
                .map_err(|_| "Could not prepare the selected login")?
        )
        .map_err(|_| "Could not prepare the selected login")?,
        serde_json::to_string(&mode).map_err(|_| "Invalid fill mode")?
    );
    let (sender, receiver) = oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    tab.eval_with_callback(script, move |result| {
        if let Ok(mut slot) = sender.lock() {
            if let Some(sender) = slot.take() {
                let _ = sender.send(result);
            }
        }
    })
    .map_err(|_| "Could not fill this page")?;
    let result = timeout(Duration::from_secs(5), receiver)
        .await
        .map_err(|_| "The page did not confirm filling")?
        .map_err(|_| "The page closed before confirming filling")?;
    let mut result: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| "The page did not return a fill result")?;
    if let Some(inner) = result.as_str() {
        result = serde_json::from_str(inner).map_err(|_| "Invalid fill result")?;
    }
    match result.get("status").and_then(|value| value.as_str()) {
        Some("filled") => Ok(()),
        Some("origin_changed") => Err("The page changed. Nothing was filled".into()),
        Some("ambiguous") => Err("Multiple login fields were found. Choose a single-field fill mode or use Proton's desktop auto-type".into()),
        _ => Err("No compatible visible login fields were found. For a two-step login, choose Username or Password mode, or use Proton desktop auto-type".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origins_must_match_scheme_host_and_effective_port() {
        let urls = vec!["https://tipi.example.com/login".into()];
        assert!(matching_origin(&urls, "https://tipi.example.com"));
        for origin in [
            "http://tipi.example.com",
            "https://tipi.example.com:8443",
            "https://tipi.example.com.evil.test",
            "https://other.example.com",
        ] {
            assert!(!matching_origin(&urls, origin));
        }
    }
    #[test]
    fn cli_metadata_and_selected_login_follow_official_json_shapes() {
        let list: CliList = serde_json::from_str(
            r#"{"items":[{"id":"item","share_id":"vault","title":"Home","item_type":"login"}]}"#,
        )
        .unwrap();
        assert_eq!(list.items[0].title, "Home");
        let item: ItemView = serde_json::from_str(r#"{"item":{"id":"item","share_id":"vault","content":{"content":{"Login":{"email":"user@example.com","username":"","password":"test-only","urls":["http://localhost:8080"]}}}}}"#).unwrap();
        let ItemContent::Login(login) = item.item.content.content;
        assert!(matching_origin(&login.urls, "http://localhost:8080"));
    }
}
