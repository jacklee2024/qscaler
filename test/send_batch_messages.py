#!/usr/bin/env python3
import boto3
import json
import sys

def send_batch_messages(queue_url, messages, region):
    # Create SQS client with specified region
    sqs = boto3.client('sqs', region_name=region)

    # Split messages into batches of 10 (SQS batch limit)
    for i in range(0, len(messages), 10):
        batch = messages[i:i + 10]
        entries = [
            {
                'Id': str(index),
                'MessageBody': json.dumps(message),
                'MessageGroupId': 'MessageGroupId-' + str(index)
            } for index, message in enumerate(batch)
        ]
        
        # Send batch messages
        response = sqs.send_message_batch(
            QueueUrl=queue_url,
            Entries=entries
        )
        
        if 'Failed' in response:
            print(f"Failed to send messages: {response['Failed']}")
        else:
            print(f"Successfully sent batch of {len(entries)} messages")

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("Usage: python send_batch_messages.py <QUEUE_URL> <NUM_MESSAGES> <REGION>")
        sys.exit(1)

    queue_url = sys.argv[1]
    num_messages = int(sys.argv[2])
    region = sys.argv[3]

    # Generate sample messages
    messages = [{'message': f'This is message {i}'} for i in range(num_messages)]

    send_batch_messages(queue_url, messages, region)