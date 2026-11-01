use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

struct RoomMember {
    conn_id: u64,
    peer_id: u32,
    stream: TcpStream,
}

struct Room {
    next_peer_id: u32,
    members: Vec<RoomMember>,
}

#[derive(Default)]
struct RelayState {
    rooms: HashMap<String, Room>,
    latest_room: Option<String>,
    conn_to_room: HashMap<u64, (String, u32)>,
}

fn resolve_project_root(explicit_arg: Option<String>) -> PathBuf {
    if let Some(arg) = explicit_arg {
        let p = PathBuf::from(arg);
        if p.join("web").join("index.html").exists() {
            return p;
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("web").join("index.html").exists() {
            return cwd;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent();
        while let Some(dir) = cur {
            if dir.join("web").join("index.html").exists() {
                return dir.to_path_buf();
            }
            cur = dir.parent();
        }
    }

    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest);
        if p.exists() {
            return p;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args
        .next()
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("PORT").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(8080);
    let root = resolve_project_root(args.next());

    let listener = TcpListener::bind(("0.0.0.0", port))
        .unwrap_or_else(|e| panic!("Failed to bind port {port}: {e}"));

    println!("============================================================");
    println!(
        "  Crimson Ledger — WebAssembly & Co-op Relay Server ({}/{})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("  Serving root: {}", root.display());
    println!("  Open in browser: http://127.0.0.1:{port}");
    println!("============================================================");
    let _ = std::io::stdout().flush();

    let state = Arc::new(Mutex::new(RelayState::default()));
    let mut next_conn_id: u64 = 1;

    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        let conn_id = next_conn_id;
        next_conn_id += 1;
        let root_clone = root.clone();
        let state_clone = state.clone();

        thread::spawn(move || {
            handle_connection(conn_id, stream, &root_clone, state_clone);
        });
    }
}

fn handle_connection(
    conn_id: u64,
    mut stream: TcpStream,
    root: &Path,
    state: Arc<Mutex<RelayState>>,
) {
    let _ = stream.set_nodelay(true);
    let mut req_vec = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        let Ok(n) = stream.read(&mut chunk) else { return };
        if n == 0 {
            break;
        }
        req_vec.extend_from_slice(&chunk[..n]);
        if req_vec.windows(4).any(|w| w == b"\r\n\r\n") || req_vec.len() >= 16384 {
            break;
        }
    }
    if req_vec.is_empty() {
        return;
    }

    let req_str = String::from_utf8_lossy(&req_vec);
    let mut lines = req_str.lines();
    let Some(first_line) = lines.next() else { return };
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1].split('?').next().unwrap_or("/");

    let mut ws_key: Option<String> = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("sec-websocket-key") {
                ws_key = Some(v.trim().to_string());
            }
        }
    }

    if path == "/ws" {
        if let Some(key) = ws_key {
            let accept = compute_ws_accept(&key);
            let resp = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
                 Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\
                 Sec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            if stream.write_all(resp.as_bytes()).is_ok() {
                run_ws_loop(conn_id, stream, state);
            }
            return;
        }
    }

    serve_static_file(&mut stream, root, method, path);
}

fn serve_static_file(stream: &mut TcpStream, root: &Path, method: &str, url_path: &str) {
    let clean = url_path.trim_start_matches('/');
    if clean.contains("..") {
        let _ = stream.write_all(
            b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        return;
    }

    let file_path = if clean.is_empty() || clean == "index.html" {
        root.join("web").join("index.html")
    } else if clean.starts_with("assets/") {
        root.join(clean)
    } else {
        let candidate = root.join("web").join(clean);
        if candidate.exists() {
            candidate
        } else {
            root.join("assets").join(clean)
        }
    };

    let Ok(bytes) = fs::read(&file_path) else {
        let msg = b"Not Found";
        let hdr = format!(
            "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
            msg.len()
        );
        let _ = stream.write_all(hdr.as_bytes());
        if !method.eq_ignore_ascii_case("HEAD") {
            let _ = stream.write_all(msg);
        }
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        return;
    };

    let mime = match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ron" | "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };

    let hdr = format!(
        "HTTP/1.1 200 OK\r\n\
         Connection: close\r\n\
         Content-Type: {mime}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-cache\r\n\
         Access-Control-Allow-Origin: *\r\n\r\n",
        bytes.len()
    );
    let _ = stream.write_all(hdr.as_bytes());
    if !method.eq_ignore_ascii_case("HEAD") {
        let _ = stream.write_all(&bytes);
    }
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Write);
}

