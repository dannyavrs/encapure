#!/bin/bash
# Encapure - Download Models from GitHub Release
# Run this after cloning the repository to fetch the pre-quantized ONNX models.

set -e

REPO_OWNER="dannz0"
REPO_NAME="encapure"
VERSION="${1:-latest}"

echo ""
echo "=========================================================================="
echo "  Encapure - Model Downloader"
echo "=========================================================================="
echo ""

# Resolve release URL
if [ "$VERSION" = "latest" ]; then
    RELEASE_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
else
    RELEASE_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/tags/$VERSION"
fi

echo -n "Fetching release info... "
RELEASE_JSON=$(curl -s -H "User-Agent: encapure-downloader" "$RELEASE_URL")

TAG=$(echo "$RELEASE_JSON" | grep -o '"tag_name":"[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$TAG" ]; then
    echo "FAILED"
    echo ""
    echo "Could not fetch release from GitHub."
    echo "Make sure the release exists at:"
    echo "  https://github.com/$REPO_OWNER/$REPO_NAME/releases"
    echo ""
    echo "You can also download models manually:"
    echo "  1. Go to the Releases page on GitHub"
    echo "  2. Download 'models.tar.gz'"
    echo "  3. Extract it in the project root: tar -xzf models.tar.gz"
    exit 1
fi
echo "OK ($TAG)"

# Find download URL for models.tar.gz
DOWNLOAD_URL=$(echo "$RELEASE_JSON" | grep -o '"browser_download_url":"[^"]*models\.tar\.gz"' | cut -d'"' -f4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "ERROR: No 'models.tar.gz' asset found in release $TAG"
    exit 1
fi

echo -n "Downloading models... "
curl -sL -H "User-Agent: encapure-downloader" -o models.tar.gz "$DOWNLOAD_URL"
echo "OK"

echo -n "Extracting models... "
tar -xzf models.tar.gz
echo "OK"

rm -f models.tar.gz

# Verify
echo ""
echo "Verifying model files:"

ALL_PRESENT=true
for FILE in "models/model_int8.onnx" "models/tokenizer.json" "bi-encoder-model/model_int8.onnx" "bi-encoder-model/tokenizerbiencoder.json"; do
    if [ -f "$FILE" ]; then
        SIZE=$(du -h "$FILE" | cut -f1)
        echo "  [OK] $FILE ($SIZE)"
    else
        echo "  [MISSING] $FILE"
        ALL_PRESENT=false
    fi
done

echo ""
if [ "$ALL_PRESENT" = true ]; then
    echo "All models downloaded successfully."
    echo "You can now build and start the server:"
    echo ""
    echo "  cargo build --release"
    echo "  ENCAPURE_MODE=single TOOLS_PATH=tests/data/comprehensive_mock_tools.json ./target/release/encapure"
else
    echo "Some model files are missing. Check the release archive."
    exit 1
fi
echo ""
