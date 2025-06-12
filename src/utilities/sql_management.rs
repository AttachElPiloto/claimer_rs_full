use anyhow::Result;
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path};
use ahash::RandomState;          // hasher ultra-rapide
use dashmap::DashMap;            // map concurrente shardée



use chrono::{DateTime, Duration, Utc};
use chrono_tz::Europe::Paris;

use super::log_and_errors::notify_drop_window;

pub const BATCH_SIZE: usize = 10;

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Suppress dead code warning for unused fields
struct MojangResponse {
    id: String,
    name: String,
}

#[derive(Debug)]
pub struct UsernameResult {
    pub username: String,
    pub uuid: Option<String>,
    pub last_seen: String,
}

pub struct UsernameEntry {
    pub username: String,
    pub uuid: Option<String>,
    pub last_seen: String,
}


pub fn init_hashmap_from_txt(
    file_path: &str,
) -> std::io::Result<DashMap<String, (Option<String>, Option<String>), RandomState>> {
    let path = Path::new(file_path);

    // ─── Map vide si le fichier n’existe pas ────────────────────────────
    if !path.exists() {
        eprintln!("❌ Fichier introuvable : {file_path}");
        return Ok(DashMap::with_capacity_and_hasher_and_shard_amount(
            0,
            RandomState::new(),
            1_024,
        ));
    }
    // ─── Pré-allocation + hasher rapide ─────────────────────────────────
    let map: DashMap<String, (Option<String>, Option<String>), RandomState> =
        DashMap::with_capacity_and_hasher_and_shard_amount(70_000, RandomState::new(), 1_024);

    // ─── Lecture ligne par ligne ────────────────────────────────────────
    let reader = BufReader::new(File::open(path)?);
    for line in reader.lines() {
        let name = line?.trim().to_string();
        if !name.is_empty() {
            // valeur initiale : (uuid = None, last_seen = None)
            map.insert(name, (None, None));
        }
    }

    Ok(map)
}

pub fn update_batch_status(
    map: &DashMap<String, (Option<String>, Option<String>), ahash::RandomState>,
    batch_results: &[UsernameResult],
    map_windows: &DashMap<String, (String, String), ahash::RandomState>, // aussi thread-safe
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in batch_results {
        let username = entry.username.to_lowercase();
        let uuid      = entry.uuid.clone();
        let last_seen = entry.last_seen.clone();

        if let Some(mut guard) = map.get_mut(&username) {
            // guard : verrou sur le shard ⇒ mutation safe
            if guard.0.is_some() && uuid.is_none() {
                println!("feur : {} a perdu son UUID", username);
                // unwrap() sûr, car guard.1 doit déjà être Some(timestamp)
                if let Some(prev_ts) = guard.1.as_deref() {
                    get_drop_window(&username, prev_ts, &last_seen, map_windows)?;
                }
            }
            guard.0 = uuid;
            guard.1 = Some(last_seen);
            // guard droppe ici ⇒ verrou libéré
        } else {
            map.insert(username, (uuid, Some(last_seen)));
        }
    }
    Ok(())
}

pub fn get_drop_window(
    username: &str,
    last_req_time_iso: &str,
    lost_at_iso: &str,
    map_windows: &DashMap<String, (String, String), ahash::RandomState>,
) -> Result<(), Box<dyn Error>> {
    // 🔹 Conversion des timestamps ISO en chrono::DateTime<Utc>
    let lost_at_utc: DateTime<Utc> = lost_at_iso.parse()?; // pars l'ISO avec le décalage
    let lost_at = lost_at_utc.with_timezone(&Paris);       // convertit vers UTC+2

    let last_req_utc: DateTime<Utc> = last_req_time_iso.parse()?;
    let last_req_time = last_req_utc.with_timezone(&Paris);

    // 🔹 Calcul de la fenêtre de snipe (début + fin)
    let snipe_window_beginning = last_req_time + Duration::days(37);
    let snipe_window_end = lost_at + Duration::days(37);

    // 🔹 Envoi webhook
    let username_clone = username.to_string();
    tokio::spawn(async move {
        if let Err(_) = notify_drop_window(&username_clone, &snipe_window_beginning.to_rfc3339(), &snipe_window_end.to_rfc3339()).await {
            eprintln!("ERREUR ENVOIE WEBHOOK @everyone");
        }
    });

    map_windows.insert(
        username.to_string(),
        (
            snipe_window_beginning.to_rfc3339(),
            snipe_window_end.to_rfc3339(),
        ),
    );
    // write it in a .txt in a common drop windows txt in case webhook fails
    let username = username.to_string();
    let begin = snipe_window_beginning.to_rfc3339();
    let end = snipe_window_end.to_rfc3339();

    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open("drop_windows.txt")?;

        writeln!(file, "{{\"username\": \"{}\", \"begin\": \"{}\", \"end\": \"{}\"}}", username, begin, end)?;
        Ok(())
    });


    Ok(())
}



