#!/bin/sh
# Uploads dist/* and SHA256SUMS to the generic package registry and creates
# the GitLab release for CI_COMMIT_TAG with links to them.
#
# Runs in the `release` job of .gitlab-ci.yml (needs curl). Uses the job
# token, so no extra credentials are required.
set -eu

tag=$CI_COMMIT_TAG
version=${tag#v}
registry="$CI_API_V4_URL/projects/$CI_PROJECT_ID/packages/generic/sprcmtrd/$version"

cd dist
sha256sum -- * > SHA256SUMS
cat SHA256SUMS

links=""
for file in *; do
    echo "uploading $file"
    curl --fail-with-body --silent --show-error \
        --header "JOB-TOKEN: $CI_JOB_TOKEN" \
        --upload-file "$file" "$registry/$file" > /dev/null
    link_type=package
    [ "$file" = SHA256SUMS ] && link_type=other
    links="$links{\"name\":\"$file\",\"url\":\"$registry/$file\",\"link_type\":\"$link_type\"},"
done
links=${links%,}

description="sprcmtrd $tag

Binaries for Linux x86_64, Linux aarch64 and Windows x86_64, and Debian packages for Linux (\`sudo apt install ./sprcmtrd_*.deb\`). Verify downloads with \`sha256sum --check --ignore-missing SHA256SUMS\`."
# JSON-escape the description (backslashes, quotes, newlines).
description_json=$(printf '%s' "$description" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | awk 'BEGIN{ORS="\\n"} {print}' | sed 's/\\n$//')

curl --fail-with-body --silent --show-error \
    --header "JOB-TOKEN: $CI_JOB_TOKEN" \
    --header "Content-Type: application/json" \
    --data "{\"tag_name\":\"$tag\",\"name\":\"sprcmtrd $tag\",\"description\":\"$description_json\",\"assets\":{\"links\":[$links]}}" \
    "$CI_API_V4_URL/projects/$CI_PROJECT_ID/releases"
echo
echo "release $tag created"
