use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    Emitter, Manager,
};

use font_kit::source::SystemSource;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::sync::OnceLock;
use std::process::Command;
use std::time::Duration;

#[derive(Serialize, Clone)]
struct FontFamily {
    family: String,
    styles: Vec<String>,
}

static FONT_CACHE: OnceLock<Vec<FontFamily>> = OnceLock::new();

const UIVERSE_HOST: &str = "uiverse.io";
const UIVERSE_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";
const UIVERSE_CURL_STATUS_MARKER: &str = "\n__UIVERSE_HTTP_STATUS__:";

#[cfg(windows)]
const UIVERSE_CURL_BINARY: &str = "curl.exe";
#[cfg(not(windows))]
const UIVERSE_CURL_BINARY: &str = "curl";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiversePostImport {
    url: String,
    id: String,
    username: String,
    friendly_id: String,
    #[serde(rename = "type")]
    post_type: Option<String>,
    theme: Option<String>,
    background_color: Option<String>,
    version: u32,
    title: Option<String>,
    author_name: Option<String>,
    author_username: Option<String>,
    source_website: Option<String>,
    html: String,
    css: String,
}

#[derive(Deserialize)]
struct UiverseRouteDataResponse {
    post: UiversePostSummary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiversePostSummary {
    id: String,
    friendly_id: String,
    version: u32,
    #[serde(default, rename = "type")]
    post_type: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    background_color: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    user: Option<UiversePostUser>,
    #[serde(default)]
    post_source: Option<UiversePostSource>,
}

#[derive(Deserialize)]
struct UiversePostUser {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct UiversePostSource {
    #[serde(default)]
    website: Option<String>,
}

#[derive(Deserialize)]
struct UiversePostCodeResponse {
    html: String,
    css: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FigImageAsset {
    hash: String,
    data: Vec<u8>,
}

#[derive(Debug)]
struct UiverseFetchError {
    message: String,
    status: Option<reqwest::StatusCode>,
    content_type: Option<String>,
    body: String,
}

struct ParsedUiverseUrl {
    username: String,
    friendly_id: String,
    canonical_url: String,
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_uiverse_post_url(input: &str) -> Result<ParsedUiverseUrl, String> {
    let parsed = reqwest::Url::parse(input)
        .map_err(|err| format!("Invalid URL: {err}"))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Unsupported URL scheme. Use http:// or https://".to_string());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL is missing a host".to_string())?
        .to_ascii_lowercase();
    if host != UIVERSE_HOST && host != format!("www.{UIVERSE_HOST}") {
        return Err(format!(
            "Unsupported host '{host}'. Expected '{UIVERSE_HOST}'"
        ));
    }

    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or_else(|| "URL path is malformed".to_string())?
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.len() != 2 {
        return Err(
            "Uiverse URL must look like https://uiverse.io/{username}/{friendlyId}".to_string(),
        );
    }

    let username = segments[0].trim().to_string();
    let friendly_id = segments[1].trim().to_string();
    if username.is_empty() || friendly_id.is_empty() {
        return Err(
            "Uiverse URL must include both username and friendlyId path segments".to_string(),
        );
    }

    let canonical_url = format!("https://{UIVERSE_HOST}/{username}/{friendly_id}");
    Ok(ParsedUiverseUrl {
        username,
        friendly_id,
        canonical_url,
    })
}

fn looks_like_html(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("<!doctype html") || lower.contains("<html") || lower.contains("cloudflare")
}

fn should_fallback_to_curl(err: &UiverseFetchError) -> bool {
    matches!(err.status, Some(status) if status == reqwest::StatusCode::FORBIDDEN)
        || err
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.to_ascii_lowercase().contains("text/html"))
        || looks_like_html(&err.body)
}

async fn fetch_json_with_reqwest<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: reqwest::Url,
) -> Result<T, UiverseFetchError> {
    let response = client
        .get(url.clone())
        .header(reqwest::header::USER_AGENT, UIVERSE_USER_AGENT)
        .header("X-Requested-With", "XMLHttpRequest")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|err| UiverseFetchError {
            message: format!("Request failed for {url}: {err}"),
            status: None,
            content_type: None,
            body: String::new(),
        })?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let body = response.text().await.map_err(|err| UiverseFetchError {
        message: format!("Failed to read response body from {url}: {err}"),
        status: Some(status),
        content_type: content_type.clone(),
        body: String::new(),
    })?;

