use aws_config::BehaviorVersion;
/// This Rust program is designed to scale the number of processes managed by Supervisor based on CPU usage and the length of an AWS SQS queue.
///
/// The main components of the program are:
///
/// - `get_cpu_usage`: Asynchronously retrieves the current CPU usage of the system.
/// - `get_sqs_queue_length`: Asynchronously retrieves the length of the specified SQS queue.
/// - `get_current_num_procs`: Reads the current number of processes from the Supervisor configuration file.
/// - `update_supervisor_config`: Updates the number of processes in the Supervisor configuration file.
/// - `reload_supervisor`: Reloads the Supervisor configuration to apply changes.
/// - `scaling_loop`: The main loop that periodically checks CPU usage and queue length, and scales the number of processes accordingly.
///
/// The program is configured using command-line arguments:
///
/// - `scale_factor`: The message count threshold to trigger scaling.
/// - `min_num_process`: The minimum number of processes to maintain.
/// - `max_num_process`: The maximum number of processes to maintain.
/// - `supervisor_config_path`: The path to the Supervisor configuration file.
/// - `queue_url`: The URL of the SQS queue.
///
/// The program runs indefinitely, periodically checking the CPU usage and queue length, and adjusting the number of processes as needed.
///
/// # Usage
///
/// ```sh
/// cargo run -- --scale_factor <SCALE_FACTOR> --min_num_process <MIN_NUM_PROCS> --max_num_process <MAX_NUM_PROCS> --supervisor_config_path <SUPERVISOR_CONFIG_PATH> --queue_url <QUEUE_URL>
/// ```
///
/// # Example
///
/// ```sh
/// cargo run -- --scale_factor 100 --min_num_process 1 --max_num_process 10 --supervisor_config_path /etc/supervisor/conf.d/myapp.conf --queue_url https://sqs.us-west-2.amazonaws.com/XXXXXX/my-queue
/// ```
///
/// # Tests
///
/// The program includes several tests to verify its functionality:
///
/// - `test_get_cpu_usage`: Tests that the CPU usage is within a valid range.
/// - `test_get_sqs_queue_length`: Tests that the SQS queue length is retrieved correctly.
/// - `test_get_current_num_procs`: Tests that the current number of processes is read correctly from the configuration file.
/// - `test_update_supervisor_config`: Tests that the Supervisor configuration file is updated correctly.
/// - `test_reload_supervisor`: Tests that the Supervisor configuration is reloaded correctly (requires `supervisorctl` to be installed and configured).
use aws_sdk_sqs::types::QueueAttributeName;
use aws_sdk_sqs::{Client, Error};
use clap::Parser;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::process::Command;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tokio::time::{sleep, Duration};
const MAX_CPU_USAGE: f32 = 75.0;
const TIME_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Sets the maximum number of processes
    #[arg(short = 'x', long = "max_num_process", value_name = "max_num_process")]
    max_num_process: usize,

    /// Sets the minimum number of processes
    #[arg(short = 'm', long = "min_num_process", value_name = "min_num_process")]
    min_num_process: usize,

    /// Sets the URL of the SQS queue
    #[arg(short = 'q', long = "queue_url", value_name = "queue_url")]
    queue_url: String,

    /// Sets the scale factor for the number of processes
    #[arg(short = 's', long = "scale_factor", value_name = "scale_factor")]
    scale_factor: usize,

    /// Sets the path to the Supervisor configuration file
    #[arg(
        short = 'c',
        long = "supervisor_config_path",
        value_name = "supervisor_config_path"
    )]
    supervisor_config_path: String,
}

async fn get_cpu_usage() -> f32 {
    let mut system =
        System::new_with_specifics(RefreshKind::default().with_cpu(CpuRefreshKind::everything()));
    sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_cpu_usage();
    system.global_cpu_usage()
}

async fn get_sqs_queue_length(client: &Client, queue_url: &str) -> Result<usize, Error> {
    let response = client
        .get_queue_attributes()
        .queue_url(queue_url)
        .attribute_names(QueueAttributeName::ApproximateNumberOfMessages)
        .send()
        .await?;

    if let Some(attributes) = response.attributes {
        if let Some(message_count) =
            attributes.get(&QueueAttributeName::ApproximateNumberOfMessages)
        {
            return Ok(message_count.parse::<usize>().unwrap_or(0));
        }
    }
    Ok(0)
}

async fn get_current_num_procs(path: &str) -> io::Result<usize> {
    let file = OpenOptions::new().read(true).open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("numprocs=") {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                if let Ok(num_procs) = parts[1].trim().parse::<usize>() {
                    return Ok(num_procs);
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "numprocs not found",
    ))
}

