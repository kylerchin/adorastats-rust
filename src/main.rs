//run an async function called fetch() every 2 minutes

use std::time::Duration;
use tokio::time;
use std::thread;
use rand::Rng;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use rand::seq::SliceRandom;
use std::sync::{Arc, RwLock};
use scylla::IntoTypedRows;
use uuid::Uuid;
use std::fs::File;
use std::io::{BufRead, BufReader};
use serde_json::{Result, Value};
use termion::{color, style};

//add scylla dependencies
use reqwest::Client as ReqwestClient;
use scylla::{Session, SessionBuilder};

use reqwest::Error;

#[tokio::main]
async fn main() {
    let client = ReqwestClient::new();
    
    let yt_file = File::open("./ytkeys.txt").unwrap();
    let yt_reader = BufReader::new(yt_file);
    let yt_api_keys: Vec<String> = yt_reader.lines().map(|line| line.unwrap()).collect();

    let scylla_file :  std::fs::File = File::open("./scyllakeys.txt").unwrap();
let scylla_reader : std::io::BufReader<std::fs::File> = BufReader::new(scylla_file);
let scylla_keys: Vec<String> = scylla_reader.lines().map(|line| line.unwrap()).collect();

    //ensure scylla_keys length is 2

    if scylla_keys.len() != 2 {
        println!("scylla_keys length is not 2");
        return;
    }

    //print yt_api_keys length

    println!("yt_api_keys length: {}", yt_api_keys.len());

    if !(yt_api_keys.len() > 0) {
        println!("You have to have more than 0 api keys in ytkeys.txt");
        return;
    }

    let number_of_calls_per_day = 10_000 * yt_api_keys.len();
    let number_of_videos = number_of_calls_per_day / 120;

    println!("number_of_calls_per_day: {}", number_of_calls_per_day);
    println!("number_of_videos: {}", number_of_videos);

    let scylla_username = scylla_keys[0].clone();
    let scylla_password = scylla_keys[1].clone();

    let session: Session = SessionBuilder::new()
    .known_node("127.0.0.1:9042")
    .user(scylla_username, scylla_password)
    .build()
    .await
    .unwrap();

    let mut lastloop = Instant::now();

    loop {

        

        fetch(&session, &yt_api_keys, &client).await;

        let duration = lastloop.elapsed();

        println!(
            "{}loop time is: {:?}{}",
            color::Bg(color::Green),
            duration,
            style::Reset
        );

        //if the iteration of the loop took <5 min, sleep for the remainder of the 5 min
        if (duration.as_millis() as i32) < 300_000 {
            let sleep_duration = Duration::from_millis(300_000) - duration;
            println!("sleeping for {:?}", sleep_duration);
            std::thread::sleep(sleep_duration);
        }
    }
}

async fn fetch(session: &Session, yt_api_keys: &Vec<String>, client: &ReqwestClient) {
    println!("fetching...");
    
    let scylla_file :  std::fs::File = File::open("./scyllakeys.txt").unwrap();
let scylla_reader : std::io::BufReader<std::fs::File> = BufReader::new(scylla_file);
let scylla_keys: Vec<String> = scylla_reader.lines().map(|line| line.unwrap()).collect();
    
    let scylla_username = scylla_keys[0].clone();
    let scylla_password = scylla_keys[1].clone();
    //select all rows from the table "adorastats.trackedytvideosids" (videoid text PRIMARY KEY, added timeuuid, videoname text)

    if let Some(rows) = session.query("SELECT videoid FROM adorastats.trackedytvideosids", &[]).await.unwrap().rows {
        
        for row in rows {
            let videoid: String = row.columns[0].as_ref().unwrap().as_text().unwrap().to_string();

            println!("videoid: {}", videoid);

            //pick random item from yt_api_keys
            let chosen_api_key = yt_api_keys.choose(&mut rand::thread_rng()).unwrap();
            let url : String = format!("https://youtube.googleapis.com/youtube/v3/videos?part=statistics&id={}&key={}", videoid, chosen_api_key);								
            
                getvideo(&session, url, videoid, client).await;
               // getvideo(session, url, videoid).await;
        }

    }
}

async fn getvideo(session: &Session, url: String, videoid: String, client: &ReqwestClient) {

    println!("url: {}", url);

   // let response = reqwest::get(url).await.unwrap();

    match client.get(url).send().await {
        Ok(response) => {
           // println!("response: {}", resp.status());
           
    let insertquery = "INSERT INTO adorastats.ytvideostats (videoid, time, views, likes, comments) VALUES (?,?,?,?,?)";

    let body = response.text().await.unwrap();

    println!("body: {}", body);

    if let Ok(json) = serde_json::from_str::<Value>(&body) {

        //if json["items"][0] does not exist, skip

        if json["items"][0].is_null() {
            println!("json[\"items\"][0] is null for video {}", videoid);
            return;
        }

        //ensure views is i64 and not null

        if json["items"][0]["statistics"]["viewCount"].is_null() {
            println!("json[\"items\"][0][\"statistics\"][\"viewCount\"] is null for video {}", videoid);
            return;
        }

        //ensure likes is i64 and not null

        if json["items"][0]["statistics"]["likeCount"].is_null() {
            println!("json[\"items\"][0][\"statistics\"][\"likeCount\"] is null for video {}", videoid);
            
            let views:i64 = json["items"][0]["statistics"]["viewCount"].as_str().unwrap().parse::<i64>().unwrap();

            println!("views: {}", views);

            let nodeid = [0,0,0,0,0,0];
            let yt_uuid = uuid::Uuid::now_v1(&nodeid);
    
            //insert into scylla
            session.query("INSERT INTO adorastats.ytvideostats (videoid, time, views) VALUES (?,?,?)", (&videoid, &yt_uuid, &views)).await.unwrap();
            session.query("UPDATE adorastats.statpoints SET amount = amount + 1 WHERE source = 'youtube';", &[]).await.unwrap();
        } else {
               //ensure comments is i64 and not null

        if json["items"][0]["statistics"]["commentCount"].is_null() {
            println!("json[\"items\"][0][\"statistics\"][\"commentCount\"] is null for video {}", videoid);
        return;
        }

        let views:i64 = json["items"][0]["statistics"]["viewCount"].as_str().unwrap().parse::<i64>().unwrap();
        let likes:i64 = json["items"][0]["statistics"]["likeCount"].as_str().unwrap().parse::<i64>().unwrap();
        let comments:i64 = json["items"][0]["statistics"]["commentCount"].as_str().unwrap().parse::<i64>().unwrap();

        println!("views: {}", views);
        println!("likes: {}", likes);
        println!("comments: {}", comments);

        let nodeid = [0,0,0,0,0,0];
        let yt_uuid = uuid::Uuid::now_v1(&nodeid);

        //insert into scylla
        session.query(insertquery, (&videoid, &yt_uuid, &views, &likes, &comments)).await.unwrap();
        session.query("UPDATE adorastats.statpoints SET amount = amount + 1 WHERE source = 'youtube';", &[]).await.unwrap();
        }

     
}
        },
        Err(e) => {
            println!("error: {}", e);
        }
    }

}