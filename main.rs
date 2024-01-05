use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: ApiData,
}

#[derive(Debug, Deserialize)]
struct ApiData {
    amount: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "--mode=cache" => {
            println!("You selected - cache");
            if args.len() >= 1 && args[2].starts_with("--times=") {
                let times: u64 = args[2].split('=').nth(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                custom_client(times).await?;
            } else {
                println!("Invalid argument for cache mode. Use --times=<seconds>.");
            }
        }
        "--mode=read" => {
            println!("You selected read mode ");
            read_operation()?;
        }
        _ => {
            println!("Invalid mode");
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!(" cargo run --mode=<cache/read> --times=<secs>");
}

async fn custom_client(times: u64) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    let shared_data = Arc::new(Mutex::new(AggregatedData::new()));

    let handles: Vec<_> = (1..=5)
        .map(|i| {
            let shared_data_clone = shared_data.clone();
            tokio::spawn(simulate_api_client(i, times, start_time, shared_data_clone))
        })
        .collect();

    for handle in handles {
        let _ = handle.await?;
    }

    let final_aggregate = shared_data.lock().unwrap().compute_final_aggregate();
    println!("Aggregated Result: Final aggregate of USD prices of BTC is: {}", final_aggregate);

    write_result_to_file(final_aggregate)?;

    Ok(())
}

fn write_result_to_file(final_aggregate: f64) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "result.txt";

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)?;

    writeln!(file, "Final aggregate of USD prices of BTC is = {}", final_aggregate)?;

    Ok(())
}

async fn simulate_api_client(
    client_id: usize,
    times: u64,
    start_time: Instant,
    shared_data: Arc<Mutex<AggregatedData>>,
) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
    let url = "https://api.coinbase.com/v2/prices/spot?currency=USD";
    let api_client = reqwest::Client::new();

    let mut total_amount = 0.0;
    let mut count = 0;

    while start_time.elapsed().as_secs() < times {
        if let Ok(response) = api_client.get(url).send().await {
            let body = response.text().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

            if let Ok(message) = serde_json::from_str::<ApiResponse>(&body) {
                let amount = message.data.amount.parse::<f64>().unwrap_or(0.0);
                total_amount += amount;
                count += 1;
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let average_amount = total_amount / count as f64;
    println!("Client {}: Average USD price of BTC is: {}", client_id, average_amount);

    shared_data.lock().unwrap().add_average(average_amount);

    Ok(())
}

fn read_operation() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "result.txt";

    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            if metadata.len() == 0 {
                println!("The result.txt file is empty. Run in cache mode first.");
            } else {
                let file = File::open(file_path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    println!("{}", line?);
                }
            }

            Ok(())
        }
        Err(_) => {
            println!("The result.txt file does not exist. Run in cache mode first.");
            Ok(())
        }
    }
}

#[derive(Debug)]
struct AggregatedData {
    averages: Vec<f64>,
}

impl AggregatedData {
    fn new() -> Self {
        AggregatedData { averages: Vec::new() }
    }

    fn add_average(&mut self, average: f64) {
        self.averages.push(average);
    }

    fn compute_final_aggregate(&self) -> f64 {
        if self.averages.is_empty() {
            0.0
        } else {
            self.averages.iter().sum::<f64>() / self.averages.len() as f64
        }
    }
}