async fn update_supervisor_config(num_procs: usize, path: &str) -> io::Result<()> {
    let file = OpenOptions::new().read(true).open(path)?;
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();

    for line in reader.lines() {
        let mut line = line?;
        if line.starts_with("numprocs=") {
            line = format!("numprocs={}", num_procs);
        }
        lines.push(line);
    }

    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    for line in lines {
        writeln!(file, "{}", line)?;
    }
    Ok(())
}

async fn reload_supervisor() -> io::Result<()> {
    let output = Command::new("sudo")
        .arg("supervisorctl")
        .arg("reread")
        .output()
        .expect("Failed to execute supervisorctl reread command");

    if !output.status.success() {
        eprintln!(
            "supervisorctl reread failed: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to reread supervisor config",
        ));
    }

    let output = Command::new("sudo")
        .arg("supervisorctl")
        .arg("update")
        .output()
        .expect("Failed to execute supervisorctl update command");

    if !output.status.success() {
        eprintln!(
            "supervisorctl update failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to update supervisor config",
        ));
    }

    Ok(())
}

async fn scaling_loop(
    scale_factor: usize,
    min_num_process: usize,
    max_num_process: usize,
    supervisor_config_path: &str,
    queue_url: &str,
) {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = Client::new(&config);
    let mut interval = tokio::time::interval(TIME_INTERVAL);
    loop {
        interval.tick().await;

        let cpu_usage = get_cpu_usage().await;
        // If CPU usage is high, continue
        if cpu_usage >= MAX_CPU_USAGE {
            println!(
                "CPU usage is high ({}%), waiting for 60 seconds...",
                cpu_usage
            );
            continue;
        }

        // Calculate the number of processes based on the queue length
        let queue_length = get_sqs_queue_length(&client, queue_url).await.unwrap_or(0);
        let num_procs: usize =
            (queue_length / scale_factor).clamp(min_num_process, max_num_process);

        let current_num_procs = get_current_num_procs(supervisor_config_path).await.unwrap();
        // If the number of processes is already at the desired level, continue
        if num_procs == current_num_procs {
            continue;
        }

        // Update the Supervisor configuration and reload the Supervisor process
        if update_supervisor_config(num_procs, supervisor_config_path)
            .await
            .is_ok()
            && reload_supervisor().await.is_ok()
        {
            println!(
                "Qscaler scaling finished, CPU threshold {}%, current CPU usage: {}%, \
                        queue length threshold {}, current queue length: {}, \
                        current number of processes {}, new number of processes {}",
                MAX_CPU_USAGE, cpu_usage, scale_factor, queue_length, current_num_procs, num_procs
            );
        } else {
            eprintln!(
                "Fail to scaling processes, rollback the number of processes {}",
                current_num_procs
            );
            update_supervisor_config(current_num_procs, supervisor_config_path)
                .await
                .expect("Fail to rollback the number of processes");
            reload_supervisor()
                .await
                .expect("Fail to reload supervisor");
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!(
        "Qscaler started with min_num_process: {}, max_num_process: {}, supervisor_config_path: {}, queue_url: {}",
        args.min_num_process, args.max_num_process, args.supervisor_config_path, args.queue_url
    );

    scaling_loop(
        args.scale_factor,
        args.min_num_process,
        args.max_num_process,
        &args.supervisor_config_path,
        &args.queue_url,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tokio::fs;

    #[tokio::test]
    async fn test_get_cpu_usage() {
        let cpu_usage = get_cpu_usage().await;
        assert!(cpu_usage >= 0.0 && cpu_usage <= 100.0);
    }

    #[tokio::test]
    async fn test_get_sqs_queue_length() {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        let queue_url =
            "https://sqs.us-west-2.amazonaws.com/XXXXXX/test.fifo";
        let queue_length = get_sqs_queue_length(&client, queue_url).await.unwrap();
        assert!(queue_length >= 0);
    }

    #[tokio::test]
    async fn test_get_current_num_procs() {
        let test_config_path = "example/sample.conf";
        let mut file = File::create(test_config_path).unwrap();
        writeln!(file, "numprocs=5").unwrap();

        let num_procs = get_current_num_procs(test_config_path).await.unwrap();
        assert_eq!(num_procs, 5);

        fs::remove_file(test_config_path).await.unwrap();
    }

    #[tokio::test]
    async fn test_update_supervisor_config() {
        let test_config_path = "test_supervisor.conf";
        let mut file = File::create(test_config_path).unwrap();
        writeln!(file, "numprocs=5").unwrap();

        update_supervisor_config(10, test_config_path)
            .await
            .unwrap();

        let num_procs = get_current_num_procs(test_config_path).await.unwrap();
        assert_eq!(num_procs, 10);
    }

    #[tokio::test]
    async fn test_reload_supervisor() {
        // This test assumes that supervisorctl is installed and configured correctly.
        // It will not work in an environment where supervisorctl is not available.
        let result = reload_supervisor().await;
        assert!(result.is_ok());
    }
}
