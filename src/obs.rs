//! OBS Studio over obs-websocket 5, which OBS ships with. Each deck keeps one connection on its own
//! thread, so a slow or missing OBS can never stall the deck.
//! Protocol: https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md
use crate::config::{MediaAction, ObsAction, ObsSettings};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Message, WebSocket};

const RPC_VERSION: u64 = 1;
const OP_HELLO: u64 = 0;
const OP_IDENTIFY: u64 = 1;
const OP_IDENTIFIED: u64 = 2;
const OP_EVENT: u64 = 5;
const OP_REQUEST: u64 = 6;
const OP_RESPONSE: u64 = 7;
/// Config, Scenes, Inputs, Transitions, Filters and SceneItems: whatever changes the name lists.
const EVENTS: u64 = 1 << 1 | 1 << 2 | 1 << 3 | 1 << 4 | 1 << 5 | 1 << 7;
/// These fire constantly and change no name.
const QUIET_EVENTS: [&str; 5] =
    ["CurrentProgramScene", "CurrentPreviewScene", "InputVolume", "InputMute", "InputAudio"];
const CLOSE_AUTH_FAILED: u16 = 4009;

const RETRY: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(3);
/// How long the thread listens for OBS before checking for actions again.
const POLL: Duration = Duration::from_millis(25);
/// One change in OBS often sends a burst of events.
const SETTLE: Duration = Duration::from_millis(300);
/// OBS refuses requests while it switches scene collection.
const NOT_READY_WAIT: Duration = Duration::from_secs(1);

/// What the editor needs: whether to offer OBS actions, and the names to pick from.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ObsState {
    pub connected: bool,
    /// Why it is not connected.
    pub error: String,
    pub scenes: Vec<String>,
    pub collections: Vec<String>,
    pub profiles: Vec<String>,
    pub transitions: Vec<String>,
    pub inputs: Vec<String>,
    /// Scene name to the sources in it.
    pub items: BTreeMap<String, Vec<String>>,
    /// Scene or input name to its filters.
    pub filters: BTreeMap<String, Vec<String>>,
}

enum Job {
    Run(ObsAction),
    Settings(ObsSettings),
}

/// Starts connecting at once and keeps trying for as long as this lives.
pub struct Obs {
    jobs: Sender<Job>,
    state: Arc<Mutex<ObsState>>,
}

impl Obs {
    pub fn start(settings: ObsSettings) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let state = Arc::new(Mutex::new(ObsState::default()));
        let shared = state.clone();
        std::thread::spawn(move || worker(settings, inbox, shared));
        Self { jobs, state }
    }

    pub fn run(&self, action: ObsAction) {
        let _ = self.jobs.send(Job::Run(action));
    }

    pub fn configure(&self, settings: ObsSettings) {
        let _ = self.jobs.send(Job::Settings(settings));
    }

    pub fn state(&self) -> ObsState {
        self.state.lock().unwrap().clone()
    }
}

