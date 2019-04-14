#!/bin/bash
set -x -e

# Start http server
nginx_cont="nginx"
docker create --name $nginx_cont nginx:alpine
docker cp site-ci.conf $nginx_cont:/etc/nginx/conf.d/default.conf
docker cp serve/. $nginx_cont:/usr/share/nginx/html
docker start $nginx_cont

# Setup tempget
testimage="tempget_test"
(cd .. && docker build -t $testimage -f .circleci/Dockerfile_tests .)

# Execute tests
for f in $(ls test_templates); do
    docker run --network container:$nginx_cont --rm $testimage \
           -c timeout 45 tempget "/test_templates/$f"
done
