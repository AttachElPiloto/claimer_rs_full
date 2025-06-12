use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use std::error::Error;

const DISCORD_WEBHOOK_URL: &str = "https://discord.com/api/webhooks/1371226128886530118/HaeJZ6Q3_K49kAOg9maXt8y6qOpGV9xgtA-YkMfUQALoKJc3TU9Iw12ND5eMtKo4uYRX";
const DISCORD_WEBHOOK_URL_2 : &str = "https://discord.com/api/webhooks/1369030910506303559/aTaTvt3MeGmZNcJNymbOHdtj3uBoD7wjd1h7glXtZkXwxsUnMxHdbyzskyoDIFF5oGZ0";

pub async fn send_webhook(message: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let payload = json!({
        "content": message
    });

    let response = client
        .post(DISCORD_WEBHOOK_URL)
        .json(&payload)
        .send()
        .await?;

    if response.status() != 204 {
        eprintln!("⚠️ Webhook Discord renvoyé status {}", response.status());
    }

    Ok(())
}

pub async fn notify_drop_window(
    name: &str,
    window_begin: &str,
    window_end: &str,
) -> Result<(), Box<dyn std::error::Error>> {

    let window_begin_ = window_begin.parse::<chrono::DateTime<Utc>>()?;
    let window_end_ = window_end.parse::<chrono::DateTime<Utc>>()?;
    let unix_timestamp_begin = window_begin_.timestamp_micros();
    let unix_timestamp_end = window_end_.timestamp_micros();

    let formatted_unix_begin = format!(
        "{}.{:06}",
        unix_timestamp_begin / 1_000_000,
        unix_timestamp_begin % 1_000_000
    );
    let formatted_unix_end = format!(
        "{}.{:06}",
        unix_timestamp_end / 1_000_000,
        unix_timestamp_end % 1_000_000
    );

    let duration_ms = (window_end_ - window_begin_).num_milliseconds();
    let minutes = duration_ms / 60_000;
    let seconds = (duration_ms % 60_000) / 1_000;
    let millis  =  duration_ms % 1_000;

    let embed = json!({
    "title": name,
    "description": format!(
        "{}\n`{}`\n→\n{}\n`{}`",
        window_begin_, formatted_unix_begin,
        window_end_,   formatted_unix_end
    ),
    "color": 7506394,
    "fields": [
        {
            "name": "Durée",
            "value": format!("{:02}m {:02}s {:03}ms", minutes, seconds, millis),
            "inline": true
        }
    ],
    "footer": { "text": "be careful" },
    "timestamp": Utc::now().to_rfc3339()
    });


    // 🔹 Envoi via reqwest
    let client = Client::new();
    let response = client
        .post(DISCORD_WEBHOOK_URL_2)
        .json(&json!({
            "content": "||drop incoming||",
            "embeds": [embed],
        }))
        .send()
        .await?;
    if response.status() != 204 {
        eprintln!("DROP WINDOWS PAS ENVOYE");
        let _ = client
            .post(DISCORD_WEBHOOK_URL_2)
            .json(&json!({
                "content": format!("⚠️ Erreur d'envoi du webhook : {}", response.status()),
            }))
            .send()
            .await?;
    }

    Ok(())
}