    if !status.is_success() {
        let snippet: String = body.chars().take(200).collect();
        return Err(UiverseFetchError {
            message: format!("Uiverse request failed ({status}) for {url}. Body: {snippet}"),
            status: Some(status),
            content_type,
            body,
        });
    }

    serde_json::from_str::<T>(&body).map_err(|err| UiverseFetchError {
        message: format!("Invalid JSON response from {url}: {err}"),
        status: Some(status),
        content_type,
        body,
    })
}

async fn fetch_json_with_curl<T: DeserializeOwned + Send + 'static>(
    url: reqwest::Url,
) -> Result<T, String> {
    let url_string = url.to_string();
    let error_url = url_string.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = Command::new(UIVERSE_CURL_BINARY);
        command
            .args([
                "-sS",
                "-L",
                "--compressed",
                "-H",
                &format!("User-Agent: {UIVERSE_USER_AGENT}"),
                "-H",
                "X-Requested-With: XMLHttpRequest",
                "-H",
                "Accept: application/json",
                "-o",
                "-",
                "-w",
                &format!("{UIVERSE_CURL_STATUS_MARKER}%{{http_code}}"),
                &url_string,
            ]);

        let output = command
            .output()
            .map_err(|err| format!("Failed to run {UIVERSE_CURL_BINARY} for {url_string}: {err}"))?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(|err| format!("Uiverse curl output for {url_string} was not valid UTF-8: {err}"))?;
        let (body, status_part) = stdout.rsplit_once(UIVERSE_CURL_STATUS_MARKER).ok_or_else(|| {
            format!("Uiverse curl output for {url_string} did not include an HTTP status marker")
        })?;

        let status = status_part
            .trim()
            .parse::<u16>()
            .map_err(|err| format!("Failed to parse curl HTTP status for {url_string}: {err}"))?;

        if status != 200 {
            let snippet: String = body.chars().take(200).collect();
            return Err(format!(
                "Uiverse request failed ({status}) for {url_string}. Body: {snippet}"
            ));
        }

        serde_json::from_str::<T>(body)
            .map_err(|err| format!("Invalid JSON response from {url_string}: {err}"))
    })
    .await
    .map_err(|err| format!("Curl task failed for {error_url}: {err}"))?
}

async fn fetch_json<T: DeserializeOwned + Send + 'static>(
    client: &reqwest::Client,
    url: reqwest::Url,
) -> Result<T, String> {
    match fetch_json_with_reqwest(client, url.clone()).await {
        Ok(value) => Ok(value),
        Err(err) if should_fallback_to_curl(&err) => {
            match fetch_json_with_curl(url).await {
                Ok(value) => Ok(value),
                Err(curl_err) => Err(format!("{}; curl fallback also failed: {curl_err}", err.message)),
            }
        }
        Err(err) => Err(err.message),
    }
}

fn enumerate_system_fonts() -> Vec<FontFamily> {
    let source = SystemSource::new();
    let mut families: Vec<FontFamily> = Vec::new();

    if let Ok(family_names) = source.all_families() {
        for name in &family_names {
            if let Ok(handle) = source.select_family_by_name(name) {
                let styles: Vec<String> = handle
                    .fonts()
                    .iter()
                    .filter_map(|f| {
                        f.load().ok().map(|font| {
                            let props = font.properties();
                            let mut style = match props.weight.0 as i32 {
                                0..=150 => "Thin",
                                151..=250 => "ExtraLight",
                                251..=350 => "Light",
                                351..=450 => "Regular",
                                451..=550 => "Medium",
                                551..=650 => "SemiBold",
                                651..=750 => "Bold",
                                751..=850 => "ExtraBold",
                                _ => "Black",
                            }
                            .to_string();
                            if props.style == font_kit::properties::Style::Italic {
                                style.push_str(" Italic");
                            }
                            style
                        })
                    })
                    .collect();

                if !styles.is_empty() {
                    families.push(FontFamily {
                        family: name.clone(),
                        styles,
                    });
                }
            }
        }
    }

    families.sort_by(|a, b| a.family.cmp(&b.family));
    families
}

#[tauri::command]
async fn list_system_fonts() -> Vec<FontFamily> {
    if let Some(cached) = FONT_CACHE.get() {
        return cached.clone();
    }

    let families = tauri::async_runtime::spawn_blocking(enumerate_system_fonts)
        .await
        .unwrap_or_default();
    let _ = FONT_CACHE.set(families.clone());
    families
}

