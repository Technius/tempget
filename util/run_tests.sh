#!/bin/bash
set -x -e

# Start http server
nginx_cont="nginx"
docker create -p 8080:80 --name $nginx_cont nginx:alpine
docker cp site-ci.conf $nginx_cont:/etc/nginx/conf.d/default.conf
docker cp serve/. $nginx_cont:/usr/share/nginx/html
docker start $nginx_cont

# Setup tempget
app_cont="tempget_app"
docker create --name $app_cont --network container:$nginx_cont debian:stretch /bin/sh -c 'while true; do sleep 1; done'
docker cp ../target/release/tempget $app_cont:/usr/bin/tempget
docker cp test_templates $app_cont:/test_templates
docker start $app_cont
docker exec $app_cont /bin/sh -c 'mkdir /testing && apt-get update && apt-get install openssl'

# Execute tests
for f in $(ls test_templates); do
    tfile="/test_templates/$f"
    docker exec $app_cont /bin/sh -c "cd /testing && tempget \"/test_templates/$f\""
done
