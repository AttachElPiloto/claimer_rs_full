use chrono::Utc;
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde_json::json;
use serde::Deserialize;
use rand_chacha::ChaCha12Rng;
use rand::SeedableRng;
use std::collections::HashMap;
use crate::utilities::sql_management::UsernameResult;


const MOJANG_POST_URL: &str = "https://api.mojang.com/profiles/minecraft";

const AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_2_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.1 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.6167.85 Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Linux; Android 14; Pixel 8 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.6261.112 Mobile Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.2365.80",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.6167.160 Safari/537.36 OPR/108.0.0.0",
];

const PATH: &[&str] = &[
         "/profiles/minecraft",
        "/profiles/minecraft/",
        "/profiles/minecraft/.",
        "/profiles/minecraft?foo=bar",
        "/profiles/%2e/minecraft",
        "/%2e/profiles/minecraft",
        "/profiles/./minecraft",
        "/profiles/minecraft?debug=true",
        "/profiles/minecraft?cb=123456",
        "/profiles/minecraft?redirect=/admin",
        "/profiles/minecraft?_=timestamp",
        "//profiles/minecraft",
        "/profiles/minecraft\\",
    ];



#[derive(Debug, Deserialize,Clone)]
struct MojangResponse {
    id: String,
    name: String,
}



pub async fn fetch_batch(
    client: &Client,
    usernames: &[String],
) -> Result<(bool, Vec<UsernameResult>,usize), Box<dyn std::error::Error + Send + Sync>>{

    let user_agent = AGENTS.choose(&mut rand::rng()).unwrap();
    let mut rng = ChaCha12Rng::from_rng(&mut rand::rng());
    let url1 = format!("https://api.minecraftservices.com{}", PATH.choose(&mut rng).unwrap());
    let url2 = format!("https://api.mojang.com{}", PATH.choose(&mut rng).unwrap());
    let url = if rand::random() {
        url1
    } else {
        url2
    };


    let body = json!(usernames);
    match client
        .post(url)
        .header("User-Agent", *user_agent)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status() == 200 {
                let now = Utc::now();
                let result = resp.json::<Vec<MojangResponse>>().await.unwrap_or_default();
                let n = result.len();
                let uuid_map = result
                    .into_iter()
                    .map(|r| (r.name.to_lowercase(), r.id))
                    .collect::<HashMap<_, _>>();
    
                let mapped = usernames
                    .iter()
                    .map(|name| UsernameResult {
                        username: name.clone(),
                        uuid: uuid_map.get(&name.to_lowercase()).cloned(),
                        last_seen : now.to_string(),
                    })
                    .collect();
                if n!=0{
                    return Ok((true, mapped,200)); // succès
                }
                else {
                    return Ok((false, vec![], 699)); // pas de résultats
                }
            } else {
                if resp.status() == 429 {
                    return Ok((false, vec![], 429)); // trop de requêtes
                }
                if resp.status() == 403 {
                    return Ok((false, vec![], 403)); // accès interdit
                }
                if resp.status() == 400{
                    println!("ERREUR USERNAME : {:?}", usernames);
                }
                else 
                {
                    println!("ERREUR MOJANG : {:?}", resp.status());
                }
                return Ok((false, vec![],699)); // autre statut
            }
        }
        Err(_) => {
        	// println!("{:?}",x);
            return Ok((false, vec![],699)); // erreur réseau
        }
    }
}
