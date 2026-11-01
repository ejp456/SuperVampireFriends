use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket},
    sync::Mutex,
    time::Duration,
};

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::Closure, JsCast};

use crate::{
    cypher::apply_cypher_solved,
    narrative::{CaseLibrary, ClueLedger, NarrativeState},
    senses::{BloodSight, SetSuspectPulseEvent},
    stats::{DetectiveId, DetectiveSheet},
    stories::{ActiveStory, SelectStoryEvent, StorySystems},
    ui::StorySelectModalOpen,
    world::{ActiveScene, CharacterVisualSprite, DetectivePlayer},
};

#[allow(dead_code)]
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
#[allow(dead_code)]
pub const DEFAULT_PORT: u16 = 7878;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetRole {
    #[default]
    Offline,
    Hosting,
    Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetPacket {
    Welcome {
        assigned_id: u32,
        #[serde(default)]
        story_id: String,
        discovered_clues: Vec<String>,
        #[serde(default)]
        solved_cyphers: Vec<String>,
    },
    StoryChanged {
        story_id: String,
    },
    PeerTransform {
        peer_id: u32,
        #[serde(default)]
        character: DetectiveId,
        #[serde(default)]
        scene_id: String,
        x: f32,
        y: f32,
        flip_x: bool,
        moving: bool,
        blood_sight: bool,
    },
    PeerLeft {
        peer_id: u32,
    },
    ClueDiscovered {
        clue_id: String,
    },
    CypherSolved {
        cypher_id: String,
    },
    SuspectPulse {
        npc_id: String,
        target_bpm: f32,
        is_lie_spike: bool,
    },
}

#[derive(Debug, Clone)]
pub struct PeerSnapshot {
    pub character: DetectiveId,
    pub scene_id: String,
    pub x: f32,
    pub y: f32,
    pub flip_x: bool,
    pub moving: bool,
    pub blood_sight: bool,
}

#[derive(Event, Debug, Clone, Copy)]
pub struct HostGameEvent;

#[derive(Event, Debug, Clone)]
pub struct JoinGameEvent(pub String);

#[derive(Event, Debug, Clone, Copy)]
pub struct DisconnectNetEvent;

// ============================================================================
// NATIVE DESKTOP TCP TRANSPORT
// ============================================================================
#[cfg(not(target_arch = "wasm32"))]
struct FramedConn {
    peer_id: u32,
    stream: TcpStream,
    buf: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FramedConn {
    fn new(peer_id: u32, stream: TcpStream) -> Self {
        let _ = stream.set_nonblocking(true);
        let _ = stream.set_nodelay(true);
        Self {
            peer_id,
            stream,
            buf: Vec::with_capacity(1024),
        }
    }

    fn send_packet(&mut self, pkt: &NetPacket) -> bool {
        let Ok(serialized) = ron::to_string(pkt) else {
            return true;
        };
        let mut line = serialized.into_bytes();
        line.push(b'\n');
        match self.stream.write_all(&line) {
            Ok(()) => true,
            Err(e) if e.kind() == ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    fn poll_packets(&mut self) -> (Vec<NetPacket>, bool) {
        let mut out = Vec::new();
        let mut temp = [0u8; 2048];
        let mut alive = true;

        loop {
            match self.stream.read(&mut temp) {
                Ok(0) => {
                    alive = false;
                    break;
                }
                Ok(n) => {
                    self.buf.extend_from_slice(&temp[..n]);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    alive = false;
                    break;
                }
            }
        }

        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
            if let Ok(line_str) = std::str::from_utf8(&line_bytes[..line_bytes.len() - 1]) {
                let trimmed = line_str.trim();
                if !trimmed.is_empty() {
                    if let Ok(pkt) = ron::from_str::<NetPacket>(trimmed) {
                        out.push(pkt);
                    }
                }
            }
        }

        (out, alive)
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum TransportInner {
    None,
    Host {
        listener: TcpListener,
        clients: Vec<FramedConn>,
        next_peer_id: u32,
    },
    Client {
        conn: FramedConn,
    },
}

// ============================================================================
// BROWSER WASM WEBSOCKET TRANSPORT
// ============================================================================
#[cfg(target_arch = "wasm32")]
struct WasmWsClient {
    ws: web_sys::WebSocket,
    inbox: Rc<RefCell<Vec<String>>>,
    pending_handshake: Option<String>,
    _on_msg: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_WS: RefCell<Option<WasmWsClient>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn generate_room_code() -> String {
    let mut s = String::with_capacity(7);
    for i in 0..6 {
        if i == 3 {
            s.push('-');
        }
        let idx = fastrand::usize(0..CROCKFORD.len());
        s.push(CROCKFORD[idx] as char);
    }
    s
}

#[cfg(target_arch = "wasm32")]
fn open_browser_ws(handshake_cmd: String) -> Result<(), String> {
    let win = web_sys::window().ok_or("No browser window")?;
    let loc = win.location();
    let proto = loc.protocol().unwrap_or_else(|_| "http:".to_string());
    let ws_proto = if proto == "https:" { "wss" } else { "ws" };
    let host = loc.host().map_err(|_| "Could not read window.location.host")?;
    let url = format!("{ws_proto}://{host}/ws");

    let ws = web_sys::WebSocket::new(&url)
        .map_err(|e| format!("WebSocket connect error: {e:?}"))?;

    let inbox: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let inbox_clone = inbox.clone();
    let on_msg = Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
        if let Some(txt) = ev.data().as_string() {
            inbox_clone.borrow_mut().push(txt);
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);

    ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));

    WASM_WS.with(|slot| {
        if let Some(old) = slot.borrow_mut().take() {
            let _ = old.ws.close();
        }
        *slot.borrow_mut() = Some(WasmWsClient {
            ws,
            inbox,
            pending_handshake: Some(handshake_cmd),
            _on_msg: on_msg,
        });
    });

    Ok(())
}

#[derive(Resource)]
pub struct NetSession {
    pub role: NetRole,
    pub my_peer_id: u32,
    pub invite_code: String,
    pub status_message: String,
    pub peers: HashMap<u32, PeerSnapshot>,
    #[cfg(not(target_arch = "wasm32"))]
    transport: Mutex<TransportInner>,
    sync_timer: Timer,
    pub(crate) known_clues_sent: usize,
    pub(crate) known_cyphers_sent: usize,
    pub(crate) last_broadcast_story_id: String,
}

impl Default for NetSession {
    fn default() -> Self {
        Self {
            role: NetRole::Offline,
            my_peer_id: 1,
            invite_code: String::new(),
            status_message: "Offline — Host any story or enter a friend's Invite Code.".to_string(),
            peers: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            transport: Mutex::new(TransportInner::None),
            sync_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
            known_clues_sent: 0,
            known_cyphers_sent: 0,
            last_broadcast_story_id: String::new(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn encode_invite_code(addr: SocketAddrV4) -> String {
    let octets = addr.ip().octets();
    let port = addr.port();
    let packed: u64 = ((octets[0] as u64) << 40)
        | ((octets[1] as u64) << 32)
        | ((octets[2] as u64) << 24)
        | ((octets[3] as u64) << 16)
        | (port as u64);

    let mut chars = [0u8; 10];
    for i in 0..10 {
        let shift = (9 - i) * 5;
        let idx = ((packed >> shift) & 0x1F) as usize;
        chars[i] = CROCKFORD[idx];
    }

    format!(
        "{}-{}",
        std::str::from_utf8(&chars[0..5]).unwrap_or(""),
        std::str::from_utf8(&chars[5..10]).unwrap_or("")
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn decode_invite_code(raw: &str) -> Result<SocketAddrV4, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("LOCAL") {
        return Ok(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_PORT));
    }
    if let Ok(sock) = trimmed.parse::<SocketAddrV4>() {
        return Ok(sock);
    }

    let cleaned: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        })
        .collect();

    if cleaned.len() != 10 {
        return Err("Invite code must be 10 characters (e.g. XXXXX-XXXXX) or 'LOCAL'.".to_string());
    }

    let mut packed: u64 = 0;
    for ch in cleaned.chars() {
        let Some(val) = CROCKFORD.iter().position(|&b| b == ch as u8) else {
            return Err(format!("Invalid character '{ch}' in invite code."));
        };
        packed = (packed << 5) | (val as u64);
    }

    if (packed >> 48) != 0 {
        return Err("Invite code checksum out of range.".to_string());
    }

    let ip = Ipv4Addr::new(
        ((packed >> 40) & 0xFF) as u8,
        ((packed >> 32) & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
    );
    let port = (packed & 0xFFFF) as u16;
    if port == 0 {
        return Err("Invite code resolved to invalid port 0.".to_string());
    }

    Ok(SocketAddrV4::new(ip, port))
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_lan_ipv4() -> Ipv4Addr {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(SocketAddr::V4(local)) = sock.local_addr() {
                if !local.ip().is_unspecified() {
                    return *local.ip();
                }
            }
        }
    }
    Ipv4Addr::LOCALHOST
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetSession>()
            .add_event::<HostGameEvent>()
            .add_event::<JoinGameEvent>()
            .add_event::<DisconnectNetEvent>()
            .add_systems(
                Update,
                (handle_net_control_events, poll_network_transport)
                    .chain()
                    .before(StorySystems),
            )
            .add_systems(Update, broadcast_local_state.after(StorySystems));
    }
}

fn handle_net_control_events(
    mut host_ev: EventReader<HostGameEvent>,
    mut join_ev: EventReader<JoinGameEvent>,
    mut disc_ev: EventReader<DisconnectNetEvent>,
    active_story: Res<ActiveStory>,
    ledger: Res<ClueLedger>,
    mut session: ResMut<NetSession>,
) {
    for _ in disc_ev.read() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            *session.transport.lock().unwrap() = TransportInner::None;
        }
        #[cfg(target_arch = "wasm32")]
        {
            WASM_WS.with(|slot| {
                if let Some(old) = slot.borrow_mut().take() {
                    let _ = old.ws.close();
                }
            });
        }
        session.role = NetRole::Offline;
        session.peers.clear();
        session.invite_code.clear();
        session.status_message = "Disconnected.".to_string();
    }

    for _ in host_ev.read() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let listener = TcpListener::bind(("0.0.0.0", DEFAULT_PORT))
                .or_else(|_| TcpListener::bind(("0.0.0.0", 0)));
            match listener {
                Ok(l) => {
                    let _ = l.set_nonblocking(true);
                    let bound_port = l.local_addr().map(|a| a.port()).unwrap_or(DEFAULT_PORT);
                    let lan_ip = detect_lan_ipv4();
                    let code = encode_invite_code(SocketAddrV4::new(lan_ip, bound_port));

                    *session.transport.lock().unwrap() = TransportInner::Host {
                        listener: l,
                        clients: Vec::new(),
                        next_peer_id: 2,
                    };
                    session.role = NetRole::Hosting;
                    session.my_peer_id = 1;
                    session.peers.clear();
                    session.invite_code = code.clone();
                    session.known_clues_sent = ledger.discovered.len();
                    session.known_cyphers_sent = ledger.solved_cyphers.len();
                    session.last_broadcast_story_id = active_story.story_id.clone();
                    session.status_message = format!(
                        "Hosting \"{}\" on port {bound_port}! Code: {code} (or 'LOCAL')",
                        active_story.story_name
                    );
                }
                Err(e) => {
                    session.status_message = format!("Failed to host socket: {e}");
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let code = generate_room_code();
            match open_browser_ws(format!("HOST|{code}")) {
                Ok(()) => {
                    session.role = NetRole::Hosting;
                    session.my_peer_id = 1;
                    session.peers.clear();
                    session.invite_code = code.clone();
                    session.known_clues_sent = ledger.discovered.len();
                    session.known_cyphers_sent = ledger.solved_cyphers.len();
                    session.last_broadcast_story_id = active_story.story_id.clone();
                    session.status_message = format!(
                        "Hosting \"{}\" — Opening Room {code}...",
                        active_story.story_name
                    );
                }
                Err(e) => {
                    session.status_message = e;
                }
            }
        }
    }

    for JoinGameEvent(code_str) in join_ev.read() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let target = match decode_invite_code(code_str) {
                Ok(addr) => addr,
                Err(msg) => {
                    session.status_message = msg;
                    continue;
                }
            };

            match TcpStream::connect_timeout(&SocketAddr::V4(target), Duration::from_millis(2500)) {
                Ok(stream) => {
                    let conn = FramedConn::new(0, stream);
                    *session.transport.lock().unwrap() = TransportInner::Client { conn };
                    session.role = NetRole::Client;
                    session.my_peer_id = 0;
                    session.peers.clear();
                    session.invite_code = encode_invite_code(target);
                    session.known_clues_sent = ledger.discovered.len();
                    session.known_cyphers_sent = ledger.solved_cyphers.len();
                    session.status_message = format!("Connected to case ({})!", session.invite_code);
                }
                Err(e) => {
                    session.status_message = format!("Could not join {target}: {e}");
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let clean = code_str.trim().to_ascii_uppercase();
            if clean.is_empty() {
                continue;
            }
            match open_browser_ws(format!("JOIN|{clean}")) {
                Ok(()) => {
                    session.role = NetRole::Client;
                    session.my_peer_id = 0;
                    session.peers.clear();
                    session.invite_code = clean.clone();
                    session.known_clues_sent = ledger.discovered.len();
                    session.known_cyphers_sent = ledger.solved_cyphers.len();
                    session.status_message = format!("Joining Room {clean}...");
                }
                Err(e) => {
                    session.status_message = e;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn poll_network_transport(
    mut session: ResMut<NetSession>,
    active_story: Res<ActiveStory>,
    mut story_modal: Option<ResMut<StorySelectModalOpen>>,
    (library, blood_sight, mut sheet): (Res<CaseLibrary>, Res<BloodSight>, ResMut<DetectiveSheet>),
    mut ledger: ResMut<ClueLedger>,
    mut nar_state: ResMut<NarrativeState>,
    mut pulse_writer: EventWriter<SetSuspectPulseEvent>,
    mut story_writer: EventWriter<SelectStoryEvent>,
) {
    if session.role == NetRole::Offline {
        return;
    }

    let existing_clues = ledger.discovered.clone();
    let existing_cyphers = ledger.solved_cyphers.clone();
    let hosted_story_id = active_story.story_id.clone();
    let mut incoming_events: Vec<NetPacket> = Vec::new();
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut disconnected_peers: Vec<u32> = Vec::new();
    let mut disconnected_from_host = false;

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut newly_connected_count = 0usize;
        {
            let mut guard = session.transport.lock().unwrap();
            match &mut *guard {
                TransportInner::None => {}
                TransportInner::Host {
                    listener,
                    clients,
                    next_peer_id,
                } => {
                    while let Ok((stream, _addr)) = listener.accept() {
                        let pid = *next_peer_id;
                        *next_peer_id += 1;
                        let mut conn = FramedConn::new(pid, stream);
                        let _ = conn.send_packet(&NetPacket::Welcome {
                            assigned_id: pid,
                            story_id: hosted_story_id.clone(),
                            discovered_clues: existing_clues.clone(),
                            solved_cyphers: existing_cyphers.clone(),
                        });
                        clients.push(conn);
                        newly_connected_count += 1;
                    }

                    let mut relay_queue: Vec<(u32, NetPacket)> = Vec::new();
                    let mut still_open = Vec::with_capacity(clients.len());

                    for mut c in clients.drain(..) {
                        let pid = c.peer_id;
                        let (pkts, alive) = c.poll_packets();
                        for mut p in pkts {
                            if let NetPacket::PeerTransform { ref mut peer_id, .. } = p {
                                *peer_id = pid;
                            }
                            incoming_events.push(p.clone());
                            relay_queue.push((pid, p));
                        }
                        if alive {
                            still_open.push(c);
                        } else {
                            disconnected_peers.push(pid);
                            relay_queue.push((pid, NetPacket::PeerLeft { peer_id: pid }));
                        }
                    }
                    *clients = still_open;

                    for (sender_id, pkt) in relay_queue {
                        for c in clients.iter_mut() {
                            if c.peer_id != sender_id {
                                let _ = c.send_packet(&pkt);
                            }
                        }
                    }
                }
                TransportInner::Client { conn } => {
                    let (pkts, alive) = conn.poll_packets();
                    incoming_events.extend(pkts);
                    if !alive {
                        disconnected_from_host = true;
                    }
                }
            }
        }
        if newly_connected_count > 0 && session.role == NetRole::Hosting {
            session.status_message = format!(
                "Code: {} — Partner joined! ({} connected)",
                session.invite_code,
                session.peers.len() + newly_connected_count
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let mut status_override: Option<String> = None;
        let mut welcome_to_send: Vec<String> = Vec::new();

        WASM_WS.with(|slot| {
            let mut borrow = slot.borrow_mut();
            let Some(client) = borrow.as_mut() else {
                disconnected_from_host = true;
                return;
            };

            let state = client.ws.ready_state();
            if state == web_sys::WebSocket::OPEN {
                if let Some(cmd) = client.pending_handshake.take() {
                    let _ = client.ws.send_with_str(&cmd);
                }
            } else if state == web_sys::WebSocket::CLOSED {
                disconnected_from_host = true;
                return;
            }

            let raw_msgs: Vec<String> = client.inbox.borrow_mut().drain(..).collect();
            for msg in raw_msgs {
                if let Some(code) = msg.strip_prefix("HOSTED|") {
                    status_override = Some(format!(
                        "Hosting Room! Share Invite Code: {code} (or click Quick Join LOCAL in another tab)"
                    ));
                } else if let Some(rest) = msg.strip_prefix("JOINED|") {
                    let parts: Vec<&str> = rest.split('|').collect();
                    if parts.len() == 2 {
                        if let Ok(pid) = parts[1].parse::<u32>() {
                            session.my_peer_id = pid;
                            session.invite_code = parts[0].to_string();
                            status_override = Some(format!(
                                "Joined Room {} as Detective #{pid}!",
                                parts[0]
                            ));
                        }
                    }
                } else if let Some(pid_str) = msg.strip_prefix("PEER_JOINED|") {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        if session.role == NetRole::Hosting {
                            if let Ok(ser) = ron::to_string(&NetPacket::Welcome {
                                assigned_id: pid,
                                story_id: hosted_story_id.clone(),
                                discovered_clues: existing_clues.clone(),
                                solved_cyphers: existing_cyphers.clone(),
                            }) {
                                welcome_to_send.push(format!("PKT|{ser}"));
                            }
                            status_override = Some(format!(
                                "Invite Code: {} — Detective #{pid} joined!",
                                session.invite_code
                            ));
                        }
                    }
                } else if let Some(err_txt) = msg.strip_prefix("ERROR|") {
                    status_override = Some(err_txt.to_string());
                } else if let Some(ron_str) = msg.strip_prefix("PKT|") {
                    if let Ok(pkt) = ron::from_str::<NetPacket>(ron_str.trim()) {
                        incoming_events.push(pkt);
                    }
                }
            }

            for w in welcome_to_send {
                let _ = client.ws.send_with_str(&w);
            }
        });

        if let Some(st) = status_override {
            session.status_message = st;
        }
    }

    if disconnected_from_host {
        #[cfg(not(target_arch = "wasm32"))]
        {
            *session.transport.lock().unwrap() = TransportInner::None;
        }
        session.role = NetRole::Offline;
        session.peers.clear();
        session.status_message = "Connection closed.".to_string();
        return;
    }

    for pid in disconnected_peers {
        session.peers.remove(&pid);
    }

    for pkt in incoming_events {
        match pkt {
            NetPacket::Welcome {
                assigned_id,
                story_id,
                discovered_clues,
                solved_cyphers,
            } => {
                if session.my_peer_id == 0 || session.my_peer_id == assigned_id {
                    session.my_peer_id = assigned_id;
                    if let Some(ref mut sm) = story_modal {
                        sm.0 = false;
                    }
                    if !story_id.is_empty() && story_id != active_story.story_id {
                        story_writer.send(SelectStoryEvent::sync_from_host(
                            story_id.clone(),
                            discovered_clues,
                            solved_cyphers,
                        ));
                    } else {
                        let mut added_any = false;
                        for cid in discovered_clues {
                            if ledger.add_clue(&cid) {
                                added_any = true;
                            }
                        }
                        for cy_id in solved_cyphers {
                            if apply_cypher_solved(
                                &cy_id,
                                &library,
                                blood_sight.active,
                                &mut ledger,
                                &mut sheet,
                                &mut nar_state,
                            ) {
                                added_any = true;
                            }
                        }
                        session.known_clues_sent = ledger.discovered.len();
                        session.known_cyphers_sent = ledger.solved_cyphers.len();
                        if added_any {
                            nar_state.generation += 1;
                        }
                    }
                    session.status_message = format!(
                        "Joined Story as Detective #{assigned_id} (Code: {})",
                        session.invite_code
                    );
                }
            }
            NetPacket::StoryChanged { story_id } => {
                if session.role == NetRole::Client
                    && !story_id.is_empty()
                    && story_id != active_story.story_id
                {
                    story_writer.send(SelectStoryEvent::sync_from_host(
                        story_id,
                        Vec::new(),
                        Vec::new(),
                    ));
                }
            }
            NetPacket::PeerTransform {
                peer_id,
                character,
                scene_id,
                x,
                y,
                flip_x,
                moving,
                blood_sight,
            } => {
                if peer_id != session.my_peer_id && peer_id != 0 {
                    session.peers.insert(
                        peer_id,
                        PeerSnapshot {
                            character,
                            scene_id,
                            x,
                            y,
                            flip_x,
                            moving,
                            blood_sight,
                        },
                    );
                }
            }
            NetPacket::PeerLeft { peer_id } => {
                session.peers.remove(&peer_id);
            }
            NetPacket::ClueDiscovered { clue_id } => {
                if ledger.add_clue(&clue_id) {
                    session.known_clues_sent = ledger.discovered.len();
                    nar_state.generation += 1;
                }
            }
            NetPacket::CypherSolved { cypher_id } => {
                if apply_cypher_solved(
                    &cypher_id,
                    &library,
                    blood_sight.active,
                    &mut ledger,
                    &mut sheet,
                    &mut nar_state,
                ) {
                    session.known_clues_sent = ledger.discovered.len();
                    session.known_cyphers_sent = ledger.solved_cyphers.len();
                }
            }
            NetPacket::SuspectPulse {
                npc_id,
                target_bpm,
                is_lie_spike,
            } => {
                pulse_writer.send(SetSuspectPulseEvent {
                    npc_id,
                    target_bpm,
                    is_lie_spike,
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn broadcast_local_state(
    time: Res<Time>,
    mut session: ResMut<NetSession>,
    active_story: Res<ActiveStory>,
    sheet: Res<DetectiveSheet>,
    active_scene: Res<ActiveScene>,
    ledger: Res<ClueLedger>,
    blood_sight: Res<BloodSight>,
    mut pulse_events: EventReader<SetSuspectPulseEvent>,
    player_q: Query<(&Transform, &DetectivePlayer)>,
    visual_q: Query<&Sprite, With<CharacterVisualSprite>>,
) {
    if session.role == NetRole::Offline {
        return;
    }

    let mut outgoing: Vec<NetPacket> = Vec::new();

    if session.role == NetRole::Hosting && session.last_broadcast_story_id != active_story.story_id {
        session.last_broadcast_story_id = active_story.story_id.clone();
        outgoing.push(NetPacket::StoryChanged {
            story_id: active_story.story_id.clone(),
        });
    }

    if ledger.discovered.len() > session.known_clues_sent {
        for cid in &ledger.discovered[session.known_clues_sent..] {
            outgoing.push(NetPacket::ClueDiscovered {
                clue_id: cid.clone(),
            });
        }
        session.known_clues_sent = ledger.discovered.len();
    }

    if ledger.solved_cyphers.len() > session.known_cyphers_sent {
        for cy_id in &ledger.solved_cyphers[session.known_cyphers_sent..] {
            outgoing.push(NetPacket::CypherSolved {
                cypher_id: cy_id.clone(),
            });
        }
        session.known_cyphers_sent = ledger.solved_cyphers.len();
    }

    for pev in pulse_events.read() {
        outgoing.push(NetPacket::SuspectPulse {
            npc_id: pev.npc_id.clone(),
            target_bpm: pev.target_bpm,
            is_lie_spike: pev.is_lie_spike,
        });
    }

    session.sync_timer.tick(time.delta());
    if session.sync_timer.just_finished() && session.my_peer_id != 0 {
        if let Ok((tf, player)) = player_q.get_single() {
            let flip_x = visual_q.get_single().map(|s| s.flip_x).unwrap_or(false);
            outgoing.push(NetPacket::PeerTransform {
                peer_id: session.my_peer_id,
                character: sheet.character,
                scene_id: active_scene.current_id.clone(),
                x: tf.translation.x,
                y: tf.translation.y,
                flip_x,
                moving: player.is_moving,
                blood_sight: blood_sight.active,
            });
        }
    }

    if outgoing.is_empty() {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut guard = session.transport.lock().unwrap();
        match &mut *guard {
            TransportInner::None => {}
            TransportInner::Host { clients, .. } => {
                for pkt in &outgoing {
                    clients.retain_mut(|c| c.send_packet(pkt));
                }
            }
            TransportInner::Client { conn } => {
                for pkt in &outgoing {
                    let _ = conn.send_packet(pkt);
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        WASM_WS.with(|slot| {
            if let Some(client) = slot.borrow().as_ref() {
                if client.ws.ready_state() == web_sys::WebSocket::OPEN {
                    for pkt in &outgoing {
                        if let Ok(ser) = ron::to_string(pkt) {
                            let _ = client.ws.send_with_str(&format!("PKT|{ser}"));
                        }
                    }
                }
            }
        });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn invite_code_roundtrips_any_ipv4_and_port() {
        let samples = [
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878),
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 42), 7878),
            SocketAddrV4::new(Ipv4Addr::new(100, 94, 210, 17), 54321),
        ];
        for addr in samples {
            let code = encode_invite_code(addr);
            assert_eq!(code.len(), 11);
            let decoded = decode_invite_code(&code).expect("Invite code should decode");
            assert_eq!(decoded, addr);
        }
    }

    #[test]
    fn tcp_host_and_client_exchange_welcome_and_clues() {
        let sample = NetPacket::Welcome {
            assigned_id: 2,
            story_id: "case_02".to_string(),
            discovered_clues: vec!["clue_staged_rug".to_string()],
            solved_cyphers: vec!["cypher_morgue_manifest".to_string()],
        };
        let encoded = format!("{}\n", ron::to_string(&sample).unwrap());
        let parsed: NetPacket = ron::from_str(encoded.trim()).unwrap();
        match parsed {
            NetPacket::Welcome {
                assigned_id,
                story_id,
                discovered_clues,
                solved_cyphers,
            } => {
                assert_eq!(assigned_id, 2);
                assert_eq!(story_id, "case_02");
                assert_eq!(discovered_clues, vec!["clue_staged_rug".to_string()]);
                assert_eq!(solved_cyphers, vec!["cypher_morgue_manifest".to_string()]);
            }
            other => panic!("Unexpected packet: {other:?}"),
        }

        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
            Err(e) => panic!("Unexpected bind error: {e}"),
        };
        listener.set_nonblocking(true).unwrap();
        let addr = match listener.local_addr().unwrap() {
            SocketAddr::V4(v4) => v4,
            _ => unreachable!(),
        };
        let code = encode_invite_code(addr);
        let decoded = decode_invite_code(&code).unwrap();

        let client_stream = match TcpStream::connect(decoded) {
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
            Err(e) => panic!("Unexpected connect error: {e}"),
        };
        let mut client_conn = FramedConn::new(0, client_stream);
        let (server_stream, _) = listener.accept().unwrap();
        let mut host_conn = FramedConn::new(2, server_stream);

        assert!(host_conn.send_packet(&sample));
        std::thread::sleep(Duration::from_millis(15));
        let (pkts, alive) = client_conn.poll_packets();
        assert!(alive);
        assert_eq!(pkts.len(), 1);
    }
}
