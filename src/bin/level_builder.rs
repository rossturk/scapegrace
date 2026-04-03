use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use scapegrace::fonts::FONT_NAMES;
use scapegrace::gen::{
    call_llm_streaming, expand_tile_defs, llm_api_key, llm_model,
    build_overworld_prompt_themed, build_single_level_design_prompt_themed,
    build_overworld_inner_for_preview, generate_overworld_map,
    BundledCampaign, BundledPack, CampaignQuality, CampaignSettings, LevelConfig,
    OverworldResult, PackStrings, Phase2Result, QualityBreakdown, StoreRaw, TileDefSlim,
};
use scapegrace::mapgen::{generate_map_with_options, MapGenResult};

struct AppState {
    pack: BundledPack,
    output_path: String,
}

fn save_pack(state: &AppState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&state.pack)
        .map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&state.output_path, json)
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

fn parse_output_path() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--output" || args[i] == "-o") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "campaigns.json".to_string()
}

// ── Request/Response types ──

#[derive(Deserialize)]
struct GenerateMapRequest {
    tile_defs: Vec<TileDefSlim>,
    palette: Vec<String>,
}

#[derive(Deserialize)]
struct OverworldRequest {
    theme: Option<String>,
}

#[derive(Deserialize)]
struct LevelDesignRequest {
    campaign_name: String,
    campaign_desc: String,
    level_config: LevelConfig,
    theme: Option<String>,
}

#[derive(Deserialize)]
struct DescriptionRequest {
    context: String,
    target: String,
}

#[derive(Serialize)]
struct TextResponse {
    text: String,
}

#[derive(Deserialize)]
struct ImageRequest {
    prompt: String,
    #[serde(default = "default_512")]
    width: u32,
    #[serde(default = "default_512")]
    height: u32,
    #[serde(default)]
    aspect_ratio: Option<String>,
}

fn default_512() -> u32 { 512 }

#[derive(Serialize)]
struct ImageResponse {
    image_base64: String,
}

// ── Handlers ──

async fn index_handler() -> impl IntoResponse {
    // Try Vite-built frontend first, fall back to legacy monolith
    let index_path = if std::path::Path::new("./static/level_builder/dist/index.html").exists() {
        "./static/level_builder/dist/index.html"
    } else {
        "./static/level_builder.html"
    };
    match std::fs::read_to_string(index_path) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, format!("Could not read {}: {}", index_path, e)).into_response(),
    }
}

async fn get_pack(State(state): State<Arc<Mutex<AppState>>>) -> Json<serde_json::Value> {
    let st = state.lock().unwrap();
    Json(serde_json::to_value(&st.pack).unwrap())
}

async fn put_pack(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(pack): Json<BundledPack>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut st = state.lock().unwrap();
    st.pack = pack;
    save_pack(&st).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::OK)
}

async fn create_campaign(
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut st = state.lock().unwrap();
    let campaign = BundledCampaign {
        id: uuid::Uuid::new_v4().to_string(),
        overworld: OverworldResult {
            name: "New Campaign".into(),
            font: None,
            description_font: None,
            label_font: None,
            description: String::new(),
            bg_color: None,
            text_color: None,
            levels: vec![],
            store: Some(StoreRaw {
                healing_potions: None,
                speed_potions: None,
                bombs: None,
            }),
            boss_level: None,
            connections: None,
            node_positions: None,
            bg_image: None,
            bg_gradient: None,
            bg_mode: None,
            terrain_seed: None,
            bg_prompt: None,
            ow_region_offsets: None, builder_regions: None,
            one_way_connections: None,
            fork_chambers: None,
            rooms: None,
            hallway_waypoints: None,
            start_room_size: None,
            store_room_size: None,
        },
        designs: vec![],
        quality: CampaignQuality {
            score: 0,
            breakdown: QualityBreakdown {
                completeness: 0,
                tile_variety: 0,
                monster_variety: 0,
                color_quality: 0,
                name_quality: 0,
                description_quality: 0,
                mode_validity: 0,
                budget_distribution: 0,
                theme_coherence: 0,
            },
        },
        settings: CampaignSettings::default(),
        monster_templates: None,
        prebuilt_overworld_map: None,
    };
    let val = serde_json::to_value(&campaign).unwrap();
    st.pack.campaigns.push(campaign);
    save_pack(&st).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(val))
}

async fn delete_campaign(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut st = state.lock().unwrap();
    let before = st.pack.campaigns.len();
    st.pack.campaigns.retain(|c| c.id != id);
    if st.pack.campaigns.len() == before {
        return Err((StatusCode::NOT_FOUND, "Campaign not found".into()));
    }
    save_pack(&st).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::OK)
}

