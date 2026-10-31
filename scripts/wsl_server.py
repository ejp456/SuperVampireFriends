#!/usr/bin/env python3
"""
Linux / WSL HTTP + RFC 6455 WebSocket Co-op Relay Server for Crimson Ledger.
Used automatically by run_wsl_server.sh if rustc is not installed inside WSL.
"""
import base64
import hashlib
import os
import platform
import socket
import struct
import sys
import threading

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

MIME_TYPES = {
    ".html": "text/html; charset=utf-8",
    ".js": "application/javascript; charset=utf-8",
    ".wasm": "application/wasm",
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".ron": "text/plain; charset=utf-8",
    ".txt": "text/plain; charset=utf-8",
}


class RelayState:
    def __init__(self):
        self.lock = threading.Lock()
        self.rooms = {}          # room_code -> {"next_peer_id": int, "members": list of (conn_id, peer_id, sock)}
        self.latest_room = None
        self.conn_to_room = {}   # conn_id -> (room_code, peer_id)


def compute_ws_accept(key: str) -> str:
    digest = hashlib.sha1((key + WS_GUID).encode("utf-8")).digest()
    return base64.b64encode(digest).decode("ascii")


def recv_exact(sock: socket.socket, n: int):
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            return None
        buf.extend(chunk)
    return bytes(buf)


def read_ws_frame(sock: socket.socket):
    hdr = recv_exact(sock, 2)
    if not hdr:
        return None
    opcode = hdr[0] & 0x0F
    masked = (hdr[1] & 0x80) != 0
    length = hdr[1] & 0x7F
    if length == 126:
        ext = recv_exact(sock, 2)
        if not ext:
            return None
        length = struct.unpack("!H", ext)[0]
    elif length == 127:
        ext = recv_exact(sock, 8)
        if not ext:
            return None
        length = struct.unpack("!Q", ext)[0]

    mask_key = b"\x00\x00\x00\x00"
    if masked:
        mk = recv_exact(sock, 4)
        if not mk:
            return None
        mask_key = mk

    if length > 2 * 1024 * 1024:
        return None
    payload = bytearray(recv_exact(sock, length) if length > 0 else b"")
    if payload is None:
        return None
    if masked:
        for i in range(len(payload)):
            payload[i] ^= mask_key[i % 4]
    return opcode, bytes(payload)


def write_ws_frame(sock: socket.socket, opcode: int, payload: bytes):
    hdr = bytearray([0x80 | (opcode & 0x0F)])
    n = len(payload)
    if n < 126:
        hdr.append(n)
    elif n <= 65535:
        hdr.append(126)
        hdr.extend(struct.pack("!H", n))
    else:
        hdr.append(127)
        hdr.extend(struct.pack("!Q", n))
    try:
        sock.sendall(bytes(hdr) + payload)
    except OSError:
        pass


def handle_ws_message(conn_id: int, sock: socket.socket, text: str, state: RelayState):
    with state.lock:
        if text.startswith("HOST|"):
            room_code = text[5:].strip().upper()
            state.rooms[room_code] = {
                "next_peer_id": 2,
                "members": [(conn_id, 1, sock)],
            }
            state.latest_room = room_code
            state.conn_to_room[conn_id] = (room_code, 1)
            write_ws_frame(sock, 0x1, f"HOSTED|{room_code}".encode("utf-8"))
        elif text.startswith("JOIN|"):
            req = text[5:].strip().upper()
            if req == "LOCAL":
                if not state.latest_room:
                    write_ws_frame(
                        sock,
                        0x1,
                        b"ERROR|No hosted room yet! Click 'Host Investigation' in Tab 1 first.",
                    )
                    return
                target_code = state.latest_room
            else:
                target_code = req

            room = state.rooms.get(target_code)
            if not room:
                write_ws_frame(sock, 0x1, f"ERROR|Room '{target_code}' not found.".encode("utf-8"))
                return

            pid = room["next_peer_id"]
            room["next_peer_id"] += 1
            joined_notice = f"PEER_JOINED|{pid}".encode("utf-8")
            for _, m_pid, m_sock in room["members"]:
                if m_pid == 1:
                    write_ws_frame(m_sock, 0x1, joined_notice)

            room["members"].append((conn_id, pid, sock))
            state.conn_to_room[conn_id] = (target_code, pid)
            write_ws_frame(sock, 0x1, f"JOINED|{target_code}|{pid}".encode("utf-8"))
        elif text.startswith("PKT|"):
            info = state.conn_to_room.get(conn_id)
            if info:
                room_code, _ = info
                room = state.rooms.get(room_code)
                if room:
                    raw = text.encode("utf-8")
                    for m_cid, _, m_sock in room["members"]:
                        if m_cid != conn_id:
                            write_ws_frame(m_sock, 0x1, raw)


