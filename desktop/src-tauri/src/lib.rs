// Planning prod, app de bureau. La page est la même que sur le web (web/index.html) ;
// ici, seulement ce que le web ne peut pas faire : la connexion Google « application de bureau »
// (le navigateur habituel s'ouvre une fois, la réponse revient sur 127.0.0.1) et l'enregistrement de fichiers.
use std::io::{Read, Write};
use std::net::TcpListener;
use tauri::{AppHandle, Emitter};

/// Ouvre un petit serveur local pour recevoir la réponse de Google ; renvoie son port.
/// La première requête reçue (…/?code=…) est transmise à la page (événement « oauth-code »), puis le serveur s'arrête.
#[tauri::command]
fn oauth_listen(app: AppHandle) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    std::thread::spawn(move || {
        // on ignore les requêtes parasites (favicon…) : on attend celle qui porte code= ou error=
        for _ in 0..10 {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let query = path.splitn(2, '?').nth(1).unwrap_or("").to_string();
            let done = query.contains("code=") || query.contains("error=");
            let body = if done {
                "<!doctype html><meta charset=utf-8><title>Planning prod</title><body style=\"font:16px system-ui;background:#0C0D11;color:#ECEAE4;display:grid;place-items:center;height:100vh;margin:0\"><p>C'est bon : tu peux fermer cet onglet et revenir à Planning prod.</p></body>"
            } else { "" };
            let resp = format!("HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                if done { "200 OK" } else { "404 Not Found" }, body.len(), body);
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            if done { let _ = app.emit("oauth-code", query); return; }
        }
    });
    Ok(port)
}

/// Envoi d'un gros fichier (son original) vers Google Drive : le corps arrive tel quel depuis la page (sans conversion), l'app fait la requête.
/// En-têtes : x-url (adresse d'envoi donnée par Drive), x-type (type du fichier). Réponse : { status, body }.
#[tauri::command]
async fn drive_put(request: tauri::ipc::Request<'_>) -> Result<serde_json::Value, String> {
    let h = request.headers();
    let url = h.get("x-url").and_then(|v| v.to_str().ok()).ok_or("adresse manquante")?.to_string();
    if !url.starts_with("https://www.googleapis.com/upload/") && !url.starts_with("http://127.0.0.1:") { return Err("adresse refusée".into()); }
    let ctype = h.get("x-type").and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
    let body = match request.body() { tauri::ipc::InvokeBody::Raw(b) => b.clone(), _ => return Err("fichier manquant".into()) };
    let r = reqwest::Client::new().put(&url).header("Content-Type", ctype).body(body).send().await.map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let text = r.text().await.unwrap_or_default();
    Ok(serde_json::json!({ "status": status, "body": text }))
}

/// Écran allumé pendant une session (Mac : « caffeinate », tant que l'app tourne) ; ailleurs : rien.
static AWAKE: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);
#[tauri::command]
fn keep_awake(on: bool) {
    let mut g = AWAKE.lock().unwrap();
    if let Some(mut c) = g.take() { let _ = c.kill(); let _ = c.wait(); }
    #[cfg(target_os = "macos")]
    if on {
        let pid = std::process::id().to_string();
        *g = std::process::Command::new("/usr/bin/caffeinate").args(["-di", "-w", &pid]).spawn().ok();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = on;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![oauth_listen, drive_put, keep_awake])
        .run(tauri::generate_context!())
        .expect("Planning prod n'a pas pu démarrer");
}
