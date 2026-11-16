"""Local LAN echo fixture. No unsolicited traffic; only echoes valid test packets."""
import argparse
import socket
import time


def valid(data):
    return len(data) == 256 and data[:4] == b"TGW1" and data[20:] == b"\xa5" * 236


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--bind", default="0.0.0.0")
    p.add_argument("--port", type=int, default=9000)
    args = p.parse_args()
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as server:
        server.bind((args.bind, args.port))
        server.settimeout(1)
        count = rejected = 0
        last = time.monotonic()
        print(f"Listening on {args.bind}:{args.port}; Ctrl+C stops", flush=True)
        while True:
            try:
                data, source = server.recvfrom(257)
                if valid(data):
                    server.sendto(data, source)
                    count += 1
                else:
                    rejected += 1
            except socket.timeout:
                pass
            if time.monotonic() - last >= 1:
                print(f"echoed={count} rejected={rejected}", flush=True)
                last = time.monotonic()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
