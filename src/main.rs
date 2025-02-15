use clap::{Arg, Command};
use rand::Rng;
use reqwest::Error;
use serde_json::{Value, Map, Number};
use std::fs;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::sleep;
use std::io::{stdout, Write};

#[derive(Debug)]
struct LoadTestStats {
    total_requests: AtomicU64,
    success_count: AtomicU64,
    error_count: AtomicU64,
    total_duration: AtomicU64,
    start_time: Instant,
}

impl LoadTestStats {
    fn new() -> Self {
        LoadTestStats {
            total_requests: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            total_duration: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn print_summary(&self) {
        let total = self.total_requests.load(Ordering::SeqCst);
        let success = self.success_count.load(Ordering::SeqCst);
        let errors = self.error_count.load(Ordering::SeqCst);
        let total_duration = self.start_time.elapsed().as_secs_f64();
        let avg_rps = total as f64 / total_duration;

        let avg_response_time = 
        if total > 0 {
            self.total_duration.load(Ordering::SeqCst) as f64 / total as f64
        }
        else {
            0.0
        };

        println!("\n==== Load Test Summary ====");
        println!("total duration:      {:.2}s", total_duration);
        println!("total requests:      {}", total);
        println!("successful requests: {}", success);
        println!("failed requests:     {}", errors);
        println!("requests per second: {:.2}", avg_rps);
        println!("avg response time:   {:.2}ms", avg_response_time);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let matches = Command::new("arctic")
        .version("0.1.0")
        .about("sends randomized json to api endpoints")
        .arg(
            Arg::new("endpoint")
                .short('e')
                .long("endpoint")
                .required(true)
        )
        .arg(
            Arg::new("time")
                .short('t')
                .long("time")
                .value_parser(clap::value_parser!(u64))
                .required(true)
        )
        .arg(
            Arg::new("data")
                .short('d')
                .long("data")
                .required(true)
        )
        .get_matches();
    
    let endpoint = matches.get_one::<String>("endpoint").unwrap();
    let duration = *matches.get_one::<u64>("time").unwrap();
    let data_file = matches.get_one::<String>("data").unwrap();

    let schema = read_json_file(data_file)?;
    let start_time = std::time::Instant::now();
    let stats = Arc::new(LoadTestStats::new());

    println!(r"
        ___              __  _     
       /   |  __________/ /_(_)____
      / /| | / ___/ ___/ __/ / ___/
     / ___ |/ /  / /__/ /_/ / /__  
    /_/  |_/_/   \___/\__/_/\___/
    ");
    println!("duration: {} seconds", duration);
    println!("endpoint: {}", endpoint);
    println!("templates: {}", data_file);

    let stats_clone = stats.clone();
    let endpoint_clone = endpoint.clone();
    let schema_clone = schema.clone();

    let is_running = Arc::new(AtomicBool::new(true));
    let spinner_is_running = is_running.clone();

    let spinner_handle = tokio::spawn(async move {
        let spinner_frames = ["|", "/", "-", "\\"];
        let mut i = 0;
        
        while spinner_is_running.load(Ordering::SeqCst) {
            print!("\rRunning... {}", spinner_frames[i]);
            stdout().flush().ok();
            i = (i+1)%spinner_frames.len();
            sleep(Duration::from_millis(150)).await; 
        }
        print!("\rDone.     \n");
        stdout().flush().ok();
    });

    let load_test_handle = tokio::spawn(async move {
        while start_time.elapsed().as_secs() < duration {
            let random_data = generate_random_data (&schema_clone);
            let requests_start = Instant::now();

            match send_data(&endpoint_clone, random_data).await {
                Ok(_) => {
                    stats_clone.success_count.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    stats_clone.error_count.fetch_add(1, Ordering::SeqCst);
                    eprintln!("error sending data: {}", e);
                }
            }

            let duration = requests_start.elapsed().as_millis() as u64;
            stats_clone.total_duration.fetch_add(duration, Ordering::SeqCst);
            stats_clone.total_requests.fetch_add(1, Ordering::SeqCst);
        }
    });

    load_test_handle.await?;
    is_running.store(false, Ordering::SeqCst);
    spinner_handle.await?;

    stats.print_summary();
    Ok(())
}

fn read_json_file(path: &str) -> Result<Value, Box<dyn std::error::Error>>{
    let data = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&data)?;
    Ok(json)
}

fn generate_random_data(schema: &Value) -> Value {
    let mut rng = rand::thread_rng();

    match schema {
        Value::Object(map) => {
            let mut random_map = Map::new();
            for(key, value) in map {
                random_map.insert(key.clone(), generate_random_data(value));
            }
            Value::Object(random_map)
        }
        Value::String(_) => Value::String(rng.gen::<u32>().to_string()),
        Value::Number(n) if n.is_i64() => Value::Number(rng.gen::<i64>().into()),
        Value::Number(n) if n.is_f64() => {
            let num = rng.gen::<f64>();
            Value::Number(Number::from_f64(num).unwrap_or_else(|| Number::from(0)))
        },
        Value::Bool(_) => Value::Bool(rng.gen()),
        Value::Array(arr) => {
            let mut random_arr = Vec::new();
            if !arr.is_empty() {
                for _ in 0..rng.gen_range(1..5) {
                    random_arr.push(generate_random_data(&arr[0]));
                }
            }
            Value::Array(random_arr)
        }
        val => val.clone(),
    }
}

async fn send_data(endpoint: &str, data: Value) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .json(&data)
        .send()
        .await?;

    response.error_for_status()?;
    Ok(())
}