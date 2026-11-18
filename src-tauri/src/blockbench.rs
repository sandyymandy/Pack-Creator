//! Blockbench, the editor for models, animations and textures. The Pack Creator finds it, installs it when it is missing, keeps the Strut Toolkit plugin up to date from its
//! GitHub releases (the release's archive is unpacked, the script and icon are kept, the rest is removed), and opens a girl's model with her texture, or an animation on a model, ready to edit. It uses the person's own Blockbench, with their own plugins.
//!
//! A Blockbench that is already open is reached through a small plugin of ours (`pc_bridge.js`) that picks up job files from a folder. A Blockbench that is not open is
//! started with a debugging port, and the page inside it is driven over that port: the toolkit and the bridge are installed, the file is handed to the toolkit as if it had
//! been picked in the import dialog, and a "Save to pack" action is added that writes straight back to the file in the pack.

use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/* A port of its own, so it does not meet a browser or another tool's debugging port. */
const PORT: u16 = 9339;
/// The plugin that comes with the Pack Creator, used until a release has been downloaded (and when there is no network).
const TOOLKIT: &[u8] = include_bytes!("../resources/strut_toolkit.js");
const TOOLKIT_ICON: &[u8] = include_bytes!("../resources/strut_icon.png");
const BRIDGE: &[u8] = include_bytes!("../resources/pc_bridge.js");
const BUNDLED_TAG: &str = "bundled";
/// The version `pc_bridge.js` says it is; a Blockbench that holds an older one (or one that never registered) gets it installed again.
const BRIDGE_VERSION: &str = "1.0.0";
/// Where the plugin's releases are.
const PLUGIN_REPO: &str = "sandyysandy/Strut-BlockBench-Plugin";
/// How long a look at the releases is trusted, in seconds.
const CHECK_EVERY: u64 = 3600;
const MIN_VERSION: (u32, u32) = (5, 1);
const NO_WINDOW: u32 = 0x0800_0000;

/* What the installer is doing: "" when idle, then a sentence for the page to show. Kept here so the page can ask again and again while a download runs. */
static INSTALL: Mutex<String> = Mutex::new(String::new());

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub exe: Option<String>,
    pub version: Option<String>,
    /// Whether the version found is one the toolkit works with (5.1 or newer).
    pub version_ok: bool,
    pub running: bool,
    /// The release of the Strut Toolkit that is in use ("bundled" until one has been downloaded).
    pub toolkit_version: String,
    pub installing: String,
    /// Whether a Blockbench that is open can be reached without being restarted.
    pub linked: bool,
}

// =================================================================
// Finding Blockbench
// =================================================================

fn data_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());

    base.join("PleasureHorizonsPackCreator").join("blockbench")
}

fn plugin_dir() -> PathBuf {
    data_dir().join("plugin")
}

fn jobs_dir() -> PathBuf {
    data_dir().join("jobs")
}

fn find_exe() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(base) = std::env::var(variable) {
            candidates.push(PathBuf::from(base).join("Blockbench").join("Blockbench.exe"));
        }
    }

    if let Ok(base) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(base).join("Programs").join("Blockbench").join("Blockbench.exe"));
    }

    candidates.into_iter().find(|p| p.is_file())
}

/// The program's own version, read from the file's properties.
fn exe_version(exe: &Path) -> Option<String> {
    let script = format!("(Get-Item -LiteralPath '{}').VersionInfo.ProductVersion", exe.display().to_string().replace('\'', "''"));
    let out = Command::new("powershell").args(["-NoProfile", "-Command", &script]).creation_flags(NO_WINDOW).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if text.is_empty() { None } else { Some(text) }
}

fn version_ok(version: &str) -> bool {
    let mut parts = version.split('.').map(|p| p.trim().parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));

    (major, minor) >= MIN_VERSION
}