def run_ws_loop(conn_id: int, sock: socket.socket, state: RelayState):
    try:
        while True:
            frame = read_ws_frame(sock)
            if frame is None:
                break
            opcode, payload = frame
            if opcode == 0x8:
                break
            elif opcode == 0x9:
                write_ws_frame(sock, 0xA, payload)
            elif opcode == 0x1:
                try:
                    text = payload.decode("utf-8")
                except UnicodeDecodeError:
                    continue
                handle_ws_message(conn_id, sock, text, state)
    finally:
        with state.lock:
            info = state.conn_to_room.pop(conn_id, None)
            if info:
                room_code, peer_id = info
                room = state.rooms.get(room_code)
                if room:
                    room["members"] = [m for m in room["members"] if m[0] != conn_id]
                    leave_msg = f"PKT|PeerLeft(peer_id:{peer_id})".encode("utf-8")
                    for _, _, m_sock in room["members"]:
                        write_ws_frame(m_sock, 0x1, leave_msg)
                    if not room["members"]:
                        state.rooms.pop(room_code, None)


def serve_static(sock: socket.socket, root: str, method: str, url_path: str):
    clean = url_path.lstrip("/")
    if ".." in clean:
        sock.sendall(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
        return

    if not clean or clean == "index.html":
        file_path = os.path.join(root, "web", "index.html")
    elif clean.startswith("assets/"):
        file_path = os.path.join(root, clean)
    else:
        candidate = os.path.join(root, "web", clean)
        file_path = candidate if os.path.exists(candidate) else os.path.join(root, "assets", clean)

    if not os.path.isfile(file_path):
        msg = b"Not Found"
        hdr = (
            f"HTTP/1.1 404 Not Found\r\n"
            f"Connection: close\r\n"
            f"Content-Type: text/plain\r\n"
            f"Content-Length: {len(msg)}\r\n\r\n"
        ).encode("utf-8")
        sock.sendall(hdr if method.upper() == "HEAD" else hdr + msg)
        return

    with open(file_path, "rb") as f:
        data = f.read()

    _, ext = os.path.splitext(file_path)
    mime = MIME_TYPES.get(ext.lower(), "application/octet-stream")
    hdr = (
        f"HTTP/1.1 200 OK\r\n"
        f"Connection: close\r\n"
        f"Content-Type: {mime}\r\n"
        f"Content-Length: {len(data)}\r\n"
        f"Cache-Control: no-cache\r\n"
        f"Access-Control-Allow-Origin: *\r\n\r\n"
    ).encode("utf-8")
    sock.sendall(hdr)
    if method.upper() != "HEAD":
        sock.sendall(data)


def handle_client(conn_id: int, sock: socket.socket, root: str, state: RelayState):
    try:
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        req = bytearray()
        while b"\r\n\r\n" not in req and len(req) < 16384:
            chunk = sock.recv(2048)
            if not chunk:
                break
            req.extend(chunk)
        if not req:
            return

        req_text = req.decode("utf-8", errors="replace")
        lines = req_text.splitlines()
        if not lines:
            return
        parts = lines[0].split()
        if len(parts) < 2:
            return
        method = parts[0]
        path = parts[1].split("?", 1)[0]

        ws_key = None
        for line in lines[1:]:
            if ":" in line:
                k, v = line.split(":", 1)
                if k.strip().lower() == "sec-websocket-key":
                    ws_key = v.strip()

        if path == "/ws" and ws_key:
            accept = compute_ws_accept(ws_key)
            resp = (
                f"HTTP/1.1 101 Switching Protocols\r\n"
                f"Upgrade: websocket\r\n"
                f"Connection: Upgrade\r\n"
                f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
            ).encode("utf-8")
            sock.sendall(resp)
            run_ws_loop(conn_id, sock, state)
            return

        serve_static(sock, root, method, path)
        try:
            sock.shutdown(socket.SHUT_WR)
        except OSError:
            pass
    except OSError:
        pass
    finally:
        try:
            sock.close()
        except OSError:
            pass


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    root = sys.argv[2] if len(sys.argv) > 2 else os.getcwd()

    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("0.0.0.0", port))
    srv.listen(128)

    print("============================================================")
    print(f"  Crimson Ledger — WSL Linux WebAssembly & Co-op Server ({platform.system().lower()}/{platform.machine()})")
    print(f"  Serving root: {root}")
    print(f"  Open in Windows browser: http://localhost:{port} (or http://127.0.0.1:{port})")
    print("============================================================")
    sys.stdout.flush()

    state = RelayState()
    next_conn_id = 1
    while True:
        client, _ = srv.accept()
        cid = next_conn_id
        next_conn_id += 1
        t = threading.Thread(target=handle_client, args=(cid, client, root, state), daemon=True)
        t.start()


if __name__ == "__main__":
    main()