// pub fn get_drop_window(username: &str, lost_at_iso: &str, last_req_time_iso: &str) -> Result<(), Box<dyn Error>> {
//     // 🔹 Conversion des timestamps ISO en chrono::DateTime<Utc>
//     let lost_at_utc: DateTime<Utc> = lost_at_iso.parse()?; // pars l'ISO avec le décalage
//     let lost_at = lost_at_utc.with_timezone(&Paris);       // convertit vers UTC+2

//     let last_req_utc: DateTime<Utc> = last_req_time_iso.parse()?;
//     let last_req_time = last_req_utc.with_timezone(&Paris);

//     // 🔹 Calcul de la fenêtre de snipe (début + fin)
//     let snipe_window_beginning = last_req_time + Duration::days(37);
//     let snipe_window_end = lost_at + Duration::days(37);

//     // 🔹 Envoi webhook
//     let username_clone = username.to_string();
//     tokio::spawn(async move {
//         if let Err(_) = notify_drop_window(&username_clone, &snipe_window_beginning.to_rfc3339(), &snipe_window_end.to_rfc3339()).await {
//             eprintln!("ERREUR ENVOIE WEBHOOK @everyone");
//         }
//     });

//     // 🔹 Connexion à la base de données des drops
//     let conn = Connection::open(&*DROP_DB_PATH)?;

//     // 🔹 Création de la table si elle n’existe pas
//     conn.execute_batch(
//         "
//         CREATE TABLE IF NOT EXISTS drops (
//             username TEXT PRIMARY KEY,
//             window_beginning TEXT,
//             window_end TEXT
//         );
//         ",
//     )?;

//     // 🔹 Insertion ou remplacement de la fenêtre de drop
//     conn.execute(
//         "INSERT OR REPLACE INTO drops (username, window_beginning, window_end)
//          VALUES (?1, ?2, ?3)",
//         params![
//             username,
//             snipe_window_beginning.to_rfc3339(),
//             snipe_window_end.to_rfc3339()
//         ],
//     )?;

//     Ok(())
// }



// pub fn update_batch_status_direct(
//     conn: &Connection,
//     batch_results: Vec<UsernameResult>,
// ) -> Result<(), Box<dyn Error>> {

//     let usernames: Vec<String> = batch_results.iter().map(|e| e.username.clone()).collect();
//     if usernames.is_empty() {
//         return Ok(());
//     }

//     let placeholders = usernames.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
//     let query = format!(
//         "SELECT username, uuid, last_seen FROM usernames WHERE username IN ({})",
//         placeholders
//     );

//     let mut stmt = conn.prepare(&query)?;
//     let username_refs: Vec<&dyn rusqlite::ToSql> = usernames.iter().map(|u| u as _).collect();

//     let row_data: Vec<(String, Option<String>, Option<String>)> = stmt
//         .query_map(&*username_refs, |row| {
//             Ok((
//                 row.get::<_, String>(0)?,
//                 row.get::<_, Option<String>>(1)?,
//                 row.get::<_, Option<String>>(2)?,
//             ))
//         })?
//         .collect::<Result<_, _>>()?;