pub fn status() -> Status {
    let exe = find_exe();
    let version = exe.as_deref().and_then(exe_version);

    Status {
        exe: exe.map(|p| p.display().to_string()),
        version_ok: version.as_deref().map_or(false, version_ok),
        version,
        running: pages().is_some() || bridge_alive() || blockbench_running(),
        toolkit_version: installed_tag(),
        installing: INSTALL.lock().unwrap().clone(),
        linked: pages().is_some() || bridge_alive(),
    }
}

// =================================================================
// Installing Blockbench
// =================================================================

fn say(message: &str) {
    *INSTALL.lock().unwrap() = message.to_string();
}

fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("curl.exe").args(args).creation_flags(NO_WINDOW).output().map_err(|e| format!("curl could not run: {e}"))?;

    if !out.status.success() {
        return Err(format!("the download failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }

    Ok(out.stdout)
}

/// Starts the install in the background and returns at once; the page watches `status().installing`. It downloads Blockbench's own installer from its GitHub releases
/// (the newest 64-bit one), then runs it, and the person clicks through it.
pub fn install() -> Result<(), String> {
    if !INSTALL.lock().unwrap().is_empty() {
        return Err("Blockbench is already being installed.".into());
    }

    say("Finding the newest Blockbench...");

    std::thread::spawn(|| {
        let result = (|| -> Result<(), String> {
            let body = curl(&["-sSL", "--max-time", "30", "-H", "User-Agent: pleasure-horizons-pack-creator", "https://api.github.com/repos/JannisX11/blockbench/releases/latest"])?;
            let release: Value = serde_json::from_slice(&body).map_err(|_| "GitHub did not answer with the release list; try again in a minute".to_string())?;
            let tag = release["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
            let assets = release["assets"].as_array().ok_or("the release has no files")?;
            let wanted = [format!("Blockbench_x64_{tag}.exe"), format!("Blockbench_{tag}.exe")];
            let asset = wanted.iter().find_map(|name| assets.iter().find(|a| a["name"].as_str() == Some(name.as_str()))).ok_or("this release has no Windows installer")?;
            let url = asset["browser_download_url"].as_str().ok_or("the installer has no address")?.to_string();
            let name = asset["name"].as_str().unwrap_or("Blockbench_setup.exe").to_string();
            let size = asset["size"].as_u64().unwrap_or(0) / 1024 / 1024;
            let target = std::env::temp_dir().join(&name);

            say(&format!("Downloading Blockbench {tag} ({size} MiB)..."));
            curl(&["-sSL", "--max-time", "1800", "-o", &target.display().to_string(), &url])?;

            if !target.is_file() {
                return Err("the installer was not saved".into());
            }

            say("The Blockbench installer is open. Finish it there, then come back here.");

            let status = Command::new(&target).status().map_err(|e| format!("the installer could not start: {e}"))?;

            let _ = std::fs::remove_file(&target);

            if !status.success() {
                return Err("the installer was closed before it finished".into());
            }

            Ok(())
        })();

        match result {
            Ok(()) => say(""),
            Err(error) => say(&format!("failed: {error}")),
        }
    });

    Ok(())
}

/// Clears a failed install so the page can offer it again.
pub fn clear_install_failure() {
    let mut state = INSTALL.lock().unwrap();

    if state.starts_with("failed") {
        state.clear();
    }
}

// =================================================================
// Talking to the page inside Blockbench
// =================================================================

/// The debugging port's list of pages (a plain local HTTP request), or None when nothing answers.
fn pages() -> Option<Vec<Value>> {
    let mut stream = TcpStream::connect_timeout(&format!("127.0.0.1:{PORT}").parse().ok()?, Duration::from_millis(400)).ok()?;

    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    stream.write_all(format!("GET /json/list HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nConnection: close\r\n\r\n").as_bytes()).ok()?;

    /* The server may keep the connection open after answering, so the reply is taken as far as it has come and parsed as soon as it is whole. */
    let mut reply: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => reply.extend_from_slice(&chunk[..n]),
        }

        if let Some(at) = reply.windows(4).position(|w| w == b"\r\n\r\n") {
            let body = String::from_utf8_lossy(&reply[at + 4..]).to_string();

            if let Ok(list) = serde_json::from_str::<Vec<Value>>(&body) {
                return Some(list);
            }
        }
    }

    None
}

struct Page {
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    next: u64,
}

impl Page {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;

        let id = self.next;

        self.socket.send(tungstenite::Message::text(json!({ "id": id, "method": method, "params": params }).to_string())).map_err(|e| format!("Blockbench did not take the request: {e}"))?;

        loop {
            let message = self.socket.read().map_err(|e| format!("Blockbench stopped answering: {e}"))?;

            if let tungstenite::Message::Text(text) = message {
                let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

                if value["id"].as_u64() == Some(id) {
                    return Ok(value);
                }
            }
        }
    }

    /// Runs a script in Blockbench and returns what it gave back (a string).
    fn run(&mut self, script: &str) -> Result<String, String> {
        let reply = self.call("Runtime.evaluate", json!({ "expression": script, "awaitPromise": true, "returnByValue": true }))?;

        if let Some(problem) = reply["result"]["exceptionDetails"].as_object() {
            let text = problem.get("exception").and_then(|e| e["description"].as_str()).or_else(|| problem.get("text").and_then(Value::as_str)).unwrap_or("a script error");

            return Err(format!("Blockbench reported: {text}"));
        }

        Ok(reply["result"]["result"]["value"].as_str().unwrap_or("").to_string())
    }
}

