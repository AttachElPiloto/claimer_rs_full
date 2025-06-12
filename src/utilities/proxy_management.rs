// use std::collections::{HashMap, VecDeque};
// use std::sync::Arc;
// use std::time::{Duration, Instant};
// use rand::distr::weighted::WeightedIndex;
// use rand::{prelude::*, rng};
// use tokio::sync::Mutex;

// const WINDOW: Duration = Duration::from_secs(60);

// #[derive(Clone)]
// pub struct ProxyStats {
//     pub url: String,
//     calls: Arc<Mutex<VecDeque<(Instant, bool)>>>,
//     last_call: Arc<Mutex<Instant>>,
// }

// impl ProxyStats {
//     pub fn new(url: &str) -> Self {
//         Self {
//             url: url.to_string(),
//             calls: Arc::new(Mutex::new(VecDeque::new())),
//             last_call: Arc::new(Mutex::new(Instant::now() - WINDOW)),
//         }
//     }

//     async fn trim(&self) {
//         let now = Instant::now();
//         let mut calls = self.calls.lock().await;
//         while let Some((ts, _)) = calls.front() {
//             if now.duration_since(*ts) > WINDOW {
//                 calls.pop_front();
//             } else {
//                 break;
//             }
//         }
//     }

//     async fn metrics(&self) -> (usize, usize) {
//         self.trim().await;
//         let calls = self.calls.lock().await;
//         let total = calls.len();
//         let ok = calls.iter().filter(|(_, success)| *success).count();
//         (total, ok)
//     }

//     pub async fn record(&self, success: bool) {
//         let mut calls = self.calls.lock().await;
//         calls.push_back((Instant::now(), success));
        
//         // Update last_call time
//         *self.last_call.lock().await = Instant::now();
//     }

//     pub async fn score(&self) -> f64 {
//         let (total, ok) = self.metrics().await;
//         if total == 0 {
//             return 100.0; // Random bonus
//         }
//         let success_rate = ok as f64 / total as f64;
//         let load_penalty = (total as f64).sqrt();
//         100.0 * success_rate.powi(3) / load_penalty
//     }
// }

// #[derive(Clone)]
// pub struct AdaptiveProxyPool {
//     proxies: Arc<Mutex<HashMap<String, ProxyStats>>>,
// }

// impl AdaptiveProxyPool {
//     pub fn new(proxy_urls: &[String]) -> Self {
//         let map = proxy_urls
//             .iter()
//             .map(|url| (url.clone(), ProxyStats::new(url)))
//             .collect();

//         Self {
//             proxies: Arc::new(Mutex::new(map)),
//         }
//     }

//     pub async fn acquire(&self) -> ProxyStats {
//         let proxies = self.proxies.lock().await;
//         if proxies.is_empty() {
//             panic!("No proxies available in the pool");
//         }

//         let mut stats_with_scores: Vec<(ProxyStats, f64)> = Vec::with_capacity(proxies.len());

//         // Calculate scores once and store them with their associated proxy stats
//         for proxy in proxies.values() {
//             let score = proxy.score().await;
//             stats_with_scores.push((proxy.clone(), score));
//         }

//         // Use the pre-calculated scores for weighted selection
//         let distribution = stats_with_scores.iter().map(|(_, score)| *score);
        
//         match WeightedIndex::new(distribution) {
//             Ok(dist) => {
//                 let index = dist.sample(&mut rng());
//                 stats_with_scores[index].0.clone()
//             },
//             Err(_) => {
//                 // Fallback if weighted selection fails
//                 let index = rng().random_range(0..stats_with_scores.len());
//                 stats_with_scores[index].0.clone()
//             }
//         }
//     }

//     pub async fn record(&self, proxy: &ProxyStats, success: bool) {
//         proxy.record(success).await;
//     }
// }