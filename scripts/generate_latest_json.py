#!/usr/bin/env python3
import json
import sys
import datetime

if len(sys.argv) != 3:
    print("Usage: generate_latest_json.py <version> <pages_base_url>")
    sys.exit(1)

version = sys.argv[1]
pages_base_url = sys.argv[2]

# Get current date in RFC 3339 format
pub_date = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')

data = {
    "version": version,
    "pub_date": pub_date,
    "platforms": {
        "windows-x86_64": {
            "signature": f"{pages_base_url}/updates/{version}/Switchboot_x64-setup.exe.sig",
            "url": f"https://github.com/moooozi/switchboot/releases/download/v{version}/Switchboot_x64-setup.exe"
        }
    }
}

with open('repo/updates/latest.json', 'w') as f:
    json.dump(data, f)

print(f"Generated latest.json for version {version}")