//     let mut uuid_map = HashMap::new();
//     let mut last_seen_map = HashMap::new();
//     for (name, uuid, last_seen) in row_data {
//         uuid_map.insert(name.clone(), uuid);
//         last_seen_map.insert(name, last_seen);
//     }

//     let tx = conn.unchecked_transaction()?;

//     for entry in batch_results {
//         let now = entry.last_seen.clone(); // ← utilise le timestamp exact
//         let name = &entry.username;
//         let new_uuid = entry.uuid.clone();
//         let old_uuid = uuid_map.get(name).cloned().flatten();
//         let last_time_req = last_seen_map.get(name).cloned().flatten();

//         if old_uuid.is_some() && new_uuid.is_none() {
//             tx.execute(
//                 "UPDATE usernames SET uuid = NULL, uuid_lost_at = ?1, last_seen = ?2 WHERE username = ?3",
//                 params![now, now, name],
//             )?;

//             if let Some(last) = last_time_req {
//                 let _ = get_drop_window(name, &now, &last);
//             }
//         } else {
//             tx.execute(
//                 "UPDATE usernames SET uuid = ?1, last_seen = ?2, uuid_lost_at = NULL WHERE username = ?3",
//                 params![new_uuid, now, name],
//             )?;
//         }
//     }

//     tx.commit()?;
//     Ok(())
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::io::Write;
//     use chrono::Utc;
//     use rusqlite::Connection;
//     use tempfile::NamedTempFile;

//     #[test]
//     fn test_init_db() {
//         let _ = init_db().expect("should initialize db");
//         assert!(DB_PATH.exists());
//     }

//     #[test]
//     fn test_init_db_from_txt() {
//         let mut tmp_file = NamedTempFile::new().unwrap();
//         writeln!(tmp_file, "Dream\nnotch\njeb_").unwrap();

//         let path = tmp_file.path().to_str().unwrap();
//         init_db_from_txt(path).expect("should import usernames");

//         let conn = Connection::open(&*DB_PATH).unwrap();
//         let count: usize = conn.query_row("SELECT COUNT(*) FROM usernames", [], |row| row.get(0)).unwrap();
//         assert!(count >= 3);
//     }

    // #[test]
    // fn test_get_drop_window() {
    //     let username = "testuser";
    //     let now = Utc::now().to_rfc3339();
    //     let past = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();

    //     get_drop_window(username, &now, &past).expect("should insert drop window");

    //     let conn = Connection::open(&*DROP_DB_PATH).unwrap();
    //     let exists: Result<String, _> = conn.query_row(
    //         "SELECT username FROM drops WHERE username = ?1",
    //         [username],
    //         |row| row.get(0),
    //     );
    //     assert_eq!(exists.unwrap(), username);
    // }

//     #[test]
//     fn test_update_batch_status() {
//         init_db().unwrap();
//         let entries = vec![
//             UsernameEntry {
//                 username: "dream".into(),
//                 uuid: Some("uuid-1".into()),
//             },
//             UsernameEntry {
//                 username: "notch".into(),
//                 uuid: None,
//             },
//         ];

//         let mut tmp_file = NamedTempFile::new().unwrap();
//         writeln!(tmp_file, "dream\nnotch\njeb_").unwrap();
//         let path = tmp_file.path().to_str().unwrap();
//         init_db_from_txt(path).unwrap();

//         update_batch_status(entries).expect("should update batch status");

//         let conn = Connection::open(&*DB_PATH).unwrap();
//         let uuid: Option<String> = conn.query_row(
//             "SELECT uuid FROM usernames WHERE username = ?1",
//             ["dream"],
//             |row| row.get(0),
//         ).unwrap();
//         assert_eq!(uuid, Some("uuid-1".to_string()));
//     }


//     #[test]
//     fn test_update_batch_status_uuid_lost() {
//         init_db().unwrap();

//         // 🔹 Étape 1 : On insère un utilisateur avec un UUID présent
//         let conn = Connection::open(&*DB_PATH).unwrap();
       