fn load_system_font_blocking(family: String, style: String) -> Result<Vec<u8>, String> {
    let source = SystemSource::new();
    let family_handle = source
        .select_family_by_name(&family)
        .map_err(|e| format!("Font family not found: {e}"))?;

    let is_italic = style.contains("Italic");
    let weight_str = style.replace(" Italic", "");
    let weight = match weight_str.as_str() {
        "Thin" => font_kit::properties::Weight::THIN,
        "ExtraLight" => font_kit::properties::Weight::EXTRA_LIGHT,
        "Light" => font_kit::properties::Weight::LIGHT,
        "Regular" | "" => font_kit::properties::Weight::NORMAL,
        "Medium" => font_kit::properties::Weight::MEDIUM,
        "SemiBold" => font_kit::properties::Weight::SEMIBOLD,
        "Bold" => font_kit::properties::Weight::BOLD,
        "ExtraBold" => font_kit::properties::Weight::EXTRA_BOLD,
        "Black" => font_kit::properties::Weight::BLACK,
        _ => font_kit::properties::Weight::NORMAL,
    };
    let style_prop = if is_italic {
        font_kit::properties::Style::Italic
    } else {
        font_kit::properties::Style::Normal
    };

    for handle in family_handle.fonts() {
        if let Ok(font) = handle.load() {
            let props = font.properties();
            let w_diff = (props.weight.0 - weight.0).abs();
            if w_diff < 50.0 && props.style == style_prop {
                if let Some(data) = font.copy_font_data() {
                    return Ok((*data).clone());
                }
            }
        }
    }

    // Fallback: return first font in family
    if let Some(handle) = family_handle.fonts().first() {
        if let Ok(font) = handle.load() {
            if let Some(data) = font.copy_font_data() {
                return Ok((*data).clone());
            }
        }
    }

    Err(format!("Could not load font {family} {style}"))
}

#[tauri::command]
async fn load_system_font(family: String, style: String) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || load_system_font_blocking(family, style))
        .await
        .map_err(|e| format!("Font load task failed: {e}"))?
}

#[tauri::command]
async fn fetch_uiverse_post(url: String) -> Result<UiversePostImport, String> {
    let parsed = parse_uiverse_post_url(&url)?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?;

    let mut metadata_url =
        reqwest::Url::parse(&format!("https://{UIVERSE_HOST}/{}/{}", parsed.username, parsed.friendly_id))
            .map_err(|err| format!("Failed to build metadata URL: {err}"))?;
    metadata_url
        .query_pairs_mut()
        .append_pair("_data", "routes/$username.$friendlyId");

    let metadata: UiverseRouteDataResponse = fetch_json(&client, metadata_url).await?;
    if metadata.post.id.trim().is_empty() {
        return Err("Uiverse metadata response did not contain a post id".to_string());
    }

    let mut code_url =
        reqwest::Url::parse(&format!("https://{UIVERSE_HOST}/resource/post/code/{}", metadata.post.id))
            .map_err(|err| format!("Failed to build code URL: {err}"))?;
    code_url
        .query_pairs_mut()
        .append_pair("v", &metadata.post.version.to_string())
        .append_pair("_data", "routes/resource.post.code.$id");

    let code: UiversePostCodeResponse = fetch_json(&client, code_url).await?;

    let author_name = normalize_optional(
        metadata
            .post
            .user
            .as_ref()
            .and_then(|user| user.name.clone()),
    );
    let author_username = normalize_optional(
        metadata
            .post
            .user
            .as_ref()
            .and_then(|user| user.username.clone()),
    )
    .or_else(|| Some(parsed.username.clone()));

    Ok(UiversePostImport {
        url: parsed.canonical_url,
        id: metadata.post.id,
        username: parsed.username,
        friendly_id: normalize_optional(Some(metadata.post.friendly_id))
            .unwrap_or(parsed.friendly_id),
        post_type: normalize_optional(metadata.post.post_type),
        theme: normalize_optional(metadata.post.theme),
        background_color: normalize_optional(metadata.post.background_color),
        version: metadata.post.version,
        title: normalize_optional(metadata.post.title),
        author_name,
        author_username,
        source_website: normalize_optional(
            metadata
                .post
                .post_source
                .as_ref()
                .and_then(|source| source.website.clone()),
        ),
        html: code.html,
        css: code.css,
    })
}