async fn get_fonts() -> Json<Vec<&'static str>> {
    Json(FONT_NAMES.to_vec())
}

async fn get_google_fonts() -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(|| {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://fonts.google.com/metadata/fonts")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .map_err(|e| format!("Failed to fetch Google Fonts: {}", e))?;
        let text = resp.text().map_err(|e| format!("Read error: {}", e))?;
        // The metadata endpoint returns JSON with a )]}' prefix
        let json_str = text.strip_prefix(")]}'").unwrap_or(&text).trim();
        let data: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Parse error: {}", e))?;
        let families: Vec<String> = data["familyMetadataList"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|f| f["family"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        Ok::<_, String>(families)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task error: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}

async fn generate_map(
    Json(req): Json<GenerateMapRequest>,
) -> Result<Json<MapGenResult>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let full_defs = expand_tile_defs(&req.tile_defs, &req.palette);
        generate_map_with_options(&full_defs, false)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?;
    Ok(Json(result))
}

async fn generate_overworld(
    Json(req): Json<OverworldRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let prompt = build_overworld_prompt_themed(req.theme.as_deref());
        let client = reqwest::blocking::Client::new();
        let api_key = llm_api_key();
        let model = llm_model();
        let content = call_llm_streaming::<fn()>(&client, &api_key, &model, &prompt, None)
            .map_err(|e| format!("LLM error: {}", e))?;
        let ow: OverworldResult = serde_json::from_str(&content)
            .map_err(|e| format!("Parse error: {}\nRaw: {}", e, &content[..content.len().min(500)]))?;
        Ok::<_, String>(ow)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(&result).unwrap()))
}

async fn generate_level_design(
    Json(req): Json<LevelDesignRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let prompt = build_single_level_design_prompt_themed(
            &req.campaign_name,
            &req.campaign_desc,
            &req.level_config,
            req.theme.as_deref(),
        );
        let client = reqwest::blocking::Client::new();
        let api_key = llm_api_key();
        let model = llm_model();
        let content = call_llm_streaming::<fn()>(&client, &api_key, &model, &prompt, None)
            .map_err(|e| format!("LLM error: {}", e))?;
        let design: Phase2Result = serde_json::from_str(&content)
            .map_err(|e| format!("Parse error: {}\nRaw: {}", e, &content[..content.len().min(500)]))?;
        Ok::<_, String>(design)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(&result).unwrap()))
}

async fn generate_description(
    Json(req): Json<DescriptionRequest>,
) -> Result<Json<TextResponse>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let prompt = format!(
            "Generate a short, atmospheric description (max 10 words) for: {}\nContext: {}\nReturn ONLY the text, no JSON.",
            req.target, req.context
        );
        let client = reqwest::blocking::Client::new();
        let api_key = llm_api_key();
        let model = llm_model();
        let content = call_llm_streaming::<fn()>(&client, &api_key, &model, &prompt, None)
            .map_err(|e| format!("LLM error: {}", e))?;
        Ok::<_, String>(content)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(TextResponse { text: result }))
}

fn image_model() -> String {
    std::env::var("IMAGE_MODEL")
        .unwrap_or_else(|_| "google/gemini-2.5-flash-image".into())
}

async fn generate_image(
    Json(req): Json<ImageRequest>,
) -> Result<Json<ImageResponse>, (StatusCode, String)> {
    // Try OpenRouter first (uses chat completions with modalities: ["image", "text"])
    let openrouter_key = std::env::var("OPENROUTER_API_KEY").or_else(|_| std::env::var("LLM_API_KEY")).ok();

    if let Some(api_key) = openrouter_key {
        let model = image_model();
        let result = tokio::task::spawn_blocking(move || {
            let client = reqwest::blocking::Client::new();
            let resp = client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": req.prompt}],
                    "modalities": ["image", "text"],
                    "image_config": {
                        "aspect_ratio": req.aspect_ratio.as_deref().unwrap_or("16:9"),
                    },
                }))
                .timeout(std::time::Duration::from_secs(120))
                .send()
                .map_err(|e| format!("OpenRouter image error: {}", e))?;
            let body: serde_json::Value = resp.json()
                .map_err(|e| format!("OpenRouter parse error: {}", e))?;

            // Extract base64 from data URL in images array
            if let Some(images) = body["choices"][0]["message"]["images"].as_array() {
                if let Some(url) = images[0]["image_url"]["url"].as_str() {
                    // Strip "data:image/png;base64," prefix
                    let b64 = if let Some(idx) = url.find("base64,") {
                        &url[idx + 7..]
                    } else {
                        url
                    };
                    return Ok(b64.to_string());
                }
            }

            // Some models return image in content as data URL
            if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                if content.contains("base64,") {
                    if let Some(idx) = content.find("base64,") {
                        let b64 = &content[idx + 7..];
                        // Trim any trailing non-base64 chars
                        let b64 = b64.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '+' && c != '/' && c != '=');
                        return Ok(b64.to_string());
                    }
                }
            }

            Err(format!("No image in OpenRouter response: {}", &body.to_string()[..body.to_string().len().min(500)]))
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        return Ok(Json(ImageResponse { image_base64: result }));
    }

    // Fall back to dedicated image API (OpenAI images format)
    let api_url = std::env::var("IMAGE_API_URL")
        .map_err(|_| (StatusCode::BAD_REQUEST, "No OPENROUTER_API_KEY or IMAGE_API_URL set. Configure one for image generation.".into()))?;
    let api_key = std::env::var("IMAGE_API_KEY")
        .map_err(|_| (StatusCode::BAD_REQUEST, "IMAGE_API_KEY env var not set".into()))?;

    let size = format!("{}x{}", req.width, req.height);
    let result = tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "prompt": req.prompt,
                "n": 1,
                "size": size,
                "response_format": "b64_json",
            }))
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .map_err(|e| format!("Image API error: {}", e))?;
        let body: serde_json::Value = resp.json()
            .map_err(|e| format!("Image API parse error: {}", e))?;
        let b64 = body["data"][0]["b64_json"]
            .as_str()
            .ok_or_else(|| "No image data in response".to_string())?
            .to_string();
        Ok::<_, String>(b64)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {}", e)))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(ImageResponse { image_base64: result }))
}

