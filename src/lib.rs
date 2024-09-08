extern crate console_error_panic_hook;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::{stream, StreamExt};
use hmac::{Hmac, Mac};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::panic;
use worker::*;

#[derive(Debug, Deserialize, Serialize)]
struct Entry {
    id: u64,
    title: String,
    url: String,
    content: String,
    feed_id: u64,
}

#[derive(Debug, Deserialize)]
struct NewEntriesRequest {
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct UpdateRequest {
    content: String,
}

async fn update_entry(
    base_url: &str,
    username: &str,
    password: &str,
    id: u64,
    content: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let auth = format!(
        "Basic {}",
        STANDARD.encode(format!("{}:{}", username, password))
    );

    let url = format!("{}/v1/entries/{}", base_url, id);
    let update_request = UpdateRequest {
        content: content.to_string(),
    };
    console_log!("created update_request");

    client
        .put(&url)
        .header(AUTHORIZATION, auth)
        .header(CONTENT_TYPE, "application/json")
        .json(&update_request)
        .send()
        .await?
        .error_for_status()?;
    console_log!("updated entry in miniflux");

    Ok(())
}

#[derive(Serialize)]
struct SummarizeRequest {
    input_text: String,
    max_length: u64,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct SummarizeResponse {
    summary: String,
}

async fn request_ai_summarization(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: String,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    console_log!("request_ai_summarization");
    let client = reqwest::Client::new();
    let request_body = SummarizeRequest {
        input_text: input,
        max_length: 512,
    };

    let response = client
        .post(format!("{}/run/{}", base_url, model))
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        console_log!("request_ai_summarization success");
        let summarize_response: SummarizeResponse = response.json().await?;
        Ok(summarize_response.summary)
    } else {
        let error_message = response.text().await?;
        console_log!("request_ai_summarization error: {}", error_message);
        Err(format!("Error: {:?}", error_message).into())
    }
}

#[derive(Debug)]
struct Miniflux {
    url: String,
    username: String,
    password: String,
    webhook_secret: String,
}

#[derive(Debug)]
struct CloudflareAi {
    url: String,
    token: String,
    model: String,
}

#[derive(Debug)]
struct Config {
    miniflux: Miniflux,
    cloudflare_ai: CloudflareAi,
}

async fn generate_and_update_entry(
    config: &Config,
    entry: Entry,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    console_log!("entry id: {}", entry.id);
    console_log!("entry title: {}", entry.title);
    console_log!("entry url: {}", entry.url);
    if entry.content.trim().is_empty() || entry.content.len() < 500 {
        console_log!("skipping entry due to empty content or short content length");
        return Ok(());
    }
    let input = format!(
        "Title: {}\nURL: {}\nContent: {}",
        &entry.title, &entry.url, &entry.content
    );

    // Generate summary
    if let Ok(summary) = request_ai_summarization(
        &config.cloudflare_ai.url,
        &config.cloudflare_ai.token,
        &config.cloudflare_ai.model,
        input,
    )
    .await
    {
        if !summary.trim().is_empty() {
            console_log!("Summary: {}", summary);
            let updated_content = format!(
                "<div class=\"ai-summary\"><h4>✨ AI Summary</h4>{}</div><hr><br />{}",
                markdown::to_html(&summary),
                &entry.content
            );

            // Update the entry
            update_entry(
                &config.miniflux.url,
                &config.miniflux.username,
                &config.miniflux.password,
                entry.id,
                &updated_content,
            )
            .await?;
        }
    }

    console_log!("processed entry: {}", entry.id);
    Ok(())
}

fn handle_options() -> Result<Response> {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "POST, OPTIONS")?;
    headers.set(
        "Access-Control-Allow-Headers",
        "Content-Type, X-Miniflux-Signature",
    )?;
    Response::ok("").map(|resp| resp.with_headers(headers))
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_log!("scheduled");
    let config = &Config {
        cloudflare_ai: CloudflareAi {
            url: env.secret("CF_AI_URL").unwrap().to_string(),
            token: env.secret("CF_AI_TOKEN").unwrap().to_string(),
            model: env.var("CF_AI_MODEL").unwrap().to_string(),
        },
        miniflux: Miniflux {
            url: env.secret("MINIFLUX_URL").unwrap().to_string(),
            username: env.secret("MINIFLUX_USERNAME").unwrap().to_string(),
            password: env.secret("MINIFLUX_PASSWORD").unwrap().to_string(),
            webhook_secret: env.secret("MINIFLUX_WEBHOOK_SECRET").unwrap().to_string(),
        },
    };
    console_log!("config");