fn worker(mut settings: ObsSettings, jobs: Receiver<Job>, state: Arc<Mutex<ObsState>>) {
    let mut session: Option<Session> = None;
    let mut pending = None;
    loop {
        if session.is_none() {
            match Session::connect(&settings) {
                Ok(s) => session = Some(s),
                Err(e) => {
                    // Running it later would surprise whoever pressed the key long ago.
                    if pending.take().is_some() {
                        eprintln!("OBS action skipped: {e}");
                    }
                    *state.lock().unwrap() = ObsState { error: e.to_string(), ..Default::default() };
                }
            }
        }
        match jobs.recv_timeout(if session.is_some() { Duration::ZERO } else { RETRY }) {
            Ok(Job::Run(action)) => pending = Some(action),
            Ok(Job::Settings(new)) => {
                settings = new;
                session = None;
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(s) = &mut session else { continue };
        if let Some(action) = pending.take() {
            // A refused request leaves the connection fine; a dead one shows up in pump.
            if let Err(e) = s.perform(&action) {
                eprintln!("OBS action failed: {e:#}");
            }
        }
        if let Err(e) = s.pump(&state) {
            eprintln!("OBS connection lost: {e:#}");
            session = None;
        }
    }
}

/// `base64(sha256(base64(sha256(password + salt)) + challenge))`
fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = STANDARD.encode(Sha256::digest(format!("{password}{salt}")));
    STANDARD.encode(Sha256::digest(format!("{secret}{challenge}")))
}

fn names(list: &Value, key: &str) -> Vec<String> {
    let items = list.as_array().map(Vec::as_slice).unwrap_or_default();
    items.iter().filter_map(|item| Some(item.get(key)?.as_str()?.to_string())).collect()
}

fn strings(list: &Value) -> Vec<String> {
    let items = list.as_array().map(Vec::as_slice).unwrap_or_default();
    items.iter().filter_map(|item| Some(item.as_str()?.to_string())).collect()
}

fn media_id(action: MediaAction) -> &'static str {
    match action {
        MediaAction::Play => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_PLAY",
        MediaAction::Pause => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_PAUSE",
        MediaAction::Stop => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_STOP",
        MediaAction::Restart => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_RESTART",
        MediaAction::Next => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_NEXT",
        MediaAction::Previous => "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_PREVIOUS",
    }
}

fn is_timeout(error: &tungstenite::Error) -> bool {
    use std::io::ErrorKind::{TimedOut, WouldBlock};
    matches!(error, tungstenite::Error::Io(e) if matches!(e.kind(), TimedOut | WouldBlock))
}

fn reachable(settings: &ObsSettings) -> Result<TcpStream> {
    let (host, port) = (settings.host.as_str(), settings.port);
    let not_found = || format!("No OBS WebSocket server at {host}:{port}.");
    let addresses: Vec<SocketAddr> = (host, port).to_socket_addrs().with_context(not_found)?.collect();
    for address in addresses {
        if let Ok(stream) = TcpStream::connect_timeout(&address, TIMEOUT) {
            return Ok(stream);
        }
    }
    bail!(not_found())
}

struct Session {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    /// When to read the name lists again.
    refresh: Option<Instant>,
}

impl Session {
    fn connect(settings: &ObsSettings) -> Result<Self> {
        let stream = reachable(settings)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let url = format!("ws://{}:{}", settings.host, settings.port);
        let mut request = url.into_client_request().context("Not a valid OBS address.")?;
        request.headers_mut().insert("Sec-WebSocket-Protocol", HeaderValue::from_static("obswebsocket.json"));
        let (socket, _) = tungstenite::client(request, stream).map_err(|_| {
            anyhow!("{}:{} did not answer like OBS WebSocket.", settings.host, settings.port)
        })?;
        let mut session = Self { socket, next_id: 0, refresh: Some(Instant::now()) };

        let hello = session.expect(OP_HELLO)?;
        let mut identify = json!({ "rpcVersion": RPC_VERSION, "eventSubscriptions": EVENTS });
        if let Some(auth) = hello.get("authentication") {
            if settings.password.is_empty() {
                bail!("OBS needs the server password.");
            }
            let text = |key| auth.get(key).and_then(Value::as_str).ok_or_else(|| anyhow!("OBS sent no {key}"));
            identify["authentication"] = auth_string(&settings.password, text("salt")?, text("challenge")?).into();
        }
        session.send(OP_IDENTIFY, identify)?;
        session.expect(OP_IDENTIFIED)?;
        session.socket.get_ref().set_read_timeout(Some(POLL))?;
        Ok(session)
    }

    fn send(&mut self, op: u64, data: Value) -> Result<()> {
        self.socket.send(Message::text(json!({ "op": op, "d": data }).to_string()))?;
        Ok(())
    }

