#!/usr/bin/env python3
"""
Generate a formatted release body with download links, SHA-256 checksums,
and links to GPG signatures hosted on GitHub Pages.
"""
import sys
import os
import hashlib
from pathlib import Path


def calculate_sha256(filepath):
    """Calculate SHA-256 checksum of a file."""
    sha256_hash = hashlib.sha256()
    with open(filepath, "rb") as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    return sha256_hash.hexdigest()


def format_file_size(size_bytes):
    """Format file size in human-readable format."""
    for unit in ['B', 'KB', 'MB', 'GB']:
        if size_bytes < 1024.0:
            return f"{size_bytes:.1f} {unit}"
        size_bytes /= 1024.0
    return f"{size_bytes:.1f} TB"


def get_file_description(filename):
    """Get a user-friendly description for each file type."""
    if filename.endswith('.deb'):
        return "Debian/Ubuntu Package"
    elif filename.endswith('.rpm'):
        return "Fedora/RHEL Package"
    elif filename.endswith('.exe'):
        # Distinguish between setup and portable executables
        if 'setup' in filename.lower():
            return "Windows Installer"
        elif 'portable' in filename.lower():
            return "Windows Portable"
        else:
            return "Windows Executable"
    return "Package"


def generate_release_body(artifacts_dir, version, github_pages_url, github_repo_url):
    """
    Generate markdown release body with file information.
    
    Args:
        artifacts_dir: Directory containing release artifacts
        version: Release version (e.g., "0.1.1")
        github_pages_url: Base URL for GitHub Pages (e.g., "https://moooozi.github.io/switchboot")
        github_repo_url: GitHub repository URL (e.g., "https://github.com/moooozi/switchboot")
    """
    artifacts_path = Path(artifacts_dir)
    
    # Find all release files (exclude .asc files)
    release_files = sorted([
        f for f in artifacts_path.iterdir() 
        if f.is_file() and not f.name.endswith('.asc')
    ])
    
    if not release_files:
        print("No release files found in artifacts directory", file=sys.stderr)
        return ""
    
    # Build the release body
    body_parts = [
        f"## Version {version} released!",
        ""
    ]
    
    for file_path in release_files:
        filename = file_path.name
        file_size = format_file_size(file_path.stat().st_size)
        description = get_file_description(filename)
        sha256 = calculate_sha256(file_path)
        
        # Check if signature file exists
        sig_file = artifacts_path / f"{filename}.asc"
        sig_exists = sig_file.exists()
        
        # Create download URL for the file
        file_url = f"{github_repo_url}/releases/download/v{version}/{filename}"
        
        # Create a compact entry with clear separation
        body_parts.append("---")
        body_parts.append("")
        body_parts.append(f"**[`{filename}`]({file_url})**")
        body_parts.append(f"*{description}* • {file_size}")
        body_parts.append("")
        body_parts.append(f"**SHA-256:** `{sha256}`")
        
        if sig_exists:
            sig_url = f"{github_pages_url}/signatures/{version}/{filename}.asc"
            body_parts.append(f"**[GPG Signature]({sig_url})**")
        
        body_parts.append("")
    
    return "\n".join(body_parts)

 
def main():
    if len(sys.argv) < 5:
        print(f"Usage: {sys.argv[0]} <artifacts_dir> <version> <github_pages_url> <github_repo_url>", file=sys.stderr)
        print(f"Example: {sys.argv[0]} artifacts 0.1.1 https://moooozi.github.io/switchboot https://github.com/moooozi/switchboot", file=sys.stderr)
        sys.exit(1)
    
    artifacts_dir = sys.argv[1]
    version = sys.argv[2]
    github_pages_url = sys.argv[3].rstrip('/')
    github_repo_url = sys.argv[4].rstrip('/')
    
    if not os.path.isdir(artifacts_dir):
        print(f"Error: Artifacts directory not found: {artifacts_dir}", file=sys.stderr)
        sys.exit(1)
    
    body = generate_release_body(artifacts_dir, version, github_pages_url, github_repo_url)
    print(body)


if __name__ == "__main__":
    main()
