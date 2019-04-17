#!/bin/bash
set -x -e

# Cleanup hook on exit
cleanup () {
    docker stop $nginx_cont || true
    docker rm $nginx_cont
}

trap cleanup EXIT

# Start http server
nginx_cont=$(docker create nginx:alpine)
docker cp site-ci.conf $nginx_cont:/etc/nginx/conf.d/default.conf
docker cp serve/. $nginx_cont:/usr/share/nginx/html
docker start $nginx_cont

# Setup tempget
testimage="tempget_test"
(cd .. && docker build -t $testimage -f .circleci/Dockerfile_tests .)

# Execute tests
success=0
for f in $(ls test_templates); do
    if docker run --network container:$nginx_cont --rm $testimage \
              -c "timeout 45 /lib64/ld-linux-x86-64.so.2 /usr/bin/tempget /test_templates/$f"; then
        echo "$f download terminated."
    else
        echo "command exited with error code $?"
        success=1
    fi
done

if [[ "$success" -eq 0 ]]; then
    echo "All downloads terminated."
else
    echo "Some downloads timed out / did not terminate successfully."
    exit 1
fi