// ── Overworld Map ──

async fn get_overworld_map(
    State(state): State<Arc<Mutex<AppState>>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let campaign_id = params.get("id").cloned().unwrap_or_default();
    let st = state.lock().unwrap();
    let campaign = st.pack.campaigns.iter()
        .find(|c| c.id == campaign_id)
        .ok_or((StatusCode::NOT_FOUND, "Campaign not found".into()))?;

    // Build a temporary Overworld to pass to the generator
    let ow = build_overworld_inner_for_preview(campaign)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let map = generate_overworld_map(campaign, &ow)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate map".into()))?;

    // Serialize just the tile grid and tile defs (not images)
    let tile_defs: std::collections::HashMap<String, serde_json::Value> = map.tile_defs.iter()
        .map(|(k, v)| (k.clone(), serde_json::json!({
            "name": v.name, "color": v.color, "walkable": v.walkable
        })))
        .collect();

    let regions: Vec<serde_json::Value> = map.level_regions.iter()
        .map(|r| serde_json::json!({
            "node_idx": r.node_idx, "ox": r.ox, "oy": r.oy, "w": r.w, "h": r.h,
            "entry_pos": r.entry_pos, "exit_pos": r.exit_pos,
        }))
        .collect();

    Ok(Json(serde_json::json!({
        "width": map.width,
        "height": map.height,
        "tiles": map.tiles,
        "tile_defs": tile_defs,
        "regions": regions,
        "player_pos": map.player_pos,
    })))
}

// ── Main ──

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let output_path = parse_output_path();

    let pack = if let Ok(content) = std::fs::read_to_string(&output_path) {
        serde_json::from_str::<BundledPack>(&content).unwrap_or_else(|_| BundledPack {
            theme: None,
            campaigns: vec![],
            strings: PackStrings::default(),
            item_sprites: Default::default(),
            item_names: Default::default(),
            item_descriptions: Default::default(),
        })
    } else {
        BundledPack {
            theme: None,
            campaigns: vec![],
            strings: PackStrings::default(),
            item_sprites: Default::default(),
            item_names: Default::default(),
            item_descriptions: Default::default(),
        }
    };

    let state = Arc::new(Mutex::new(AppState { pack, output_path }));

    let app = Router::new()
        .route("/", get(index_handler))
        .nest_service("/assets", ServeDir::new("./static/level_builder/dist/assets"))
        .route("/api/pack", get(get_pack))
        .route("/api/pack", put(put_pack))
        .route("/api/campaigns", post(create_campaign))
        .route("/api/campaigns/{id}", delete(delete_campaign))
        .route("/api/fonts", get(get_fonts))
        .route("/api/google-fonts", get(get_google_fonts))
        .route("/api/generate-map", post(generate_map))
        .route("/api/generate/overworld", post(generate_overworld))
        .route("/api/generate/level-design", post(generate_level_design))
        .route("/api/generate/description", post(generate_description))
        .route("/api/generate-image", post(generate_image))
        .route("/api/overworld-map", get(get_overworld_map))
        .fallback(get(index_handler))
        .layer(axum::extract::DefaultBodyLimit::max(200 * 1024 * 1024)) // 200MB
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    println!("Level Builder running at http://localhost:3001");
    axum::serve(listener, app).await.unwrap();
}