fn connect() -> Result<Page, String> {
    let list = pages().ok_or("Blockbench is not running")?;
    let page = list.iter().find(|p| p["type"] == "page" && p["url"].as_str().map_or(false, |u| u.contains("index.html"))).ok_or("Blockbench has no window yet")?;
    let address = page["webSocketDebuggerUrl"].as_str().ok_or("Blockbench gave no address to talk to")?;
    let (socket, _) = tungstenite::connect(address).map_err(|e| format!("could not connect to Blockbench: {e}"))?;

    Ok(Page { socket, next: 0 })
}

/// Whether any Blockbench is running (with or without a way to reach it).
fn blockbench_running() -> bool {
    Command::new("tasklist").args(["/FI", "IMAGENAME eq Blockbench.exe", "/NH"]).creation_flags(NO_WINDOW).output().map(|o| String::from_utf8_lossy(&o.stdout).contains("Blockbench.exe")).unwrap_or(false)
}

/// Whether our plugin inside a running Blockbench has given a sign of life in the last few seconds.
fn bridge_alive() -> bool {
    std::fs::metadata(jobs_dir().join("alive")).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |age| age < Duration::from_secs(4))
}

/// Starts Blockbench (the person's own, with their own settings and plugins) with the debugging port, and waits until its window is ready.
fn start_with_port() -> Result<Page, String> {
    let exe = find_exe().ok_or("Blockbench is not installed. Press Install Blockbench first.")?;

    Command::new(&exe).arg(format!("--remote-debugging-port={PORT}")).spawn().map_err(|e| format!("Blockbench could not start: {e}"))?;

    let started = Instant::now();

    loop {
        if let Ok(mut page) = connect() {
            /* The page exists before Blockbench has finished loading; wait for its own objects. */
            if page.run("typeof Plugins !== 'undefined' && typeof BarItems !== 'undefined' && !!Plugins.installed ? 'yes' : ''").map_or(false, |v| v == "yes") {
                return Ok(page);
            }
        }

        if started.elapsed() > Duration::from_secs(60) {
            return Err("Blockbench did not finish starting in a minute.".into());
        }

        std::thread::sleep(Duration::from_millis(400));
    }
}

/// How the script reaches Blockbench.
enum Route {
    /// A Blockbench that is open, through our plugin in it.
    Bridge,
    /// A Blockbench the debugging port is open on.
    Port(Page),
}