//         // 🔹 Étape 2 : On passe un nouveau batch sans UUID → simulate perte d'UUID
//         let batch = vec![
//             UsernameEntry {
//                 username: "dream".to_string(),
//                 uuid: None,
//             }
//         ];

//         update_batch_status(batch).unwrap();

//         // 🔹 Étape 3 : On vérifie que l'UUID est devenu NULL
//         let (uuid, lost_at): (Option<String>, Option<String>) = conn.query_row(
//             "SELECT uuid, uuid_lost_at FROM usernames WHERE username = 'dream'",
//             [],
//             |row| Ok((row.get(0)?, row.get(1)?))
//         ).unwrap();

//         assert_eq!(uuid, None);
//         assert!(lost_at.is_some());

//         // 🔹 Étape 4 : On vérifie que la ligne a été ajoutée dans drop_windows.db
//         let conn_drop = Connection::open(&*DROP_DB_PATH).unwrap();
//         let exists: Result<String, _> = conn_drop.query_row(
//             "SELECT username FROM drops WHERE username = 'dream'",
//             [],
//             |row| row.get(0),
//         );

//         assert_eq!(exists.unwrap(), "dream");
//     }
// }


#[test]
fn test_init_hashmap_from_txt() {
    

    let path = "C:/Users/cinq2/Documents/claimer_rs_full/feur.txt";
    if !Path::new(path).exists() {
        panic!("Test file not found: {}", path);
    }
    let map = init_hashmap_from_txt(path).expect("should initialize hashmap from txt");


    println!("Map contents:");
    for entry in map.iter() {
        let (key, value) = entry.pair();
        println!("{}: {:?}", key, value);
    }
    for mut entry in map.iter_mut() {
        let (_, value_mut) = entry.pair_mut();
        // modify the value
        let (uuid, last_seen) = value_mut;
        let new_value = (uuid.clone(), last_seen.clone());
        *value_mut = new_value;
    }
    println!("Map size: {}", map.len());


}

#[tokio::test(flavor = "current_thread")]     // runtime Tokio dédié au test
async fn test_update_batch_status() -> Result<(), Box<dyn std::error::Error>> {
    // ───── Map principale pré-remplie ────────────────────────────────
    let users: DashMap<String, (Option<String>, Option<String>), RandomState> =
        DashMap::with_capacity_and_hasher_and_shard_amount(16, RandomState::new(), 16);

    users.insert(
        "dream".into(),
        (Some("uuid-0".into()), Some(Utc::now().to_rfc3339())),
    );
    users.insert(
        "notch".into(),
        (None, Some(Utc::now().to_rfc3339())),
    );

    // ───── Map des fenêtres de drop ──────────────────────────────────
    let drop_windows: DashMap<String, (String, String), RandomState> =
        DashMap::with_capacity_and_hasher_and_shard_amount(4, RandomState::new(), 4);

    // ───── 1ʳᵉ vague de résultats : Dream obtient un nouvel UUID ─────
    let batch1 = vec![
        UsernameResult {
            username: "dream".into(),
            uuid: Some("uuid-1".into()),
            last_seen: Utc::now().to_rfc3339(),
        },
        UsernameResult {
            username: "notch".into(),
            uuid: None,
            last_seen: Utc::now().to_rfc3339(),
        },
    ];
    update_batch_status(&users, &batch1, &drop_windows)?;

    assert_eq!(
        users.get("dream").unwrap().0,
        Some("uuid-1".to_string())
    );
    assert!(users.get("notch").unwrap().0.is_none());

    // ───── 2ᵉ vague : Dream perd son UUID → doit créer une fenêtre ────
    let batch2 = vec![UsernameResult {
        username: "dream".into(),
        uuid: None,
        last_seen: Utc::now().to_rfc3339(),
    }];
    update_batch_status(&users, &batch2, &drop_windows)?;
    tokio::task::yield_now().await;           // ou sleep 50 ms


    // Une entrée "dream" doit exister dans `drop_windows`
    let window = drop_windows.get("dream").expect("drop window missing");
    println!("Dream drop window: {:?}", *window);
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await; // pour laisser le temps à la tâche asynchrone de s'exécuter

    Ok(())
}