#[tauri::command]
fn build_fig_file(
    schema_deflated: Vec<u8>,
    kiwi_data: Vec<u8>,
    thumbnail_png: Vec<u8>,
    meta_json: String,
    images: Vec<FigImageAsset>,
) -> Result<Vec<u8>, String> {
    use std::io::{Cursor, Write};

    // Zstd-compress kiwi data with content size in frame header
    let mut encoder = zstd::Encoder::new(Vec::new(), 3).map_err(|e| e.to_string())?;
    encoder
        .include_contentsize(true)
        .map_err(|e| e.to_string())?;
    encoder
        .set_pledged_src_size(Some(kiwi_data.len() as u64))
        .map_err(|e| e.to_string())?;
    encoder.write_all(&kiwi_data).map_err(|e| e.to_string())?;
    let zstd_data = encoder.finish().map_err(|e| e.to_string())?;

    // Build fig-kiwi container
    let version: u32 = 106;
    let fig_kiwi_len = 8 + 4 + 4 + schema_deflated.len() + 4 + zstd_data.len();
    let mut fig_kiwi = Vec::with_capacity(fig_kiwi_len);
    fig_kiwi.extend_from_slice(b"fig-kiwi");
    fig_kiwi.extend_from_slice(&version.to_le_bytes());
    fig_kiwi.extend_from_slice(&(schema_deflated.len() as u32).to_le_bytes());
    fig_kiwi.extend_from_slice(&schema_deflated);
    fig_kiwi.extend_from_slice(&(zstd_data.len() as u32).to_le_bytes());
    fig_kiwi.extend_from_slice(&zstd_data);

    // Deflate-compress the schema for verification it's already deflated
    // (schema_deflated is already deflated, we just pass it through)

    // Build ZIP with canvas.fig + thumbnail.png + meta.json (all STORED)
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("canvas.fig", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(&fig_kiwi).map_err(|e| e.to_string())?;

    zip.start_file("thumbnail.png", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(&thumbnail_png).map_err(|e| e.to_string())?;

    zip.start_file("meta.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(meta_json.as_bytes())
        .map_err(|e| e.to_string())?;

    for image in images {
        if image.hash.trim().is_empty() {
            continue;
        }
        zip.start_file(format!("images/{}", image.hash), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&image.data).map_err(|e| e.to_string())?;
    }

    let result = zip.finish().map_err(|e| e.to_string())?;
    Ok(result.into_inner())
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            build_fig_file,
            list_system_fonts,
            load_system_font,
            fetch_uiverse_post
        ])
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .on_menu_event(|app, event| {
            #[cfg(debug_assertions)]
            if event.id().0.as_str() == "dev-tools" {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_devtools_open() {
                        window.close_devtools();
                    } else {
                        window.open_devtools();
                    }
                }
                return;
            }
            let _ = app.emit("menu-event", event.id().0.as_str());
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            let app_menu = SubmenuBuilder::new(app, "OpenPencil")
                .item(&PredefinedMenuItem::about(
                    app,
                    Some("About OpenPencil"),
                    None,
                )?)
                .separator()
                .item(&PredefinedMenuItem::services(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::hide(app, None)?)
                .item(&PredefinedMenuItem::hide_others(app, None)?)
                .item(&PredefinedMenuItem::show_all(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::quit(app, None)?)
                .build()?;

            #[allow(unused_mut)]
            let mut file_menu_builder = SubmenuBuilder::new(app, "File")
                .item(
                    &MenuItemBuilder::new("New")
                        .id("new")
                        .accelerator("CmdOrCtrl+N")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Open…")
                        .id("open")
                        .accelerator("CmdOrCtrl+O")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Save")
                        .id("save")
                        .accelerator("CmdOrCtrl+S")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Save As…")
                        .id("save-as")
                        .accelerator("CmdOrCtrl+Shift+S")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Export…")
                        .id("export")
                        .accelerator("CmdOrCtrl+Shift+E")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Close Tab")
                        .id("close")
                        .accelerator("CmdOrCtrl+W")
                        .build(app)?,
                );
            #[cfg(not(target_os = "macos"))]
            {
                file_menu_builder = file_menu_builder
                    .separator()
                    .item(&PredefinedMenuItem::quit(app, None)?);
            }
            let file_menu = file_menu_builder.build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, None)?)
                .item(&PredefinedMenuItem::redo(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, None)?)
                .item(&PredefinedMenuItem::copy(app, None)?)
                .item(&PredefinedMenuItem::paste(app, None)?)
                .item(
                    &MenuItemBuilder::new("Paste in Place")
                        .id("paste-in-place")
                        .accelerator("CmdOrCtrl+Shift+V")
                        .build(app)?,
                )
                .item(&PredefinedMenuItem::select_all(app, None)?)
                .separator()
                .item(
                    &MenuItemBuilder::new("Duplicate")
                        .id("duplicate")
                        .accelerator("CmdOrCtrl+D")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Delete")
                        .id("delete")
                        .accelerator("Backspace")
                        .build(app)?,
                )
                .build()?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .item(
                    &MenuItemBuilder::new("Zoom In")
                        .id("zoom-in")
                        .accelerator("CmdOrCtrl+=")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Zoom Out")
                        .id("zoom-out")
                        .accelerator("CmdOrCtrl+-")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Zoom to Fit")
                        .id("zoom-fit")
                        .accelerator("CmdOrCtrl+1")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Zoom to 100%")
                        .id("zoom-100")
                        .accelerator("CmdOrCtrl+0")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Toggle Rulers")
                        .id("toggle-rulers")
                        .accelerator("Shift+R")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Toggle Grid")
                        .id("toggle-grid")
                        .accelerator("CmdOrCtrl+'")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Toggle UI")
                        .id("toggle-ui")
                        .accelerator("CmdOrCtrl+\\")
                        .build(app)?,
                )
                .item(&PredefinedMenuItem::fullscreen(app, None)?)
                .separator()
                .item(
                    &MenuItemBuilder::new("Developer Tools")
                        .id("dev-tools")
                        .accelerator("CmdOrCtrl+Alt+I")
                        .build(app)?,
                )
                .build()?;

            let object_menu = SubmenuBuilder::new(app, "Object")
                .item(
                    &MenuItemBuilder::new("Group Selection")
                        .id("group")
                        .accelerator("CmdOrCtrl+G")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Ungroup Selection")
                        .id("ungroup")
                        .accelerator("CmdOrCtrl+Shift+G")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Frame Selection")
                        .id("frame-selection")
                        .accelerator("CmdOrCtrl+Alt+G")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Bring to Front")
                        .id("bring-front")
                        .accelerator("CmdOrCtrl+]")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Send to Back")
                        .id("send-back")
                        .accelerator("CmdOrCtrl+[")
                        .build(app)?,
                )
                .separator()
                .item(
                    &MenuItemBuilder::new("Flip Horizontal")
                        .id("flip-h")
                        .accelerator("Shift+H")
                        .build(app)?,
                )
                .item(
                    &MenuItemBuilder::new("Flip Vertical")
                        .id("flip-v")
                        .accelerator("Shift+V")
                        .build(app)?,
                )
                .build()?;

            let window_menu = SubmenuBuilder::new(app, "Window")
                .item(&PredefinedMenuItem::minimize(app, None)?)
                .item(&PredefinedMenuItem::maximize(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::close_window(app, None)?)
                .build()?;

            #[allow(unused_mut)]
            let mut help_menu_builder = SubmenuBuilder::new(app, "Help").item(
                &MenuItemBuilder::new("Keyboard Shortcuts")
                    .id("shortcuts")
                    .accelerator("CmdOrCtrl+/")
                    .build(app)?,
            );
            #[cfg(not(target_os = "macos"))]
            {
                help_menu_builder = help_menu_builder
                    .separator()
                    .item(&PredefinedMenuItem::about(
                        app,
                        Some("About OpenPencil"),
                        None,
                    )?);
            }
            let help_menu = help_menu_builder.build()?;

            let mut builder = MenuBuilder::new(app);
            #[cfg(target_os = "macos")]
            {
                builder = builder.item(&app_menu);
            }
            let menu = builder
                .items(&[
                    &file_menu,
                    &edit_menu,
                    &view_menu,
                    &object_menu,
                    &window_menu,
                    &help_menu,
                ])
                .build()?;

            app.set_menu(menu)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    show_main_window(app);
                }
            }
        });
}
