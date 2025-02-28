#!/bin/bash

NUM_PROCESSES=5
echo "Starting $NUM_PROCESSES parallel cargo processes..."

declare -a PIDS

run_backend() {
    local port=$((8080 + $1))
    echo "Process $port starting..."
    cargo run --example mock_backend -- -p $port
    if [ $? -eq 0 ]; then
        echo "Process $port completed successfully"
    else
        echo "Process $port failed"
    fi
}

for ((i=0; i<NUM_PROCESSES; i++))
do
    run_backend $i &
    PIDS[$i]=$!
done

# run the load balancer
cargo run -- -c $NUM_PROCESSES & PIDS[$NUM_PROCESSES]=$!

echo "Waiting for all processes to finish..."
for pid in ${PIDS[@]}
do
    wait $pid
done

echo "All processes have completed"
