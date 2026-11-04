mod knowledge;

use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use knowledge::KnowledgeBase;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message as LettreMessage, Tokio1Executor};
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
    project_name: String,
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

#[derive(Deserialize, Serialize, Debug, Clone)]
struct LeadRequest {
    name: String,
    email: String,
    message: String,
    #[serde(default)]
    phone: Option<String>,
}

#[derive(Serialize)]
struct LeadResponse {
    success: bool,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set in .env");
    let model_name = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string());
    let project_name = env::var("PROJECT_NAME").unwrap_or_else(|_| "RadialCode".to_string());
    let knowledge_dir = env::var("KNOWLEDGE_DIR").unwrap_or_else(|_| "data/pages".to_string());

    println!("Initializing Universal AI Agent...");
    println!("Project: {}", project_name);
    println!("AI Model: {}", model_name);
    println!("Knowledge Directory: {}", knowledge_dir);

    // Check SMTP status on startup
    if let Ok(smtp_email) = env::var("SMTP_EMAIL") {
        if !smtp_email.trim().is_empty() {
            println!("Email Dispatcher: Configured via Gmail SMTP ({})", smtp_email.trim());
        }
    } else {
        println!("Email Dispatcher: Not configured yet (leads will be saved to disk)");
    }

    let knowledge_path = std::path::Path::new(&knowledge_dir);
    let knowledge_base = Arc::new(KnowledgeBase::load_from_dir(knowledge_path));

    let state = AppState {
        gemini_client: gemini::Client::new(&api_key),
        model_name,
        project_name,
        knowledge_base,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/chat", post(chat_handler))
        .route("/lead", post(lead_handler))
        .layer(cors)
        .with_state(state);

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("AI Agent running on http://127.0.0.1:{}", port);
    println!("Ready for queries!");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    let project_name = env::var("PROJECT_NAME").unwrap_or_else(|_| "RadialCode".to_string());
    let smtp_configured = env::var("SMTP_EMAIL").map(|s| !s.trim().is_empty()).unwrap_or(false);
    Json(serde_json::json!({
        "status": "ok",
        "project": project_name,
        "service": "universal-knowledge-agent",
        "email_service": if smtp_configured { "active" } else { "unconfigured" }
    }))
}

async fn send_lead_email(
    project_name: String,
    name: String,
    email: String,
    phone: Option<String>,
    message: String,
) -> Result<(), String> {
    let smtp_email = match env::var("SMTP_EMAIL") {
        Ok(val) if !val.trim().is_empty() => val.trim().to_string(),
        _ => {
            println!("ℹ️ SMTP_EMAIL not configured in .env. Skipping email dispatch.");
            return Ok(());
        }
    };

    let smtp_password = match env::var("SMTP_PASSWORD") {
        Ok(val) if !val.trim().is_empty() => val.trim().replace(' ', "").to_string(),
        _ => {
            println!("ℹ️ SMTP_PASSWORD not configured in .env. Skipping email dispatch.");
            return Ok(());
        }
    };

    let receiver_email = env::var("LEAD_RECEIVER_EMAIL")
        .unwrap_or_else(|_| smtp_email.clone());

    let subject = format!("🔔 New Lead: {} — {} Inquiry", name, project_name);

    let phone_html = match &phone {
        Some(p) if !p.trim().is_empty() => format!(
            r#"<tr style="border-bottom: 1px solid #f1f5f9;">
              <td style="padding: 10px 14px; font-weight: 600; color: #475569; width: 130px;">Phone / WhatsApp:</td>
              <td style="padding: 10px 14px; color: #0f172a;"><a href="tel:{phone}" style="color: #0284c7; text-decoration: none;">{phone}</a></td>
            </tr>"#,
            phone = p.trim()
        ),
        _ => String::new(),
    };

    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>New Lead Captured</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #f1f5f9; margin: 0; padding: 28px 16px;">
  <div style="max-width: 580px; margin: 0 auto; background: #ffffff; border-radius: 14px; overflow: hidden; border: 1px solid #e2e8f0; box-shadow: 0 4px 12px rgba(0,0,0,0.06);">
    <!-- Header -->
    <div style="background: linear-gradient(135deg, #0ea5e9 0%, #4338ca 100%); padding: 24px 28px; color: #ffffff;">
      <h2 style="margin: 0; font-size: 21px; font-weight: 700; letter-spacing: -0.5px;">🔔 New Client Lead Captured!</h2>
      <p style="margin: 6px 0 0 0; font-size: 13px; opacity: 0.9;">Website Inquiry for {project_name}</p>
    </div>
    
    <!-- Content Table -->
    <div style="padding: 24px 28px;">
      <table style="width: 100%; border-collapse: collapse; font-size: 14px;">
        <tr style="border-bottom: 1px solid #f1f5f9;">
          <td style="padding: 10px 14px; font-weight: 600; color: #475569; width: 130px;">Client Name:</td>
          <td style="padding: 10px 14px; color: #0f172a; font-weight: 600;">{name}</td>
        </tr>
        <tr style="border-bottom: 1px solid #f1f5f9;">
          <td style="padding: 10px 14px; font-weight: 600; color: #475569;">Email Address:</td>
          <td style="padding: 10px 14px; color: #0f172a;"><a href="mailto:{email}" style="color: #0284c7; text-decoration: none; font-weight: 500;">{email}</a></td>
        </tr>
        {phone_html}
        <tr>
          <td style="padding: 10px 14px; font-weight: 600; color: #475569; vertical-align: top;">Requirement:</td>
          <td style="padding: 10px 14px; color: #0f172a; line-height: 1.6; white-space: pre-wrap;">{message}</td>
        </tr>
      </table>

      <!-- Quick Action Button -->
      <div style="margin-top: 28px; padding-top: 20px; border-top: 1px solid #e2e8f0; text-align: center;">
        <a href="mailto:{email}?subject=Regarding%20your%20inquiry%20with%20{project_name}" style="display: inline-block; background: #0284c7; color: #ffffff; text-decoration: none; padding: 11px 24px; font-size: 13px; font-weight: 600; border-radius: 8px; box-shadow: 0 2px 4px rgba(2,132,199,0.25);">
          ✉️ Reply to {name}
        </a>
      </div>
    </div>
    
    <!-- Footer -->
    <div style="background: #f8fafc; padding: 14px 28px; border-top: 1px solid #e2e8f0; font-size: 11px; color: #94a3b8; text-align: center;">
      Automated Lead Notification • {project_name} AI Agent
    </div>
  </div>
</body>
</html>"#,
        project_name = project_name,
        name = name,
        email = email,
        phone_html = phone_html,
        message = message
    );

    let from_header = format!("{} AI Agent <{}>", project_name, smtp_email);
    let to_header = receiver_email.clone();

    let email_msg = LettreMessage::builder()
        .from(from_header.parse().map_err(|e| format!("Invalid From address: {}", e))?)
        .to(to_header.parse().map_err(|e| format!("Invalid To address: {}", e))?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html_content)
        .map_err(|e| format!("Failed to build email: {}", e))?;

    let creds = Credentials::new(smtp_email, smtp_password);

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
        .map_err(|e| format!("Failed to configure SMTP relay: {}", e))?
        .credentials(creds)
        .build();

    mailer
        .send(email_msg)
        .await
        .map_err(|e| format!("SMTP Send Error: {}", e))?;

    println!("✅ Lead notification email successfully dispatched to {}", receiver_email);
    Ok(())
}

