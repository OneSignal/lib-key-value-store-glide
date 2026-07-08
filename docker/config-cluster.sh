#!/bin/sh

echo "Waiting for Valkey nodes..."
for h in valkey-cluster-1-kvs-glide valkey-cluster-2-kvs-glide valkey-cluster-3-kvs-glide; do
    until valkey-cli -u valkey://default:development_password@${h}:6379/0 ping | grep -q PONG; do
        sleep 1
    done
done

echo "Checking if cluster is already created..."
if valkey-cli -u valkey://default:development_password@valkey-cluster-1-kvs-glide:6379/0 cluster info 2>/dev/null | grep -q "cluster_state:ok"; then
    echo "Cluster already OK, nothing to do."
    exit 0
fi

echo "Creating cluster..."
yes yes | valkey-cli -u valkey://default:development_password@valkey-cluster-1-kvs-glide:6379/0 --cluster create \
    valkey-cluster-1-kvs-glide:6379 valkey-cluster-2-kvs-glide:6379 valkey-cluster-3-kvs-glide:6379

echo "Done."
