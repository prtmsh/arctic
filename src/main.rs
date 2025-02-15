use clap::{Arg, Command};
use rand::Rng;
use reqwest::Error;
use serde_json::{Value, Map, Number};
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

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

    println!("starting arctic...");
    println!("duration: {} seconds", duration);
    println!("endpoint: {}", endpoint);
    println!("templates: {}", data_file);

    while start_time.elapsed().as_secs() < duration {
        let random_data = generate_random_data(&schema);
        if let Err(e) = send_data(endpoint, random_data).await {
            eprintln!("error sending data: {}", e);
        }
    }

    println!("completed {} seconds of data transmission", duration);
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

    if !response.status().is_success() {
        eprintln!("server responded with: {}", response.status());
    }

    Ok(())
}