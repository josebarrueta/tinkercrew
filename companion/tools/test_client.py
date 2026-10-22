#!/usr/bin/env python3
"""Minimal authenticated Companion client for UNO Q physical acceptance tests."""

import argparse
import json
import ssl
import time
import urllib.request


def request(base, token, path, body=None, insecure=False):
    data = json.dumps(body).encode() if body is not None else None
    headers = {"Authorization": f"Bearer {token}"}
    if data:
        headers["Content-Type"] = "application/json"
    context = ssl._create_unverified_context() if insecure else None  # Pairing client must use the pinned certificate instead.
    with urllib.request.urlopen(urllib.request.Request(base + path, data=data, headers=headers), context=context, timeout=5) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("base", help="for example https://uno-q.local:7443")
    parser.add_argument("token")
    parser.add_argument("--insecure", action="store_true", help="physical test only; production must pin the Board certificate")
    parser.add_argument("--watch", action="store_true", help="poll button and distance events")
    args = parser.parse_args()

    print(json.dumps(request(args.base, args.token, "/v1/hello", insecure=args.insecure), indent=2))
    if not args.watch:
        return
    sequence = 0
    while True:
        request(args.base, args.token, "/v1/control-lease", {"lease_ms": 1500}, args.insecure)
        result = request(args.base, args.token, f"/v1/events?after={sequence}", insecure=args.insecure)
        for event in result["events"]:
            sequence = max(sequence, event["sequence"])
            print(json.dumps(event), flush=True)
        time.sleep(0.1)


if __name__ == "__main__":
    main()
