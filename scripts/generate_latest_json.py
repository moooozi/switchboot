#!/usr/bin/env python3
import json
import sys
import datetime
import os

if len(sys.argv) != 3:
    print("Usage: generate_latest_json.py <version> <pages_base_url>")
    sys.exit(1)

version = sys.argv[1]
pages_base_url = sys.argv[2]

# Get current date in RFC 3339 format
pub_date = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')

# Read the signature from the sig file
sig_file_path = f"repo/updates/{version}/Switchboot_x64-setup.exe.sig"
if os.path.exists(sig_file_path):
    with open(sig_file_path, 'r') as f:
        lines = f.readlines()
        if len(lines) >= 2:
            signature = lines[1].strip()
        else:
            print("Error: Sig file does not have enough lines")
            sys.exit(1)
else:
    print(f"Error: Sig file not found at {sig_file_path}")
    sys.exit(1)

data = {
    "version": version,
    "pub_date": pub_date,
    "platforms": {
        "windows-x86_64": {
            "signature": signature,
            "url": f"https://github.com/moooozi/switchboot/releases/download/v{version}/Switchboot_x64-setup.exe"
        }
    }
}

with open('repo/updates/latest.json', 'w') as f:
    json.dump(data, f)

print(f"Generated latest.json for version {version}")
