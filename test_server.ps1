$env:RUST_LOG = "info"
$env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"
& ".\target\release\encapure.exe"