    /// One message from OBS, or None if it has nothing to say right now.
    fn receive(&mut self) -> Result<Option<Value>> {
        match self.socket.read() {
            Ok(Message::Text(text)) => Ok(Some(serde_json::from_str(&text)?)),
            Ok(Message::Close(Some(frame))) if frame.code == CloseCode::from(CLOSE_AUTH_FAILED) => {
                bail!("OBS rejected the server password.")
            }
            Ok(Message::Close(frame)) => bail!("OBS closed the connection: {frame:?}"),
            Ok(_) => Ok(None),
            Err(e) if is_timeout(&e) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn note_event(&mut self, message: &Value) {
        let kind = message["d"]["eventType"].as_str().unwrap_or_default();
        if !QUIET_EVENTS.iter().any(|quiet| kind.starts_with(quiet)) {
            self.refresh.get_or_insert_with(|| Instant::now() + SETTLE);
        }
    }

    /// Waits for a message of this kind, keeping track of events that arrive first.
    fn wait_for(&mut self, op: u64, matches: impl Fn(&Value) -> bool) -> Result<Value> {
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            let Some(message) = self.receive()? else { continue };
            if message["op"] == op && matches(&message["d"]) {
                return Ok(message["d"].clone());
            }
            if message["op"] == OP_EVENT {
                self.note_event(&message);
            }
        }
        bail!("OBS did not answer in time")
    }

    fn expect(&mut self, op: u64) -> Result<Value> {
        self.wait_for(op, |_| true)
    }

    fn request(&mut self, kind: &str, data: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id.to_string();
        self.send(OP_REQUEST, json!({ "requestType": kind, "requestId": id, "requestData": data }))?;
        let reply = self.wait_for(OP_RESPONSE, |d| d["requestId"] == id.as_str())?;
        if reply["requestStatus"]["result"] != true {
            bail!("{kind}: {}", reply["requestStatus"]["comment"].as_str().unwrap_or("refused by OBS"));
        }
        Ok(reply["responseData"].clone())
    }

    fn fetch(&mut self) -> Result<ObsState> {
        let scenes = names(&self.request("GetSceneList", json!({}))?["scenes"], "sceneName");
        let inputs = names(&self.request("GetInputList", json!({}))?["inputs"], "inputName");
        let mut items = BTreeMap::new();
        for scene in &scenes {
            let reply = self.request("GetSceneItemList", json!({ "sceneName": scene }))?;
            items.insert(scene.clone(), names(&reply["sceneItems"], "sourceName"));
        }
        let mut filters = BTreeMap::new();
        for source in scenes.iter().chain(&inputs) {
            let reply = self.request("GetSourceFilterList", json!({ "sourceName": source }))?;
            filters.insert(source.clone(), names(&reply["filters"], "filterName"));
        }
        Ok(ObsState {
            connected: true,
            error: String::new(),
            collections: strings(&self.request("GetSceneCollectionList", json!({}))?["sceneCollections"]),
            profiles: strings(&self.request("GetProfileList", json!({}))?["profiles"]),
            transitions: names(&self.request("GetSceneTransitionList", json!({}))?["transitions"], "transitionName"),
            scenes,
            inputs,
            items,
            filters,
        })
    }

    /// Listens to OBS for a moment and refreshes the name lists if something changed.
    fn pump(&mut self, state: &Mutex<ObsState>) -> Result<()> {
        if let Some(message) = self.receive()? {
            if message["op"] == OP_EVENT {
                self.note_event(&message);
            }
        }
        if self.refresh.is_some_and(|at| Instant::now() >= at) {
            // Cleared first, so an event during the fetch asks for another one.
            self.refresh = None;
            match self.fetch() {
                Ok(fresh) => *state.lock().unwrap() = fresh,
                Err(e) => {
                    eprintln!("OBS lists not read: {e:#}");
                    self.refresh = Some(Instant::now() + NOT_READY_WAIT);
                }
            }
        }
        Ok(())
    }

    fn perform(&mut self, action: &ObsAction) -> Result<()> {
        let none = json!({});
        let reply = match action {
            ObsAction::ToggleRecord => self.request("ToggleRecord", none),
            ObsAction::ToggleRecordPause => self.request("ToggleRecordPause", none),
            ObsAction::ToggleStream => self.request("ToggleStream", none),
            ObsAction::ToggleVirtualCam => self.request("ToggleVirtualCam", none),
            ObsAction::ToggleReplayBuffer => self.request("ToggleReplayBuffer", none),
            ObsAction::SaveReplay => self.request("SaveReplayBuffer", none),
            ObsAction::Transition => self.request("TriggerStudioModeTransition", none),
            ObsAction::ToggleStudioMode => {
                let on = self.request("GetStudioModeEnabled", none)?["studioModeEnabled"] == true;
                self.request("SetStudioModeEnabled", json!({ "studioModeEnabled": !on }))
            }
            ObsAction::Screenshot => self.screenshot(),
            // No name lets OBS pick its own numbered one.
            ObsAction::Chapter(name) if name.is_empty() => self.request("CreateRecordChapter", none),
            ObsAction::Chapter(name) => self.request("CreateRecordChapter", json!({ "chapterName": name })),
            ObsAction::Scene(name) => self.request("SetCurrentProgramScene", json!({ "sceneName": name })),
            ObsAction::SceneCollection(name) => {
                self.request("SetCurrentSceneCollection", json!({ "sceneCollectionName": name }))
            }
            ObsAction::Profile(name) => self.request("SetCurrentProfile", json!({ "profileName": name })),
            ObsAction::SceneTransition(name) => {
                self.request("SetCurrentSceneTransition", json!({ "transitionName": name }))
            }
            ObsAction::ToggleSource { scene, source } => {
                let found = json!({ "sceneName": scene, "sourceName": source });
                let id = self.request("GetSceneItemId", found)?["sceneItemId"].clone();
                let item = json!({ "sceneName": scene, "sceneItemId": id });
                let on = self.request("GetSceneItemEnabled", item)?["sceneItemEnabled"] == true;
                let change = json!({ "sceneName": scene, "sceneItemId": id, "sceneItemEnabled": !on });
                self.request("SetSceneItemEnabled", change)
            }
            ObsAction::ToggleFilter { source, filter } => {
                let found = json!({ "sourceName": source, "filterName": filter });
                let on = self.request("GetSourceFilter", found)?["filterEnabled"] == true;
                let change = json!({ "sourceName": source, "filterName": filter, "filterEnabled": !on });
                self.request("SetSourceFilterEnabled", change)
            }
            ObsAction::Media { input, action } => {
                let data = json!({ "inputName": input, "mediaAction": media_id(*action) });
                self.request("TriggerMediaInputAction", data)
            }
            ObsAction::ToggleMute(input) => self.request("ToggleInputMute", json!({ "inputName": input })),
            ObsAction::Volume { input, db } => {
                let current = self.request("GetInputVolume", json!({ "inputName": input }))?;
                // A silent input can report no number at all.
                let level = current["inputVolumeDb"].as_f64().unwrap_or(-100.0);
                let target = (level + *db as f64).clamp(-100.0, 0.0); // a knob should not boost past unity
                self.request("SetInputVolume", json!({ "inputName": input, "inputVolumeDb": target }))
            }
        };
        reply.map(|_| ())
    }

    /// Saves what is on air next to the recordings, so no folder has to be configured.
    fn screenshot(&mut self) -> Result<Value> {
        let scene = self.request("GetCurrentProgramScene", json!({}))?["sceneName"].clone();
        let folder = self.request("GetRecordDirectory", json!({}))?;
        let folder = folder["recordDirectory"].as_str().unwrap_or_default().to_string();
        let stamp = SystemTime::UNIX_EPOCH.elapsed().map_or(0, |d| d.as_secs());
        let file = Path::new(&folder).join(format!("OBS screenshot {stamp}.png"));
        let data = json!({ "sourceName": scene, "imageFormat": "png", "imageFilePath": file });
        self.request("SaveSourceScreenshot", data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Plays OBS for one connection: checks the login, then answers from `reply`, logging requests.
    fn fake_obs(password: &'static str, reply: fn(&str, &Value) -> Value) -> (u16, Receiver<(String, Value)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (log, seen) = mpsc::channel();
        std::thread::spawn(move || {
            let stream = listener.accept().unwrap().0;
            let echo = |request: &tungstenite::handshake::server::Request, mut response: tungstenite::handshake::server::Response| {
                let wanted = request.headers().get("Sec-WebSocket-Protocol").cloned();
                response.headers_mut().insert("Sec-WebSocket-Protocol", wanted.unwrap());
                Ok(response)
            };
            let mut socket = tungstenite::accept_hdr(stream, echo).unwrap();
            let (salt, challenge) = ("c2FsdA==", "Y2hhbGxlbmdl");
            let hello = json!({ "op": 0, "d": { "rpcVersion": 1, "authentication": { "salt": salt, "challenge": challenge } } });
            socket.send(Message::text(hello.to_string())).unwrap();
            while let Ok(Message::Text(text)) = socket.read() {
                let message: Value = serde_json::from_str(&text).unwrap();
                if message["op"] == 1 {
                    if message["d"]["authentication"] != auth_string(password, salt, challenge) {
                        let frame = tungstenite::protocol::CloseFrame { code: CLOSE_AUTH_FAILED.into(), reason: "no".into() };
                        let _ = socket.close(Some(frame));
                        let _ = socket.read();
                        return;
                    }
                    socket.send(Message::text(json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string())).unwrap();
                    continue;
                }
                let d = &message["d"];
                let kind = d["requestType"].as_str().unwrap().to_string();
                let data = reply(&kind, &d["requestData"]);
                let _ = log.send((kind.clone(), d["requestData"].clone()));
                let status = json!({ "result": !data.is_null(), "code": 100 });
                let answer = json!({ "op": 7, "d": { "requestType": kind, "requestId": d["requestId"], "requestStatus": status, "responseData": data } });
                socket.send(Message::text(answer.to_string())).unwrap();
            }
        });
        (port, seen)
    }

    fn settings(port: u16, password: &str) -> ObsSettings {
        ObsSettings { host: "127.0.0.1".into(), port, password: password.into() }
    }

    #[test]
    fn auth_matches_the_formula_in_the_spec() {
        // Worked out separately with Python's hashlib.
        assert_eq!(auth_string("hunter2", "c2FsdA==", "Y2hhbGxlbmdl"), "HbemcTRAK8GnBZpkRzKZdmk94xa5VYtjm6/uKbA1epI=");
    }

    #[test]
    fn wrong_password_is_reported_as_such() {
        let (port, _seen) = fake_obs("right", |_, _| json!({}));
        let error = Session::connect(&settings(port, "wrong")).err().unwrap();
        assert_eq!(error.to_string(), "OBS rejected the server password.");
    }

    #[test]
    fn missing_password_is_reported_as_such() {
        let (port, _seen) = fake_obs("right", |_, _| json!({}));
        let error = Session::connect(&settings(port, "")).err().unwrap();
        assert_eq!(error.to_string(), "OBS needs the server password.");
    }

    #[test]
    fn nothing_listening_is_reported_with_the_address() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let error = Session::connect(&settings(port, "")).err().unwrap();
        assert_eq!(error.to_string(), format!("No OBS WebSocket server at 127.0.0.1:{port}."));
    }

    #[test]
    fn toggling_a_source_reads_then_flips_it() {
        let (port, seen) = fake_obs("pw", |kind, _| match kind {
            "GetSceneItemId" => json!({ "sceneItemId": 7 }),
            "GetSceneItemEnabled" => json!({ "sceneItemEnabled": true }),
            _ => json!({}),
        });
        let mut session = Session::connect(&settings(port, "pw")).unwrap();
        let action = ObsAction::ToggleSource { scene: "Main".into(), source: "Cam".into() };
        session.perform(&action).unwrap();
        let log: Vec<(String, Value)> = seen.try_iter().collect();
        assert_eq!(log.len(), 3);
        assert_eq!(log[2].0, "SetSceneItemEnabled");
        assert_eq!(log[2].1, json!({ "sceneName": "Main", "sceneItemId": 7, "sceneItemEnabled": false }));
    }

    #[test]
    fn volume_stops_at_unity() {
        let (port, seen) = fake_obs("pw", |kind, _| match kind {
            "GetInputVolume" => json!({ "inputVolumeDb": -1.0 }),
            _ => json!({}),
        });
        let mut session = Session::connect(&settings(port, "pw")).unwrap();
        session.perform(&ObsAction::Volume { input: "Mic".into(), db: 2 }).unwrap();
        let log: Vec<(String, Value)> = seen.try_iter().collect();
        assert_eq!(log[1].1, json!({ "inputName": "Mic", "inputVolumeDb": 0.0 }));
    }

    #[test]
    fn a_refused_request_keeps_the_comment_and_the_connection() {
        let (port, _seen) = fake_obs("pw", |kind, _| if kind == "ToggleRecordPause" { Value::Null } else { json!({}) });
        let mut session = Session::connect(&settings(port, "pw")).unwrap();
        assert!(session.perform(&ObsAction::ToggleRecordPause).is_err());
        session.perform(&ObsAction::ToggleRecord).unwrap();
    }

    #[test]
    fn lists_come_from_the_keys_obs_uses() {
        let (port, _seen) = fake_obs("pw", |kind, _| match kind {
            "GetSceneList" => json!({ "scenes": [{ "sceneName": "Main" }] }),
            "GetInputList" => json!({ "inputs": [{ "inputName": "Mic" }] }),
            "GetSceneItemList" => json!({ "sceneItems": [{ "sourceName": "Cam" }] }),
            "GetSourceFilterList" => json!({ "filters": [{ "filterName": "Blur" }] }),
            "GetSceneCollectionList" => json!({ "sceneCollections": ["Live"] }),
            "GetProfileList" => json!({ "profiles": ["Work"] }),
            "GetSceneTransitionList" => json!({ "transitions": [{ "transitionName": "Fade" }] }),
            _ => json!({}),
        });
        let state = Session::connect(&settings(port, "pw")).unwrap().fetch().unwrap();
        assert!(state.connected);
        assert_eq!(state.items["Main"], ["Cam"]);
        assert_eq!(state.filters["Mic"], ["Blur"]);
        assert_eq!((state.collections, state.profiles, state.transitions), (vec!["Live".to_string()], vec!["Work".to_string()], vec!["Fade".to_string()]));
    }

    #[test]
    fn thread_connects_runs_actions_and_notices_obs_leaving() {
        let (port, seen) = fake_obs("pw", |kind, _| if kind == "ToggleRecord" { json!({}) } else { json!({ "x": [] }) });
        let obs = Obs::start(settings(port, "pw"));
        let wait = |want: bool, obs: &Obs| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while obs.state().connected != want && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(obs.state().connected, want);
        };
        wait(true, &obs);
        obs.run(ObsAction::ToggleRecord);
        let found = seen.iter().find(|(kind, _)| kind == "ToggleRecord");
        assert!(found.is_some());
        obs.configure(settings(1, "pw")); // nothing listens on port 1
        wait(false, &obs);
        assert!(obs.state().error.contains("127.0.0.1:1"));
    }
}
