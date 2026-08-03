use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn project_root() -> PathBuf {
    if let Ok(root) = std::env::var("LINLIS_ROOT") {
        return PathBuf::from(root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSlotStatus {
    pub slot: String,
    pub port: u16,
    pub http_status: Option<u16>,
    pub release: Option<serde_json::Value>,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseStatus {
    pub prod: ReleaseSlotStatus,
    pub canary: ReleaseSlotStatus,
    pub note: String,
}

async fn probe_http(port: u16) -> Option<u16> {
    let url = format!("http://127.0.0.1:{port}/");
    let output = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "3",
            &url,
        ])
        .output()
        .await
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn read_release_json(slot_dir: &Path) -> Option<serde_json::Value> {
    let path = slot_dir.join("meta").join("RELEASE.json");
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub async fn release_status() -> ReleaseStatus {
    let root = project_root();
    let release_root =
        std::env::var("RELEASE_ROOT").unwrap_or_else(|_| "/opt/linlis-workpanel".into());
    let prod_port: u16 = std::env::var("PROD_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let canary_port: u16 = std::env::var("CANARY_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8081);
    let prod_data = std::env::var("PROD_DATA")
        .unwrap_or_else(|_| root.join("data").to_string_lossy().into_owned());
    let canary_data = std::env::var("CANARY_DATA")
        .unwrap_or_else(|_| root.join("data-canary").to_string_lossy().into_owned());
    let prod_slot = PathBuf::from(&release_root).join("prod");
    let canary_slot = PathBuf::from(&release_root).join("canary");

    ReleaseStatus {
        prod: ReleaseSlotStatus {
            slot: "prod".into(),
            port: prod_port,
            http_status: probe_http(prod_port).await,
            release: read_release_json(&prod_slot),
            data_dir: prod_data,
        },
        canary: ReleaseSlotStatus {
            slot: "canary".into(),
            port: canary_port,
            http_status: probe_http(canary_port).await,
            release: read_release_json(&canary_slot),
            data_dir: canary_data,
        },
        note: "Promote 请使用 scripts/promote-canary.sh（不进 UI，避免覆盖生产 DB）".into(),
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpsJobState {
    pub running: bool,
    pub kind: String,
    pub exit_code: Option<i32>,
    pub log: String,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

fn ops_job() -> &'static Mutex<OpsJobState> {
    static CELL: OnceLock<Mutex<OpsJobState>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(OpsJobState::default()))
}

fn ops_lock() -> &'static AtomicBool {
    static CELL: OnceLock<AtomicBool> = OnceLock::new();
    CELL.get_or_init(|| AtomicBool::new(false))
}

pub fn ops_job_snapshot() -> OpsJobState {
    ops_job().lock().map(|g| g.clone()).unwrap_or_default()
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

async fn run_script(kind: &str, script_rel: &str) -> Result<(), String> {
    if ops_lock().swap(true, Ordering::SeqCst) {
        return Err("已有运维任务在运行，请稍后再试。".into());
    }
    let root = project_root();
    let script = root.join(script_rel);
    if !script.is_file() {
        ops_lock().store(false, Ordering::SeqCst);
        return Err(format!("找不到脚本：{}", script.display()));
    }
    {
        let mut g = ops_job().lock().map_err(|e| e.to_string())?;
        *g = OpsJobState {
            running: true,
            kind: kind.into(),
            exit_code: None,
            log: String::new(),
            started_at: Some(now_ms()),
            finished_at: None,
        };
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script)
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "1")
        .env("NODE_OPTIONS", "--max-old-space-size=1024")
        .env_remove("LINLIS_SKIP_TEST_GATE")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let result = async {
        let mut child = cmd.spawn().map_err(|e| format!("启动失败：{e}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let log_buf = Arc::new(Mutex::new(String::new()));

        async fn pump_out(
            stream: Option<impl tokio::io::AsyncRead + Unpin>,
            buf: Arc<Mutex<String>>,
        ) {
            let Some(stream) = stream else { return };
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut g) = buf.lock() {
                    g.push_str(&line);
                    g.push('\n');
                    if g.len() > 200_000 {
                        let drain = g.len() - 160_000;
                        g.drain(..drain);
                    }
                }
                if let (Ok(mut job), Ok(b)) = (ops_job().lock(), buf.lock()) {
                    job.log = b.clone();
                }
            }
        }

        let buf_out = log_buf.clone();
        let buf_err = log_buf.clone();
        let t1 = tokio::spawn(async move { pump_out(stdout, buf_out).await });
        let t2 = tokio::spawn(async move { pump_out(stderr, buf_err).await });
        let status = child.wait().await.map_err(|e| format!("等待进程失败：{e}"))?;
        let _ = t1.await;
        let _ = t2.await;
        Ok::<i32, String>(status.code().unwrap_or(-1))
    }
    .await;

    let exit = match &result {
        Ok(code) => Some(*code),
        Err(_) => Some(-1),
    };
    if let Ok(mut g) = ops_job().lock() {
        g.running = false;
        g.exit_code = exit;
        g.finished_at = Some(now_ms());
        if let Err(e) = &result {
            g.log.push_str(&format!("\nERROR: {e}\n"));
        }
    }
    ops_lock().store(false, Ordering::SeqCst);
    match result {
        Ok(0) => Ok(()),
        Ok(code) => Err(format!("脚本退出码 {code}")),
        Err(e) => Err(e),
    }
}

pub fn kickoff_test_gate() -> Result<(), String> {
    if ops_lock().load(Ordering::SeqCst) {
        return Err("已有运维任务在运行，请稍后再试。".into());
    }
    tokio::spawn(async {
        let _ = run_script("test-gate", "scripts/test-gate.sh").await;
    });
    Ok(())
}

pub fn kickoff_deploy_canary() -> Result<(), String> {
    if ops_lock().load(Ordering::SeqCst) {
        return Err("已有运维任务在运行，请稍后再试。".into());
    }
    tokio::spawn(async {
        let _ = run_script("deploy-canary", "scripts/deploy-canary.sh").await;
    });
    Ok(())
}
