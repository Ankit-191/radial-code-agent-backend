mod knowledge;

use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use knowledge::{KnowledgeBase, PageDoc};
use rig::completion::Prompt;
use rig::providers::gemini;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    gemini_client: gemini::Client,
    model_name: String,
    knowledge_base: Arc<KnowledgeBase>,
}

#[derive(Deserialize)]
struct ChatRequest {
    question: String,
}

#[derive(Serialize)]
struct ChatResponse {
    answer: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set in .env");
    let model_name = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string());

    println!("Initializing RadialCode AI Agent with Local Markdown Knowledge Base...");
    println!("AI Model: {}", model_name);

    let knowledge_path = std::path::Path::new("data/pages");
    let knowledge_base = Arc::new(KnowledgeBase::load_from_dir(knowledge_path));

    let state = AppState {
        gemini_client: gemini::Client::new(&api_key),
        model_name,
        knowledge_base,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/chat", post(chat_handler))
        .layer(cors)
        .with_state(state);

    let port = 3001;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("RadialCode Agent running on http://127.0.0.1:{}", port);
    println!("Ready for queries!");

    axum::serve(listener, app).await?;
    Ok(())
}


async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "radialcode-knowledge-agent"
    }))
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let question = payload.question.trim();
    if question.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Question cannot be empty".to_string()));
    }

    // 1. Search markdown pages using query relevance
    let matching_pages = state.knowledge_base.search(question, 3);

    println!("Query: \"{}\" matched {} markdown pages:", question, matching_pages.len());
    for p in &matching_pages {
        println!(" - {}.md ({})", p.filename, p.title);
    }

    let mut context = String::new();
    for page in matching_pages {
        context.push_str(&format!(
            "--- DOCUMENT: {} (Title: {}) ---\n{}\n--- END DOCUMENT ---\n\n",
            page.filename, page.title, page.content
        ));
    }

    // 2. Strict AI prompt: direct answer, factual, no conversational filler
    let prompt = format!(
        "You are the official RadialCode AI Assistant.\n\
        The user is asking: \"{}\"\n\n\
        Use the official documents below to provide an EXACT, DIRECT, and COMPLETE answer.\n\n\
        RULES:\n\
        1. Answer directly and concisely. Do NOT begin with conversational filler like 'Based on the official website', 'Based on the provided documents', 'According to...', or 'Here are the details'.\n\
        2. If the user asks for team members or team names, list all the team members and their roles given in the team document.\n\
        3. If the user asks for courses, list all the specific courses provided by RadialCode from the courses document.\n\
        4. If the user asks for contact info, provide phone (+91 97157 11119), email (contact@radialcode.com), and location (Hisar, Haryana).\n\
        5. If the user asks in Hindi or Hinglish, answer in the same natural helpful language.\n\
        6. Do not make up information that is not in the documents.\n\n\
        === OFFICIAL DOCUMENTS ===\n\
        {}\n\
        === END DOCUMENTS ===\n\n\
        Direct Answer:",
        question, context
    );

    let agent = state.gemini_client.agent(&state.model_name).build();

    let raw_answer = agent
        .prompt(&prompt)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("AI Model error: {}", e)))?;

    // Post-processing: programmatically strip any remaining conversational fillers
    let mut answer = raw_answer.trim().to_string();
    let filler_prefixes = [
        "Based on the official website data,",
        "Based on the official website,",
        "Based on the official Radial Code website,",
        "Based on the official RadialCode website,",
        "Based on the official documents provided,",
        "Based on the provided documents,",
        "Based on the documents,",
        "Based on the website,",
        "According to the official website,",
        "According to the website,",
        "According to the provided documents,",
        "Here are the details:",
    ];

    for prefix in &filler_prefixes {
        if answer.to_lowercase().starts_with(&prefix.to_lowercase()) {
            answer = answer[prefix.len()..].trim().to_string();
            if let Some(first_char) = answer.chars().next() {
                answer = first_char.to_uppercase().to_string() + &answer[first_char.len_utf8()..];
            }
        }
    }

    Ok(Json(ChatResponse { answer }))
}