async fn lead_handler(
    State(state): State<AppState>,
    Json(payload): Json<LeadRequest>,
) -> Result<Json<LeadResponse>, (StatusCode, String)> {
    let name = payload.name.trim().to_string();
    let email = payload.email.trim().to_string();
    let message = payload.message.trim().to_string();
    let phone = payload.phone.clone();

    if name.is_empty() || email.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Name and email are required fields.".to_string(),
        ));
    }

    println!("\n=======================================================");
    println!("🔔 NEW BUSINESS LEAD RECEIVED FOR {}", state.project_name);
    println!("Name:    {}", name);
    println!("Email:   {}", email);
    if let Some(ref p) = phone {
        println!("Phone:   {}", p);
    }
    println!("Message: {}", message);
    println!("=======================================================\n");

    // 1. Persist lead to disk in JSON Lines format so it is never lost
    let _ = std::fs::create_dir_all("data");
    let lead_record = serde_json::json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "project": state.project_name,
        "name": name,
        "email": email,
        "phone": phone,
        "message": message,
    });

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/leads.jsonl")
    {
        use std::io::Write;
        let _ = writeln!(file, "{}", lead_record);
    }

    // 2. Dispatch email notification in background via Gmail SMTP
    let project_name = state.project_name.clone();
    let name_clone = name.clone();
    let email_clone = email.clone();
    let phone_clone = phone.clone();
    let message_clone = message.clone();

    tokio::spawn(async move {
        if let Err(err) = send_lead_email(
            project_name,
            name_clone,
            email_clone,
            phone_clone,
            message_clone,
        )
        .await
        {
            eprintln!("⚠️ Failed to send lead email: {}", err);
        }
    });

    Ok(Json(LeadResponse {
        success: true,
        message: format!(
            "Thank you, {}! Your inquiry has been sent directly to the {} team.",
            name, state.project_name
        ),
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

    // 1. Search markdown pages dynamically using TF-IDF style relevancy
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

    // 2. 100% Generic AI prompt: Zero hardcoded project-specific rules!
    let prompt = format!(
        "You are the official intelligent AI Assistant for {project_name}.\n\
        The user is asking: \"{question}\"\n\n\
        Use the official documents below to provide an EXACT, DIRECT, FACTUAL, and COMPLETE answer.\n\n\
        RULES:\n\
        1. Answer directly and concisely. Do NOT begin with conversational filler like 'Based on the official website', 'Based on the documents', 'According to...', or 'Here are the details'.\n\
        2. Answer the user's specific intent thoroughly using all details found in the documents.\n\
        3. If the user asks in Hindi or Hinglish, answer naturally in that same language.\n\
        4. Never make up facts or extrapolate outside the provided documents. If information is genuinely absent, clearly state so.\n\n\
        === OFFICIAL DOCUMENTS ===\n\
        {context}\n\
        === END DOCUMENTS ===\n\n\
        Direct Answer:",
        project_name = state.project_name,
        question = question,
        context = context
    );

    let agent = state.gemini_client.agent(&state.model_name).build();

    let raw_answer = agent
        .prompt(&prompt)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("AI Model error: {}", e)))?;

    // Post-processing: programmatically strip any conversational filler prefixes
    let mut answer = raw_answer.trim().to_string();
    let filler_prefixes = [
        "Based on the official website data,",
        "Based on the official website,",
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