/// Finds the way to a Blockbench: one that is already open (through the bridge, or through the port), else a new one is started.
fn reach() -> Result<Route, String> {
    if bridge_alive() {
        return Ok(Route::Bridge);
    }

    if pages().is_some() {
        return Ok(Route::Port(connect()?));
    }

    if blockbench_running() {
        /* Starting another would only hand over to the one that is open, and nothing could be said to it. */
        return Err("Blockbench is already open, but it has not been linked to the Pack Creator yet. Close it once and try again: the Pack Creator opens it, links itself, and after that it can find Blockbench while it is open.".into());
    }

    Ok(Route::Port(start_with_port()?))
}

/// Runs the script with its data in the Blockbench that `route` leads to.
fn deliver(route: Route, script: &str, data: &Value) -> Result<String, String> {
    match route {
        Route::Port(mut page) => {
            let reply = page.run(&format!("({script})({data})"))?;

            let _ = page.call("Page.bringToFront", json!({}));
            Ok(reply)
        }
        Route::Bridge => {
            let dir = jobs_dir();
            let id = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
            let result = dir.join(format!("{id}.result"));
            let temp = dir.join(format!("{id}.tmp"));

            std::fs::create_dir_all(&dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
            std::fs::write(&temp, json!({ "script": script, "data": data }).to_string()).map_err(|e| format!("could not leave the job for Blockbench: {e}"))?;
            std::fs::rename(&temp, dir.join(format!("{id}.job"))).map_err(|e| format!("could not leave the job for Blockbench: {e}"))?;

            let started = Instant::now();

            loop {
                if let Ok(text) = std::fs::read_to_string(&result) {
                    let _ = std::fs::remove_file(&result);

                    return Ok(text);
                }

                if started.elapsed() > Duration::from_secs(90) {
                    let _ = std::fs::remove_file(dir.join(format!("{id}.job")));
                    return Err("Blockbench did not answer. Is a dialog open in it?".into());
                }

                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

// =================================================================
// The Strut Toolkit plugin, and keeping it up to date
// =================================================================

/// The release of the plugin whose files are in the plugin folder.
fn installed_tag() -> String {
    std::fs::read_to_string(plugin_dir().join("version.txt")).map(|t| t.trim().to_string()).ok().filter(|t| !t.is_empty()).unwrap_or_else(|| BUNDLED_TAG.to_string())
}

/// What identifies the files in the plugin folder: the release and when its archive was uploaded (so an archive that is uploaded again under the same tag is taken too). The
/// plugin files that came before this was kept have none, and are replaced by the first release looked at.
fn installed_marker() -> String {
    std::fs::read_to_string(plugin_dir().join("release.txt")).map(|t| t.trim().to_string()).ok().filter(|t| !t.is_empty()).unwrap_or_else(|| BUNDLED_TAG.to_string())
}

fn write_if_different(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(path).map(|existing| existing == bytes).unwrap_or(false) {
        return Ok(());
    }

    std::fs::write(path, bytes).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// Makes sure the plugin folder has the plugin (the one that comes with the Pack Creator when no release has been downloaded) and the bridge. Returns their paths.
fn plugin_files() -> Result<(PathBuf, PathBuf), String> {
    let folder = plugin_dir();

    std::fs::create_dir_all(&folder).map_err(|e| format!("could not make {}: {e}", folder.display()))?;

    let toolkit = folder.join("strut_toolkit.js");

    if !toolkit.is_file() {
        std::fs::write(&toolkit, TOOLKIT).map_err(|e| format!("could not write the toolkit: {e}"))?;
        std::fs::write(folder.join("version.txt"), BUNDLED_TAG).ok();
    }

    if !folder.join("icon.png").is_file() {
        std::fs::write(folder.join("icon.png"), TOOLKIT_ICON).ok();
    }

    /* The bridge is ours, so it is kept as the Pack Creator has it. */
    let bridge = folder.join("pc_bridge.js");
    let jobs = jobs_dir().display().to_string().replace('\\', "/").replace('\'', "\\'");

    /* Blockbench gives a plugin no way to find this folder itself, so it is written into the file. */
    write_if_different(&bridge, String::from_utf8_lossy(BRIDGE).replace("__JOBS_DIR__", &jobs).as_bytes())?;

    Ok((toolkit, bridge))
}

/// The newest release: its tag, the address of the archive (`.rar`) it carries the plugin in, and when that archive was last uploaded.
fn latest_release() -> Result<(String, String, String), String> {
    let body = curl(&["-sSL", "--max-time", "15", "-H", "User-Agent: pleasure-horizons-pack-creator", &format!("https://api.github.com/repos/{PLUGIN_REPO}/releases/latest")])?;
    let release: Value = serde_json::from_slice(&body).map_err(|_| "GitHub did not answer with the release".to_string())?;
    let tag = release["tag_name"].as_str().map(str::to_string).ok_or_else(|| "the plugin has no release yet".to_string())?;
    let assets = release["assets"].as_array().cloned().unwrap_or_default();
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("strut_toolkit.rar"))
        .or_else(|| assets.iter().find(|a| a["name"].as_str().map_or(false, |n| n.to_lowercase().ends_with(".rar"))))
        .ok_or_else(|| format!("the release {tag} has no .rar file with the plugin in it"))?;
    let archive = asset["browser_download_url"].as_str().ok_or_else(|| format!("the release {tag} gave no address for its .rar file"))?.to_string();
    let uploaded = asset["updated_at"].as_str().unwrap_or("").to_string();

    Ok((tag, archive, uploaded))
}

/// A folder in the temp folder that is removed when it goes out of scope, whatever happened in between.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Where Windows keeps its own `tar`, which also opens `.rar` files.
fn windows_tar() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())).join("System32").join("tar.exe")
}

/// Looks for a newer release of the plugin and installs it into the plugin folder. The release's `.rar` is downloaded and unpacked in a temporary folder, the script and the
/// icon are taken out of it, and everything else (and the archive) is removed again. Blockbench takes the new plugin up the next time a job is sent (the old one is
/// uninstalled and the new one installed). With `force` it looks now; otherwise at most once an hour, and a failure (no network) keeps what there is. Returns the release now in use.
pub fn refresh_plugin(force: bool) -> Result<String, String> {
    plugin_files()?;

    let folder = plugin_dir();
    let stamp = folder.join("checked.txt");
    let recent = !force && std::fs::metadata(&stamp).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |age| age.as_secs() < CHECK_EVERY);

    if recent {
        return Ok(installed_tag());
    }

    let (tag, archive, uploaded) = latest_release()?;
    let marker = format!("{tag}|{uploaded}");

    std::fs::write(&stamp, &tag).ok();

    if marker == installed_marker() {
        return Ok(tag);
    }

    let scratch = Scratch(std::env::temp_dir().join(format!("ph-toolkit-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))));
    let unpacked = scratch.0.join("unpacked");
    let file = scratch.0.join("strut_toolkit.rar");

    std::fs::create_dir_all(&unpacked).map_err(|e| format!("could not make a folder to unpack the plugin in: {e}"))?;
    curl(&["-sSL", "-f", "--max-time", "1800", "-o", &file.display().to_string(), &archive]).map_err(|e| format!("could not download the plugin {tag}: {e}"))?;

    let tar = Command::new(windows_tar()).arg("-xf").arg(&file).arg("-C").arg(&unpacked).creation_flags(NO_WINDOW).output().map_err(|e| format!("Windows' own unpacker (tar) could not run: {e}"))?;

    if !tar.status.success() {
        return Err(format!("the plugin {tag} could not be unpacked: {}", String::from_utf8_lossy(&tar.stderr).trim()));
    }

    /* The archive may keep the files in a folder; the script and the icon are found wherever they are. */
    let found = |name: &str| pc_core::check::walk(&unpacked).into_iter().find(|rel| rel.rsplit('/').next().map_or(false, |n| n.eq_ignore_ascii_case(name))).map(|rel| unpacked.join(rel));
    let script = found("strut_toolkit.js").ok_or_else(|| format!("the plugin {tag} has no strut_toolkit.js in it"))?;
    let bytes = std::fs::read(&script).map_err(|e| format!("could not read the unpacked plugin: {e}"))?;

    if bytes.len() < 1000 {
        return Err(format!("the plugin {tag} came back empty"));
    }

    /* Only these two are kept, and only now that the unpacking worked: a failure leaves the working plugin as it was. */
    std::fs::write(folder.join("strut_toolkit.js"), bytes).map_err(|e| format!("could not write the plugin: {e}"))?;

    if let Some(icon) = found("icon.png") {
        std::fs::copy(icon, folder.join("icon.png")).map_err(|e| format!("could not write the icon: {e}"))?;
    }

    std::fs::write(folder.join("version.txt"), &tag).map_err(|e| format!("could not write the version: {e}"))?;
    std::fs::write(folder.join("release.txt"), &marker).map_err(|e| format!("could not write the version: {e}"))?;

    /* The archive and what was unpacked are removed as `scratch` goes out of scope here. */
    Ok(tag)
}

// =================================================================
// Opening files
// =================================================================

/// What to open. Paths are real files on disk, already checked to be inside the pack.
pub struct Job {
    pub model: PathBuf,
    pub texture: Option<PathBuf>,
    pub animation: Option<PathBuf>,
}

const SCRIPT: &str = r#"(async (job) => {
  const wait = ms => new Promise(r => setTimeout(r, ms));
  const bytesOf = b64 => Uint8Array.from(atob(b64), c => c.charCodeAt(0)).buffer;

  /* The toolkit is installed from the Pack Creator's copy, and again whenever a newer release has been downloaded: the old one is uninstalled first. Installed (not just
     loaded), so Blockbench loads it by itself the next time it starts. The bridge lets the Pack Creator reach this Blockbench while it is open. */
  const have = Plugins.installed.find(p => p.id === 'strut_toolkit');
  let seen = null;
  try { seen = localStorage.getItem('pc_strut_tag'); } catch (e) { /* no storage */ }

  if (!have || seen !== job.tag) {
    if (have) { try { await have.uninstall(); } catch (e) { /* replaced below anyway */ } }
    await new Plugin().loadFromFile({ path: job.toolkit }, false);
    try { localStorage.setItem('pc_strut_tag', job.tag); } catch (e) { /* no storage */ }
  }

  const link = Plugins.all.find(p => p.id === 'pc_bridge');

  if (!link || !link.installed || link.version !== job.bridgeVersion) {
    if (link) { try { await link.uninstall(); } catch (e) { /* replaced below anyway */ } }
    try { await new Plugin().loadFromFile({ path: job.bridge }, false); } catch (e) { /* the link is only a convenience */ }
  }

  for (let i = 0; i < 50 && !(BarItems.import_strut_binary_model && BarItems.export_strut_binary_model); i++) await wait(200);

  if (!BarItems.import_strut_binary_model) return 'The Strut Toolkit did not load in Blockbench.';

  const show = () => { try { currentwindow.show(); currentwindow.focus(); } catch (e) { /* the window is already in front, or cannot be */ } };

  if (!job.model) { show(); return 'ok'; }

  /* The page has no direct file access; Blockbench's own writer saves the bytes as they are. */
  const writeBack = (target, bytes) => Filesystem.writeFile(target, { content: bytes, savetype: 'binary' }, () => {});

  /* "Save to pack" writes the project straight over the file it was opened from, instead of asking where to put it. */
  const saver = (id, name, source, property, exporter, message) => {
    if (BarItems[id]) return;

    const action = new Action(id, {
      name, icon: 'save', category: 'file',
      keybind: new Keybind({ key: id === 'pc_save_model' ? 'e' : 'r', ctrl: true, shift: true }),
      click() {
        const target = Project && Project[property];
        if (!target) { Blockbench.showQuickMessage('This project was not opened from the Pack Creator.', 2500); return; }
        const original = Filesystem.exportFile;
        Filesystem.exportFile = options => { writeBack(target, options.content); Blockbench.showQuickMessage(message, 1800); };
        try { BarItems[exporter].click(); } finally { Filesystem.exportFile = original; }
      }
    });
    MenuBar.addAction(action, 'file');
  };

  saver('pc_save_model', 'Save model to the pack', 'model', 'pc_model_path', 'export_strut_binary_model', 'Model saved to the pack');
  saver('pc_save_animations', 'Save animations to the pack', 'animation', 'pc_animation_path', 'export_strut_binary_animations', 'Animations saved to the pack');

  const feed = (actionId, file) => {
    const original = Blockbench.import;
    Blockbench.import = (options, callback) => callback([{ name: file.name, path: file.path, content: bytesOf(file.b64) }]);
    try { BarItems[actionId].click(); } finally { Blockbench.import = original; }
  };

  feed('import_strut_binary_model', job.model);
  await wait(2000);

  if (!Project) return 'The model did not open.';

  Project.pc_model_path = job.model.path;

  if (job.texture && Texture.all.length) {
    const base = job.texture.split('/').pop();
    const target = Texture.all.find(t => t.name === base) || Texture.all[0];
    target.fromPath(job.texture);
  }

  if (job.animation) {
    feed('import_strut_binary_animations', job.animation);
    await wait(1200);
    Project.pc_animation_path = job.animation.path;
    if (typeof Modes !== 'undefined' && Modes.options.animate) Modes.options.animate.select();
  }

  show();
  return 'ok';
})"#;

fn file_arg(path: &Path) -> Result<Value, String> {
    use base64::Engine;

    let bytes = std::fs::read(path).map_err(|e| format!("{} could not be read: {e}", path.display()))?;

    Ok(json!({
        "name": path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "path": path.display().to_string().replace('\\', "/"),
        "b64": base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

/// What the script is told about the plugin, whichever way it goes.
fn plugin_data() -> Result<Value, String> {
    /* A look for a newer release first; with no network the copy there is is used. */
    let _ = refresh_plugin(false);

    let (toolkit, bridge) = plugin_files()?;

    Ok(json!({
        "toolkit": toolkit.display().to_string().replace('\\', "/"),
        "bridge": bridge.display().to_string().replace('\\', "/"),
        "tag": installed_marker(),
        "bridgeVersion": BRIDGE_VERSION,
    }))
}

/// Opens the model (and the texture, and the animation file on top of it) in Blockbench: the one that is open, or a new one.
pub fn open(job: &Job) -> Result<(), String> {
    let mut data = plugin_data()?;

    data["model"] = file_arg(&job.model)?;

    if let Some(texture) = &job.texture {
        data["texture"] = json!(texture.display().to_string().replace('\\', "/"));
    }

    if let Some(animation) = &job.animation {
        data["animation"] = file_arg(animation)?;
    }

    let reply = deliver(reach()?, SCRIPT, &data)?;

    if reply == "ok" { Ok(()) } else { Err(reply) }
}

/// Brings Blockbench up with the toolkit installed and nothing open, so the person can see it is ready.
pub fn prepare() -> Result<(), String> {
    let data = plugin_data()?;
    let reply = deliver(reach()?, SCRIPT, &data)?;

    if reply == "ok" { Ok(()) } else { Err(reply) }
}

/// Looks for a newer release of the plugin now (the button in the Blockbench window), and says what happened.
pub fn update_plugin() -> Result<String, String> {
    let before = installed_tag();
    let before_marker = installed_marker();
    let after = refresh_plugin(true)?;

    Ok(if before_marker == installed_marker() { format!("The Strut Toolkit {after} is the newest release.") } else { if before == after { format!("The Strut Toolkit {after} was downloaded again, because its file on GitHub is not the one that was here. It is installed in Blockbench the next time a model is opened.") } else { format!("The Strut Toolkit was updated from {before} to {after}. It is installed in Blockbench the next time a model is opened.") } })
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Needs a Blockbench running with the debugging port; run by hand while one is open. */
    #[test]
    #[ignore]
    fn talks_to_a_running_blockbench() {
        let mut page = connect().expect("connect");
        let value = page.run("'hello ' + typeof Plugins").expect("run");

        println!("{value}");
        assert!(value.starts_with("hello"));
    }
}
