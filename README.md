# Dynamic Process Scaler
This Rust program dynamically adjusts the number of processes managed by Supervisor based on CPU usage and the length of an AWS SQS queue.

## Constants
- `MAX_CPU_USAGE`: The maximum CPU usage percentage allowed before scaling processes.
- `TIME_INTERVAL`: The interval (in seconds) between each scaling check.

## Functions

- `get_cpu_usage`: Asynchronously retrieves the current CPU usage percentage.
- `get_sqs_queue_length`: Asynchronously retrieves the approximate number of messages in the specified SQS queue.
- `update_supervisor_config`: Updates the Supervisor configuration file to set the number of processes and reloads the configuration.

## Main Function

The `main` function:
1. Parses command-line arguments for the minimum number of processes, maximum number of processes, Supervisor configuration file path, and SQS queue URL.
2. Loads AWS configuration and creates an SQS client.
3. Enters an infinite loop where it:
    - Retrieves the current CPU usage.
    - Retrieves the current SQS queue length.
    - If CPU usage is below the threshold and the queue length exceeds the threshold, it calculates the number of processes to run and updates the Supervisor configuration.
    - Sleeps for 60 seconds before repeating.

## Usage
``` 
cargo run -- --max_num_process=3 --min_num_process=1 --queue_url=https://sqs.ca-central-1.amazonaws.com/XXXXXX/test.fifo --scale_factor=3 --supervisor_config_path=/home/ubuntu/environment/dcm/srv/bean/qscaler/example/sample.conf

# cargo run ${SCALE_FACTOR} ${MIN_NUM_PROCS} ${MAX_NUM_PROCS} ${PROCESS_CONFIG_PATH} ${SQS_URL}
```
Create test messages
```
python3 ./test/send_batch_messages.py https://sqs.ca-central-1.amazonaws.com/XXXXXX/test.fifo 100 ca-central-1
```


## Notes
Make sure the instance role has permission to get messages from AWS SQS.

