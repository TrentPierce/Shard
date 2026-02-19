#!/usr/bin/env python3
import sys
import argparse

def main():
    parser = argparse.ArgumentParser(description="Shard Project Management CLI")
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Release Build
    parser_release = subparsers.add_parser("release", help="Build release bundle")
    parser_release.add_argument("--skip-freeze", action="store_true", help="skip pyinstaller step")

    # Sync Versions
    parser_sync = subparsers.add_parser("version", help="Sync versions across the project")
    parser_sync.add_argument("--set", dest="set_version", help="Set VERSION file then sync")

    # Pack repo
    parser_pack = subparsers.add_parser("pack", help="Pack codebase context for LLMs")

    args = parser.parse_args()

    if args.command == "release":
        from scripts import build_release
        # Patch sys.argv for the sub-script
        sys.argv = [sys.argv[0]]
        if args.skip_freeze:
            sys.argv.append("--skip-freeze")
        build_release.main()

    elif args.command == "version":
        from scripts import sync_versions
        sys.argv = [sys.argv[0]]
        if args.set_version:
            sys.argv.extend(["--set", args.set_version])
        sync_versions.main()

    elif args.command == "pack":
        from scripts import pack_shard
        pack_shard.pack_repo()

    else:
        parser.print_help()

if __name__ == "__main__":
    main()
