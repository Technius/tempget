#!/bin/bash
set -e
# Enable to debug this script
# set -x

test_case_file="test_cases.json"

# Cleanup hook on exit
cleanup () {
    docker stop "$nginx_cont" || true > /dev/null
    docker rm "$nginx_cont" > /dev/null
}

trap cleanup EXIT

# Start http server
echo "Starting HTTP server"
nginx_cont=$(docker create nginx:alpine)
docker cp site-ci.conf "$nginx_cont":/etc/nginx/conf.d/default.conf
docker cp serve/. "$nginx_cont":/usr/share/nginx/html
docker start "$nginx_cont" > /dev/null

# Setup tempget
testimage="tempget_test"
[[ "$REBUILD_TESTIMAGE" == "false" ]] || \
    (cd .. && docker build -t $testimage -f .circleci/Dockerfile_tests .)

# Execute tests
success=0
for f in test_templates/*; do
    f=$(echo "$f" | cut -d'/' -f2-)
    echo "Testing $f"
    set +e
    output=$(docker run --network container:"$nginx_cont" --rm $testimage \
                    -c "timeout 45 /lib64/ld-linux-x86-64.so.2 /usr/bin/tempget /test_templates/$f 2>&1")
    exit_code=$?
    test_name="$(basename "${f%.*}")"
    expected_result=$(jq ".$test_name.should_succeed" $test_case_file -M)
    [[ $expected_result == "null" ]] && expected_result="true"
    set -e
    printf '%s\n' "$output"

    if [[ $exit_code -eq 0 ]]; then
        result="true"
    elif [[ $exit_code -eq 124 ]]; then
        echo "At least one of the $f downloads timed out."
        success=1
        continue
    elif ! echo "$output" | grep 'downloads failed:' > /dev/null; then
        echo "The $f downloads failed due to some unrelated error"
        success=1
        continue
    else
        result="false"
    fi

    if [[ $expected_result == "$result" ]]; then
        if [[ $expected_result == "true" ]]; then
            echo "$f downloads succeeded, as expected."
        else
            echo "$f downloads failed, as expected."
        fi
    else
        if [[ $expected_result == "false" ]]; then
            echo "$f downloads succeeded when it should have failed."
        else
            echo "$f downloads failed when it should have succeeded."
        fi
        success=1
    fi
done

if [[ "$success" -eq 0 ]]; then
    echo "All downloads terminated with the expected results."
else
    echo "Some downloads timed out / did not terminate with the expected results."
    exit 1
fi

