use ahash::RandomState;
use chrono::prelude::*;
use chrono_tz::Europe::Paris;
use dashmap::DashMap;
use rand::seq::IndexedRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use reqwest::{Client, Proxy};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Semaphore};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{timeout, Duration};
use rayon::prelude::*;
use claimer_rs_full::utilities::sql_management::{init_hashmap_from_txt, update_batch_status, BATCH_SIZE};
use claimer_rs_full::utilities::requests::fetch_batch;
use claimer_rs_full::utilities::log_and_errors::send_webhook;


const NB_THREADS: usize = 7500;

pub async fn load_proxies(path: &str) -> Vec<String> {
    let proxy_file = Path::new(path);
    if !proxy_file.exists() {
        println!("❗️ Fichier de proxy introuvable : {}", path);
        return vec![];
    }

    fs::read_to_string(proxy_file)
        .unwrap()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}


pub async fn process_batches(proxies: Vec<String>) {
    let mut debut_programme = Utc::now();
    let map_usernames = Arc::new(init_hashmap_from_txt("./names/3c.txt").expect("Failed to initialize map_usernames"));
    let map_windows : Arc<DashMap<String, (String, String), RandomState>> = Arc::new(DashMap::with_capacity_and_hasher_and_shard_amount(70_000, RandomState::new(), 128));
    let semaphore = Arc::new(Semaphore::new(150000));
    let counter_200  = Arc::new(AtomicUsize::new(0));
    let error_counter  = Arc::new(AtomicUsize::new(0));
    let counter_429 = Arc::new(AtomicUsize::new(0));
    let counter_403 = Arc::new(AtomicUsize::new(0));
    let usernames = Arc::new(
    map_usernames.iter().map(|e| e.key().clone()).collect::<Vec<_>>()
    );
    let total_batches = (usernames.len() + BATCH_SIZE - 1) / BATCH_SIZE;
    // print usernames and dashmap to be sure they are loaded
   

    let clients: Vec<Client> = proxies
    .par_iter()
    .take(10000)
    .filter_map(|proxy_url| {
        let proxy = Proxy::all(proxy_url).ok()?;
        let client = Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(3))
            .build()
            .ok()?;
        Some(client)
    })
    .collect();
    let clients = Arc::new(clients);

    {
        tokio::spawn({
            // on capture les compteurs & constantes
            let counter_200 = counter_200.clone();
            let counter_429 = counter_429.clone();
            let counter_403 = counter_403.clone();
            let error_counter = error_counter.clone();
        
            async move {
                // point de départ et état précédent
                let mut last_instant     = Utc::now();
        
                loop {
                    tokio::time::sleep(Duration::from_secs(60)).await;          // ← 1 minute
        
                    // valeurs courantes
                    let now      = Utc::now();
                    let count_200 = counter_200.load(Ordering::Relaxed);
                    let count_429 = counter_429.load(Ordering::Relaxed);
                    let count_403 = counter_403.load(Ordering::Relaxed);
                    let total_requests = count_200 + count_429 + count_403;
                    let errors   = error_counter.load(Ordering::Relaxed);
        
                    // delta depuis le précédent rapport        
                    // durée écoulée depuis le précédent rapport (≈ 60 s)
                    let duration = now.signed_duration_since(last_instant).num_milliseconds() as f64 / 1000.0;
                    let rpm      = if duration > 0.0 {
                        count_200 as f64 / duration
                    } else { 0.0 };


                    let uptime_secs = now
                    .signed_duration_since(debut_programme)
                    .num_seconds();

                    // Décompose `uptime_secs` en jours, heures, minutes, secondes
                    let days    = uptime_secs / 86_400;
                    let hours   = (uptime_secs % 86_400) / 3_600;
                    let minutes = (uptime_secs % 3_600) / 60;
                    let seconds =  uptime_secs % 60;
        
                    // taux cumulés (depuis le lancement)
        
                    // ───── affichage & webhook ─────
                    println!(
                        "\n | Checkpoint {:.0}s |
                         \n | 200 : {} | {} %
                         \n | 429 : {} | {} %
                         \n | 403 : {} | {} %
                         \n Erreurs de batch : {} ❌ {} %
                         \n Req/s : {:.1} |
                         \n Uptime : {} jours, {} heures, {} minutes, {} secondes",
                        duration,
                        count_200,
                        if total_requests > 0 {
                            (count_200 as f64 / total_requests as f64 * 100.0).round()
                        } else { 0.0 },
                        count_429,
                        if total_requests > 0 {
                            (count_429 as f64 / total_requests as f64 * 100.0).round()
                        } else { 0.0 },
                        count_403,
                        if total_requests > 0 {
                            (count_403 as f64 / total_requests as f64 * 100.0).round()
                        } else { 0.0 },
                        errors,
                        if total_requests > 0 {
                            errors as f64 / total_requests as f64 * 100.0
                        } else { 0.0 },
                        rpm,
                        days,
                        hours,
                        minutes,
                        seconds,
                    );
                    // Print all usernames that have not an UUID
                    
        
                    

                    let _ = send_webhook(&format!(
                    "**Checkpoint** `{:.0}s`\
                    \n\
                    • 200  : `{}` ({}%)\n\
                    • 429  : `{}` ({}%)\n\
                    • 403  : `{}` ({}%)\n\
                    • Err  : `{}` ({}%)\n\
                    • RPS  : `{:.1}`\n\
                    • UPT  : `{}D {:02}H {:02}m {:02}s`",
                    duration,
                    count_200,
                    if total_requests > 0 {
                        (count_200 as f64 / total_requests as f64 * 100.0).round()
                    } else { 0.0 },
                    count_429,
                    if total_requests > 0 {
                        (count_429 as f64 / total_requests as f64 * 100.0).round()
                    } else { 0.0 },
                    count_403,
                    if total_requests > 0 {
                        (count_403 as f64 / total_requests as f64 * 100.0).round()
                    } else { 0.0 },
                    errors,
                    if total_requests > 0 {
                        errors as f64 / total_requests as f64 * 100.0
                    } else { 0.0 },
                    rpm,
                    days,
                    hours,
                    minutes,
                    seconds
                ))
                .await;

                // reset des compteurs
                counter_200.store(0, Ordering::Relaxed);
                counter_429.store(0, Ordering::Relaxed);
                counter_403.store(0, Ordering::Relaxed);
                error_counter.store(0, Ordering::Relaxed);
                if days > 4 {
                    debut_programme = Utc::now(); // reset le compteur de temps si > 4 jours
                }
                // on prépare le prochain tour
                last_instant = now;
                }
            }
        });
    }

    let ratio = usernames.len() as f64 / (NB_THREADS * BATCH_SIZE) as f64;
    let max_loop = ratio.ceil() as usize;
    for batch in 0..NB_THREADS {
        let semaphore = semaphore.clone();
        let clients = clients.clone();
        let counter_200 = counter_200.clone();
        let counter_429 = counter_429.clone();
        let counter_403 = counter_403.clone();
        let error_counter = error_counter.clone();
        let map_usernames = map_usernames.clone();
        let map_windows = map_windows.clone();
        let mut k = 0;
        let mut rng = ChaCha12Rng::from_os_rng();
        let usernames_clone = usernames.clone();
        tokio::spawn(async move {
            loop {
                let batch_usernames: Vec<String> = usernames_clone.iter().cycle().skip((batch * BATCH_SIZE + k*NB_THREADS*BATCH_SIZE) % usernames_clone.len()).take(BATCH_SIZE).cloned().collect();
                // select random client
                let mut error : bool = true;
                for _retries in 1..=30{
                    let client = clients.choose(&mut rng).expect("No clients available").clone();
                    let _permit = semaphore.acquire().await.expect("Semaphore closed unexpectedly");
                    match timeout(Duration::from_secs(5), fetch_batch(&client, &batch_usernames)).await {
                        Ok(Ok((success, results,status))) => {
                            let now = Utc::now().with_timezone(&Paris).to_rfc3339();
                            if success {
                                let converted: Vec<claimer_rs_full::utilities::sql_management::UsernameResult> = results.into_iter().map(|res| 
                                    claimer_rs_full::utilities::sql_management::UsernameResult {
                                        username: res.username,
                                        uuid: res.uuid,
                                        last_seen: now.clone(),
                                    }
                                ).collect();
                                
                                if let Ok(()) = update_batch_status(&map_usernames, &converted, &map_windows) {
                                    let count = counter_200.fetch_add(1, Ordering::Relaxed) + 1;
                                    print!("\r🔨 {}/{} batchs traités", count, total_batches);
                                    error = false; // on a réussi
                                    break;
                                } 
                            }
                                if status == 429 {
                                    let _ = counter_429.fetch_add(1, Ordering::Relaxed) + 1;
                                } 
                                if status == 403 {
                                    let _= counter_403.fetch_add(1, Ordering::Relaxed) + 1;
                                }
                                // if status == 699{
                                //     println!("Erreur client : {:?}", client);
                                // }
                        }
                        _ => {}
                    }
                }
                if error 
                    {error_counter.fetch_add(1, Ordering::Relaxed);}
                tokio::time::sleep(Duration::from_millis(550)).await; 
                k = (k + 1)%max_loop; 
            }
            
        });
    }

    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl-c");
    println!("\n🛑 Ctrl-C reçu → arrêt propre.");
    
}
    



#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
    let os = std::env::consts::OS;
    println!("OS: {}", os);
    if let Err(e) = run_main().await {
        eprintln!("❌ Error: {}", e);
        
    }
    
}

async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!(" Démarrage de la vérification des pseudos Minecraft...");
    let proxies = load_proxies("proxies.txt").await;
    if proxies.is_empty() {
        eprintln!("⚠️ Aucun proxy chargé. Vérifiez proxies.txt.");
        return Ok(());
    }

    process_batches(proxies).await;
    println!("Vérification terminée.");
    Ok(())
}
