#!/usr/bin/env python3
"""独立端到端验证用的 mock CDN：真实的 HTTP 服务器，故意在首个 Range 请求上截断连接，
复现"网络抖动导致分片不完整"的场景，用来验证下载引擎的校验+重试+续传是否真的工作。"""
import hashlib
import os
import re
import socket
import socketserver
import sys
import threading

SIZE = 12 * 1024 * 1024  # 12MB → 走多分片路径
PAYLOAD = bytes((i * 7 + (i >> 8)) % 251 for i in range(SIZE))
PAYLOAD_HASH = hashlib.sha256(PAYLOAD).hexdigest()

# 行为开关（由命令行参数控制）
MODE = sys.argv[1] if len(sys.argv) > 1 else 'truncate-once'
truncated_once = threading.Event()
stats = {'range_requests': 0, 'truncated': 0, 'full': 0}


def dump_stats(tag=''):
    import json
    with open('mock_stats.json', 'w') as f:
        json.dump({'tag': tag, **stats}, f)


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        raw = b''
        while b'\r\n\r\n' not in raw:
            chunk = self.request.recv(4096)
            if not chunk:
                return
            raw += chunk
        head = raw.decode('latin-1')
        first_line = head.split('\r\n')[0]
        m = re.search(r'Range:\s*bytes=(\d+)-(\d*)', head, re.I)

        if m:
            stats['range_requests'] += 1
            dump_stats('range')
            start = int(m.group(1))
            end = int(m.group(2)) if m.group(2) else SIZE - 1
            end = min(end, SIZE - 1)
            body = PAYLOAD[start:end + 1]

            should_truncate = (
                (MODE == 'truncate-once' and not truncated_once.is_set())
                or MODE == 'truncate-always'
            )
            if should_truncate:
                truncated_once.set()
                stats['truncated'] += 1
                dump_stats('truncated')
                cut = min(4096, len(body))
                hdr = (
                    f"HTTP/1.1 206 Partial Content\r\n"
                    f"Content-Length: {len(body)}\r\n"
                    f"Content-Range: bytes {start}-{end}/{SIZE}\r\n"
                    f"\r\n"
                ).encode()
                self.request.sendall(hdr)
                self.request.sendall(body[:cut])
                # 声明了完整长度，只发 4KB 就硬断 —— 模拟传输中断
                self.request.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER,
                                        b'\x01\x00\x00\x00\x00\x00\x00\x00')
                self.request.close()
                return

            stats['full'] += 1
            dump_stats('full')
            hdr = (
                f"HTTP/1.1 206 Partial Content\r\n"
                f"Content-Length: {len(body)}\r\n"
                f"Content-Range: bytes {start}-{end}/{SIZE}\r\n"
                f"\r\n"
            ).encode()
            self.request.sendall(hdr + body)
        else:
            hdr = (
                f"HTTP/1.1 200 OK\r\n"
                f"Content-Length: {SIZE}\r\n"
                f"Accept-Ranges: bytes\r\n"
                f"\r\n"
            ).encode()
            self.request.sendall(hdr + PAYLOAD)


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == '__main__':
    port = int(os.environ.get('MOCK_PORT', '0'))
    srv = Server(('127.0.0.1', port), Handler)
    actual_port = srv.server_address[1]
    print(f"LISTENING {actual_port}", flush=True)
    print(f"SHA256 {PAYLOAD_HASH}", flush=True)
    print(f"SIZE {SIZE}", flush=True)
    print(f"MODE {MODE}", flush=True)
    dump_stats('boot')
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        dump_stats('exit')
    