fn run_ws_loop(conn_id: u64, mut stream: TcpStream, state: Arc<Mutex<RelayState>>) {
    while let Some((opcode, payload)) = read_ws_frame(&mut stream) {
        match opcode {
            0x8 => break, // Close
            0x9 => {
                // Ping -> Pong
                let _ = write_ws_frame(&mut stream, 0xA, &payload);
            }
            0x1 => {
                let Ok(text) = String::from_utf8(payload) else { continue };
                handle_ws_message(conn_id, &mut stream, &text, &state);
            }
            _ => {}
        }
    }

    // Cleanup on disconnect and notify room partners!
    let mut guard = state.lock().unwrap();
    if let Some((room_code, peer_id)) = guard.conn_to_room.remove(&conn_id) {
        if let Some(room) = guard.rooms.get_mut(&room_code) {
            room.members.retain(|m| m.conn_id != conn_id);
            let leave_msg = format!("PKT|PeerLeft(peer_id:{peer_id})");
            for m in &mut room.members {
                let _ = write_ws_frame(&mut m.stream, 0x1, leave_msg.as_bytes());
            }
            if room.members.is_empty() {
                guard.rooms.remove(&room_code);
            }
        }
    }
}

fn handle_ws_message(
    conn_id: u64,
    stream: &mut TcpStream,
    text: &str,
    state: &Arc<Mutex<RelayState>>,
) {
    let mut guard = state.lock().unwrap();

    if let Some(code) = text.strip_prefix("HOST|") {
        let room_code = code.trim().to_ascii_uppercase();
        let Ok(cloned) = stream.try_clone() else { return };
        guard.rooms.insert(
            room_code.clone(),
            Room {
                next_peer_id: 2,
                members: vec![RoomMember {
                    conn_id,
                    peer_id: 1,
                    stream: cloned,
                }],
            },
        );
        guard.latest_room = Some(room_code.clone());
        guard.conn_to_room.insert(conn_id, (room_code.clone(), 1));
        let _ = write_ws_frame(stream, 0x1, format!("HOSTED|{room_code}").as_bytes());
    } else if let Some(raw_code) = text.strip_prefix("JOIN|") {
        let req = raw_code.trim().to_ascii_uppercase();
        let target_code = if req == "LOCAL" {
            match guard.latest_room.clone() {
                Some(c) => c,
                None => {
                    let _ = write_ws_frame(
                        stream,
                        0x1,
                        b"ERROR|No hosted room yet! Click 'Host Investigation' in Tab 1 first.",
                    );
                    return;
                }
            }
        } else {
            req
        };

        let Some(room) = guard.rooms.get_mut(&target_code) else {
            let _ = write_ws_frame(
                stream,
                0x1,
                format!("ERROR|Room '{target_code}' not found.").as_bytes(),
            );
            return;
        };

        let pid = room.next_peer_id;
        room.next_peer_id += 1;
        let Ok(cloned) = stream.try_clone() else { return };

        // Notify Host (peer_id == 1) that a detective joined so Host sends Welcome + clues
        let joined_notice = format!("PEER_JOINED|{pid}");
        for m in &mut room.members {
            if m.peer_id == 1 {
                let _ = write_ws_frame(&mut m.stream, 0x1, joined_notice.as_bytes());
            }
        }

        room.members.push(RoomMember {
            conn_id,
            peer_id: pid,
            stream: cloned,
        });
        guard.conn_to_room.insert(conn_id, (target_code.clone(), pid));
        let _ = write_ws_frame(
            stream,
            0x1,
            format!("JOINED|{target_code}|{pid}").as_bytes(),
        );
    } else if text.starts_with("PKT|") {
        if let Some((room_code, _my_pid)) = guard.conn_to_room.get(&conn_id).cloned() {
            if let Some(room) = guard.rooms.get_mut(&room_code) {
                for m in &mut room.members {
                    if m.conn_id != conn_id {
                        let _ = write_ws_frame(&mut m.stream, 0x1, text.as_bytes());
                    }
                }
            }
        }
    }
}

fn read_ws_frame(stream: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut hdr = [0u8; 2];
    stream.read_exact(&mut hdr).ok()?;
    let opcode = hdr[0] & 0x0F;
    let masked = (hdr[1] & 0x80) != 0;
    let mut len = (hdr[1] & 0x7F) as usize;

    if len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext).ok()?;
        len = u16::from_be_bytes(ext) as usize;
    } else if len == 127 {
        let mut ext = [0u8; 8];
        stream.read_exact(&mut ext).ok()?;
        len = u64::from_be_bytes(ext) as usize;
    }

    if len > 65_536 {
        return None;
    }

    let mut mask = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask).ok()?;
    }

    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).ok()?;
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i & 3];
        }
    }

    Some((opcode, payload))
}

fn write_ws_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(10 + payload.len());
    frame.push(0x80 | (opcode & 0x0F));
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else {
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)
}

// --- RFC 6455 SHA-1 + BASE64 HANDSHAKE ---
fn compute_ws_accept(client_key: &str) -> String {
    let mut input = client_key.as_bytes().to_vec();
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let digest = sha1(&input);
    base64_encode(&digest)
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6455_accept_key_matches_spec_vector() {
        assert_eq!(
            compute_ws_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
