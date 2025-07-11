use chrono;
use reqwest;
use serde_json;
use std::fs;
use tokio;

struct Game {
    title: String,
    id: String,
}

fn write_json_to_disk(
    map: &serde_json::Value,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let json_string = serde_json::to_string_pretty(map)?;
    fs::write(filename, json_string)?;
    Ok(())
}

async fn get_schedule(date_string: &str) -> Result<Vec<Game>, Box<dyn std::error::Error>> {
    let body = reqwest::get(format!(
        "http://statsapi.mlb.com/api/v1/schedule?sportId=1&hydrate=team,linescore&date={}",
        date_string
    ))
    .await?
    .text()
    .await?;

    let json: serde_json::Value = serde_json::from_str(&body)?;

    let mut games: Vec<Game> = Vec::new();

    json["dates"].as_array().unwrap().iter().for_each(|date| {
        date["games"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|game| {
                let status = game["status"]["abstractGameCode"].as_str().unwrap_or("");
                status != "F" && status != "P"
            })
            .for_each(|game| {
                let home_team = game["teams"]["home"]["team"]["abbreviation"]
                    .as_str()
                    .unwrap();
                let away_team = game["teams"]["away"]["team"]["abbreviation"]
                    .as_str()
                    .unwrap();

                let game_key = format!("{}_{}", home_team, away_team);
                let home_team_score = game["teams"]["home"]["score"].as_u64().unwrap_or(0);
                let away_team_score = game["teams"]["away"]["score"].as_u64().unwrap_or(0);

                let default_map = &serde_json::Map::new();
                let linescore = game["linescore"].as_object().unwrap_or(default_map);

                let inning = linescore
                    .get("currentInningOrdinal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");
                let inning_half = linescore
                    .get("inningHalf")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Top");

                let mut inning_char = "Top of";
                if inning_half == "Bottom" {
                    inning_char = "Bottom of";
                }

                let home_team_full_name = game["teams"]["home"]["team"]["name"].as_str().unwrap();
                let away_team_full_name = game["teams"]["away"]["team"]["name"].as_str().unwrap();
                let id = format!("{} vs {}", home_team_full_name, away_team_full_name);

                games.push(Game {
                    title: format!(
                        "{} ({}) vs {} ({}) | {} {}",
                        away_team, away_team_score, home_team, home_team_score, inning_char, inning
                    ),
                    id: id,
                });
            });
    });

    return Ok(games);
}

async fn get_sources(id: String) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    println!("Getting sources for {}...", id);

    let body = reqwest::get("https://streamed.su/api/matches/baseball")
        .await?
        .text()
        .await?;

    let json: serde_json::Value = serde_json::from_str(&body)?;

    let matches = json.as_array().unwrap();

    for m in matches {
        let match_title = m["title"].as_str().unwrap();
        if match_title == id {
            let m_sources = m["sources"].as_array().unwrap().clone();
            return Ok(m_sources);
        }
    }

    return Ok(Vec::new());
}

async fn get_streams(sources: Vec<serde_json::Value>) -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    println!("Streams: ");
    println!("");

    for source in sources {
        let source_id = source["id"].as_str().unwrap();
        let source_type = source["source"].as_str().unwrap();

        let url = format!(
            "https://streamed.su/api/stream/{}/{}",
            source_type, source_id
        );

        let body = reqwest::get(url).await?.text().await?;

        let json: serde_json::Value = serde_json::from_str(&body)?;

        let streams = json.as_array().unwrap();
        for stream in streams {
            println!("{}", stream["embedUrl"].as_str().unwrap());
        }
    }

    return Ok(());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut games = get_schedule(&date).await?;

    if games.len() == 0 {
        let date = (chrono::Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        games = get_schedule(&date).await?;
    }

    println!("\nAvailable games:");
    for (i, game) in games.iter().enumerate() {
        println!("{}. {}", i + 1, game.title);
    }

    println!("\nSelect a game number:");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");

    let game_number: i32 = match input.trim().parse::<i32>() {
        Ok(num) if num > 0 && num <= games.len() as i32 => num - 1,
        _ => {
            println!("Invalid selection");
            return Ok(());
        }
    };

    println!("\nSelected game: {}", games[game_number as usize].title);
    println!("");

    let game_id = games[game_number as usize].id.clone();

    let sources = get_sources(game_id).await?;
    get_streams(sources).await?;

    return Ok(());
}
