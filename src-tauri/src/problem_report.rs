// TTP - "Report a problem"
//
// When someone writes "it doesn't work", the answer is almost always in their
// `ttp-trace.log` — it is how every bug of September 2026 was found on the
// maintainer's Mac. This puts that file within one click of any user, and
// only when they click: nothing is sent in the background, and the report is
// a plain text file they can read before attaching it.
//
// What goes in: app version, macOS version, architecture, permission states,
// the settings (no API key — it lives in the keychain, not in settings),
// dictionary and history *counts*, and the last trace lines. What never goes
// in: anything dictated. Verbose diagnostics write the text of every stage
// into `"text"` fields; every one of them is removed here, at any depth,
// whatever the setting says.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

/// Where reports go. One constant so it can change without touching the rest.
/// The `+ttp` suffix lands in the same inbox and lets a filter file reports
/// away from everything else.
pub const SUPPORT_EMAIL: &str = "amirksmain+ttp@gmail.com";

/// Trace lines included, newest kept. About the last twenty dictations plus
/// the hotkey and permission events around them.
const TRACE_LINES: usize = 800;

#[derive(Debug, Serialize)]
pub struct ReportResult {
    /// Full path of the report file, shown to the user.
    pub path: String,
    pub email: &'static str,
}

/// Remove every `"text"` key, at any depth.
pub fn strip_text(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("text");
            map.values_mut().for_each(strip_text);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_text),
        _ => {}
    }
}

/// One trace line without its dictated text. A line whose JSON cannot be
/// parsed is dropped if it might carry text, rather than passed through.
pub fn scrub_line(line: &str) -> Option<String> {
    let Some(start) = line.find('{') else {
        return Some(line.to_string());
    };
    let (prefix, json) = line.split_at(start);
    match serde_json::from_str::<Value>(json) {
        Ok(mut value) => {
            strip_text(&mut value);
            Some(format!("{prefix}{value}"))
        }
        Err(_) if json.contains("\"text\"") => None,
        Err(_) => Some(line.to_string()),
    }
}

/// The last `max` lines across the trace files, oldest first, scrubbed.
fn recent_trace_lines(max: usize) -> Vec<String> {
    let mut newest_first: Vec<String> = Vec::new();
    for path in crate::logging::trace_files_newest_first() {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines().rev() {
            if newest_first.len() >= max {
                break;
            }
            if let Some(clean) = scrub_line(line) {
                newest_first.push(clean);
            }
        }
        if newest_first.len() >= max {
            break;
        }
    }
    newest_first.reverse();
    newest_first
}

fn macos_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn permissions_line() -> String {
    #[cfg(target_os = "macos")]
    {
        format!(
            "microphone={:?} accessibility={} input_monitoring={}",
            crate::permissions::check_microphone_permission_impl(),
            crate::paste::check_accessibility(),
            crate::fnkey::has_input_monitoring(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        "n/a".to_string()
    }
}

/// The whole report as text.
pub fn build(version: &str) -> String {
    let mut settings = serde_json::to_value(crate::settings::get_settings()).unwrap_or(Value::Null);
    strip_text(&mut settings);
    let settings = serde_json::to_string_pretty(&settings).unwrap_or_default();

    let mut out = String::new();
    out.push_str("TTP problem report\n");
    out.push_str("==================\n");
    out.push_str(&format!("generated: {}\n", chrono::Local::now().to_rfc3339()));
    out.push_str(&format!("ttp: {version}\n"));
    out.push_str(&format!("os: macOS {} ({})\n", macos_version(), std::env::consts::ARCH));
    out.push_str(&format!("permissions: {}\n", permissions_line()));
    out.push_str(&format!(
        "dictionary_entries: {}  history_entries: {}\n",
        crate::dictionary::store::get_dictionary().len(),
        crate::history::store::get_history().len(),
    ));
    out.push_str("\nsettings:\n");
    out.push_str(&settings);
    out.push_str("\n\nWhat was dictated is not in this report: every \"text\" field is removed.\n");
    out.push_str(&format!("\ntrace (last {TRACE_LINES} lines):\n"));
    for line in recent_trace_lines(TRACE_LINES) {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Percent-encode for a `mailto:` URL. Spaces become `%20`: mail clients
/// read a `+` literally.
fn mailto_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn mailto_url(version: &str, file_name: &str) -> String {
    let subject = crate::i18n::tr("report.emailSubject").replace("{{version}}", version);
    let body = crate::i18n::tr("report.emailBody").replace("{{file}}", file_name);
    format!(
        "mailto:{SUPPORT_EMAIL}?subject={}&body={}",
        mailto_encode(&subject),
        mailto_encode(&body)
    )
}

fn report_path() -> Result<PathBuf, String> {
    let dir = dirs::download_dir()
        .or_else(dirs::desktop_dir)
        .ok_or_else(|| "No Downloads folder".to_string())?;
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M");
    Ok(dir.join(format!("TTP-report-{stamp}.txt")))
}

/// Write the report to Downloads, show it in Finder, and open a pre-addressed
/// e-mail asking for it to be attached.
#[tauri::command]
pub async fn report_problem(app: tauri::AppHandle) -> Result<ReportResult, String> {
    let version = app.package_info().version.to_string();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<ReportResult, String> {
        let path = report_path()?;
        std::fs::write(&path, build(&version)).map_err(|e| format!("Could not write report: {e}"))?;
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();

        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
            let _ = std::process::Command::new("open").arg(mailto_url(&version, &file_name)).spawn();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = file_name;

        crate::trace::event("report.created", serde_json::json!({ "bytes": std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) }));
        Ok(ReportResult {
            path: path.display().to_string(),
            email: SUPPORT_EMAIL,
        })
    })
    .await
    .map_err(|e| format!("Report task failed: {e}"))?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictated_text_never_reaches_the_report() {
        let line = r#"[2026-09-15 21:22:40.185] [0000-bb48] +  411ms polish {"proc":"45ff","from":{"chars":42,"text":"je parle avec Kélou"},"to":{"chars":42,"text":"secret"},"ms":894}"#;
        let clean = scrub_line(line).unwrap();
        assert!(!clean.contains("Kélou"));
        assert!(!clean.contains("secret"));
        assert!(clean.contains("\"chars\":42"));
        assert!(clean.starts_with("[2026-09-15 21:22:40.185] [0000-bb48] +  411ms polish {"));
    }

    #[test]
    fn a_line_that_cannot_be_parsed_is_dropped_if_it_may_hold_text() {
        assert_eq!(scrub_line(r#"[x] polish {"text":"half a line"#), None);
        assert_eq!(scrub_line("no json here").as_deref(), Some("no json here"));
    }

    #[test]
    fn the_email_is_encoded_for_mail_clients() {
        assert_eq!(mailto_encode("a b+c/é"), "a%20b%2Bc%2F%C3%A9");
        let url = mailto_url("3.2.0", "TTP-report.txt");
        assert!(url.starts_with("mailto:amirksmain+ttp@gmail.com?subject="));
        assert!(!url.contains(' '));
    }
}

#[cfg(test)]
mod live {
    /// Builds a report from this machine's real settings and trace, for
    /// reading by eye: `cargo test --lib problem_report::live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn prints_a_real_report() {
        println!("{}", super::build("dev"));
    }
}