    let kv = env.kv("entries").unwrap();

    // List all keys with the "entry:" prefix
    let keys = kv
        .list()
        .prefix("entry:".to_string())
        .execute()
        .await
        .unwrap();

    let max_concurrent_tasks = 5;
    // Create a stream to process tasks with concurrency limit
    let _: Vec<_> = stream::iter(keys.keys)
        .map(|key| {
            let config = &config;
            let kv = kv.clone();
            async move {
                // Retrieve the entry
                if let Ok(Some(entry)) = kv.get(&key.name).json::<Entry>().await {
                    console_log!("Processing entry: {}", key.name);

                    // Process the entry (call AI API, etc.)
                    match generate_and_update_entry(config, entry).await {
                        Ok(_) => {
                            // If processing was successful, delete the entry from KV
                            kv.delete(&key.name).await.unwrap();
                            console_log!("Processed and removed entry: {}", key.name);
                        }
                        Err(e) => {
                            console_error!("Error processing entry {}: {:?}", key.name, e);
                            // Optionally, you could implement retry logic here
                        }
                    }
                }
            }
        })
        .buffer_unordered(max_concurrent_tasks)
        .collect()
        .await;
}

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log!("fetch");
    // Check if it's an OPTIONS request and handle it
    if req.method() == Method::Options {
        return handle_options();
    }
    console_log!("not options");

    // Only proceed with POST requests
    if req.method() != Method::Post {
        return Response::error("Method not allowed", 405);
    }
    console_log!("is post");

    let config = &Config {
        cloudflare_ai: CloudflareAi {
            url: env.secret("CF_AI_URL").unwrap().to_string(),
            token: env.secret("CF_AI_TOKEN").unwrap().to_string(),
            model: env.var("CF_AI_MODEL").unwrap().to_string(),
        },
        miniflux: Miniflux {
            url: env.secret("MINIFLUX_URL").unwrap().to_string(),
            username: env.secret("MINIFLUX_USERNAME").unwrap().to_string(),
            password: env.secret("MINIFLUX_PASSWORD").unwrap().to_string(),
            webhook_secret: env.secret("MINIFLUX_WEBHOOK_SECRET").unwrap().to_string(),
        },
    };
    console_log!("config");

    let signature = req
        .headers()
        .get("x-miniflux-signature")
        .map_err(|_err| {
            Error::RustError("Missing signature header in webhook request".to_string())
        })?
        .ok_or_else(|| {
            Error::RustError("Missing signature header in webhook request".to_string())
        })?;
    console_log!("signature");
    let payload = match req.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => return Response::error(format!("Failed to read payload: {}", err), 400),
    };
    console_log!("payload");

    let mut mac = Hmac::<Sha256>::new_from_slice(config.miniflux.webhook_secret.as_bytes())
        .map_err(|_| Error::RustError("HMAC key error".to_string()))?;
    console_log!("mac");

    mac.update(&payload);
    let hmac = hex::encode(mac.finalize().into_bytes());
    console_log!("hmac");

    if hmac != signature {
        return Response::error("Incorrect webhook request signature", 403);
    }

    // convert body to json
    let body: NewEntriesRequest = serde_json::from_slice::<NewEntriesRequest>(&payload)
        .map_err(|_err| Error::RustError("Failed to parse webhook json body".to_string()))?;
    console_log!("body");

    let kv = env.kv("entries")?;

    let max_concurrent_tasks = 5;

    // Create a stream to process tasks with concurrency limit
    let _: Vec<_> = stream::iter(body.entries)
        .map(|entry| {
            let key = format!("entry:{}", entry.id);
            let kv = kv.clone();
            console_log!("putting KV key {}", key);
            async move { kv.put(&key, entry)?.execute().await }
        })
        .buffer_unordered(max_concurrent_tasks)
        .collect()
        .await;
    Response::ok("Webhook request processed")
}
