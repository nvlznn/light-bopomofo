//! Updates. A scheduled task (installed by the installer) runs
//! `LightBopomofo.exe --check-update` at sign-in and once a day; it asks
//! GitHub which release is newest and, when it is newer, shows a
//! notification. Clicking it opens `lightbopomofo:update`, which starts this
//! app; the app downloads the installer and runs it silently.
//!
//! Only the app goes online, only to read the public release list and
//! download the installer; the input method never does.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use lightbopomofo_core::release;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};
use windows::Win32::System::Registry::{
    RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, REG_QWORD, REG_SZ,
    RRF_RT_REG_DWORD, RRF_RT_REG_QWORD, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows_core::{w, HSTRING, PCWSTR};

/// The installer's file name in every release.
const ASSET: &str = "LightBopomofo-Setup.exe";
/// Must match AppUserModelID of the Start menu shortcut (installer.iss).
const APP_ID: &str = "Noky.LightBopomofo";
const TASK: &str = "LightBopomofo\\UpdateCheck";
const SETTINGS: PCWSTR = w!("Software\\LightBopomofo");
/// The background check asks at most this often…
const CHECK_EVERY: u64 = 12 * 3600;
/// …and reminds of an update it already announced this often.
const REMIND_EVERY: u64 = 7 * 24 * 3600;

/// This app's version. LIGHTBOPOMOFO_PRETEND_VERSION overrides it, to try
/// updating without publishing a release.
pub fn current_version() -> String {
    std::env::var("LIGHTBOPOMOFO_PRETEND_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------
// Settings, per user (HKCU\Software\LightBopomofo).
// ---------------------------------------------------------------------------

fn get_dword(name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            SETTINGS,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut _ as *mut _),
            Some(&mut size),
        )
    }
    .is_ok()
    .then_some(value)
}

fn get_qword(name: PCWSTR) -> u64 {
    let mut value = 0u64;
    let mut size = 8u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            SETTINGS,
            name,
            RRF_RT_REG_QWORD,
            None,
            Some(&mut value as *mut _ as *mut _),
            Some(&mut size),
        )
    }
    .is_ok();
    if ok {
        value
    } else {
        0
    }
}

fn get_string(name: PCWSTR) -> String {
    let mut buf = [0u16; 64];
    let mut size = std::mem::size_of_val(&buf) as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            SETTINGS,
            name,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    }
    .is_ok();
    if !ok {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn set_value(name: PCWSTR, kind: u32, data: &[u8]) {
    unsafe {
        let _ = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            SETTINGS,
            name,
            kind,
            Some(data.as_ptr().cast()),
            data.len() as u32,
        );
    }
}

fn set_qword(name: PCWSTR, value: u64) {
    set_value(name, REG_QWORD.0, &value.to_le_bytes());
}

fn set_string(name: PCWSTR, value: &str) {
    let bytes: Vec<u8> = value
        .encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    set_value(name, REG_SZ.0, &bytes);
}

/// Whether to check for updates in the background (on unless turned off).
pub fn auto_check() -> bool {
    get_dword(w!("CheckForUpdates")) != Some(0)
}

pub fn set_auto_check(on: bool) {
    set_value(
        w!("CheckForUpdates"),
        REG_DWORD.0,
        &u32::from(on).to_le_bytes(),
    );
}

// ---------------------------------------------------------------------------
// Network.
// ---------------------------------------------------------------------------

struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn opened(handle: *mut core::ffi::c_void, what: &str) -> Result<Handle, String> {
    if handle.is_null() {
        Err(format!("{what}: {}", windows_core::Error::from_thread()))
    } else {
        Ok(Handle(handle))
    }
}

fn fail(what: &'static str) -> impl Fn(windows_core::Error) -> String {
    move |e| format!("{what}: {e}")
}

/// HTTPS GET of `https://{host}{path}`, following redirects (GitHub serves
/// downloads from another host).
fn get(host: &str, path: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let session = opened(
            WinHttpOpen(
                w!("LightBopomofo"),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ),
            "WinHttpOpen",
        )?;
        let _ = WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 60_000);
        let connection = opened(
            WinHttpConnect(
                session.0,
                &HSTRING::from(host),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            ),
            "WinHttpConnect",
        )?;
        let request = opened(
            WinHttpOpenRequest(
                connection.0,
                w!("GET"),
                &HSTRING::from(path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "WinHttpOpenRequest",
        )?;
        let headers: Vec<u16> = "Accept: application/vnd.github+json\r\n"
            .encode_utf16()
            .collect();
        WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0)
            .map_err(fail("WinHttpSendRequest"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(fail("WinHttpReceiveResponse"))?;
        let mut status = 0u32;
        let mut size = 4u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut _ as *mut _),
            &mut size,
            std::ptr::null_mut(),
        )
        .map_err(fail("WinHttpQueryHeaders"))?;
        if status != 200 {
            return Err(format!("HTTP {status}"));
        }
        let mut body = Vec::new();
        loop {
            let mut available = 0u32;
            WinHttpQueryDataAvailable(request.0, &mut available)
                .map_err(fail("WinHttpQueryDataAvailable"))?;
            if available == 0 {
                break;
            }
            let start = body.len();
            body.resize(start + available as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                available,
                &mut read,
            )
            .map_err(fail("WinHttpReadData"))?;
            body.truncate(start + read as usize);
            if read == 0 {
                break;
            }
        }
        Ok(body)
    }
}

/// The newest release that has the Windows installer, e.g. "2.0".
/// LIGHTBOPOMOFO_PRETEND_LATEST stands in for GitHub (screenshots, tests).
pub fn latest() -> Result<Option<String>, String> {
    if let Ok(version) = std::env::var("LIGHTBOPOMOFO_PRETEND_LATEST") {
        return Ok(Some(version));
    }
    let body = get(
        "api.github.com",
        "/repos/nvlznn/light-bopomofo/releases/latest",
    )?;
    Ok(release::latest_version(
        &String::from_utf8_lossy(&body),
        ASSET,
    ))
}

/// A newer release than this app, if there is one.
pub fn available() -> Result<Option<String>, String> {
    Ok(latest()?.filter(|v| release::is_newer(v, &current_version())))
}

/// Download `version`'s installer into the temp folder.
pub fn download(version: &str) -> Result<PathBuf, String> {
    let body = get(
        "github.com",
        &format!("/nvlznn/light-bopomofo/releases/download/v{version}/{ASSET}"),
    )?;
    // An Inno Setup installer is a few MB; anything tiny is an error page.
    if body.len() < 1_000_000 || !body.starts_with(b"MZ") {
        return Err("下載的檔案不完整".to_string());
    }
    let path = std::env::temp_dir().join(format!("LightBopomofo-Setup-{version}.exe"));
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Start the installer, silently (Windows asks for administrator approval).
/// The caller then quits, so that the installer can replace this program.
pub fn run_installer(path: &std::path::Path) -> bool {
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(path.as_os_str()),
            w!("/SILENT /SUPPRESSMSGBOXES /NORESTART"),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    result.0 as isize > 32
}

// ---------------------------------------------------------------------------
// The notification and the background check.
// ---------------------------------------------------------------------------

/// "輕注音 2.0 可以更新"; clicking it (or 更新) opens lightbopomofo:update.
pub fn notify(version: &str) -> windows_core::Result<()> {
    let xml = format!(
        r#"<toast activationType="protocol" launch="lightbopomofo:update">
  <visual><binding template="ToastGeneric">
    <text>輕注音 {version} 可以更新</text>
    <text>按「更新」下載並安裝，不需要關閉其他程式或重新開機。</text>
  </binding></visual>
  <actions><action content="更新" activationType="protocol" arguments="lightbopomofo:update"/></actions>
</toast>"#
    );
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&document)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)
}

/// `--check-update` (from the scheduled task): exit code 0 nothing to do,
/// 10 an update is available, 1 the check failed. `force` (`--now`) skips
/// the setting and the once-in-a-while limits.
pub fn check_in_background(force: bool) -> i32 {
    if !force && (!auto_check() || now() < get_qword(w!("LastUpdateCheck")) + CHECK_EVERY) {
        return 0;
    }
    set_qword(w!("LastUpdateCheck"), now());
    match available() {
        Ok(Some(version)) => {
            let announced = get_string(w!("NotifiedVersion")) == version
                && now() < get_qword(w!("NotifiedAt")) + REMIND_EVERY;
            if (force || !announced) && notify(&version).is_ok() {
                set_string(w!("NotifiedVersion"), &version);
                set_qword(w!("NotifiedAt"), now());
            }
            10
        }
        Ok(None) => 0,
        Err(_) => 1,
    }
}

// ---------------------------------------------------------------------------
// The scheduled task (created by the installer, as administrator).
// ---------------------------------------------------------------------------

fn schtasks(args: &[&str]) -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Run `--check-update` for whoever signs in, at sign-in and once a day
/// (later if the computer was off or asleep), only with a network.
pub fn register_task() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>輕注音：檢查有沒有新版本（可以在輕注音 App 關閉）</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger><Enabled>true</Enabled><Delay>PT2M</Delay></LogonTrigger>
    <CalendarTrigger>
      <StartBoundary>2026-01-01T12:00:00</StartBoundary>
      <Enabled>true</Enabled>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Users"><GroupId>S-1-5-32-545</GroupId><RunLevel>LeastPrivilege</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>true</RunOnlyIfNetworkAvailable>
    <ExecutionTimeLimit>PT10M</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Users">
    <Exec><Command>{}</Command><Arguments>--check-update</Arguments></Exec>
  </Actions>
</Task>
"#,
        xml_escape(&exe.to_string_lossy())
    );
    // schtasks reads task XML as UTF-16.
    let bytes: Vec<u8> = std::iter::once(0xfeff_u16)
        .chain(xml.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    let file = std::env::temp_dir().join("LightBopomofo-UpdateCheck.xml");
    if std::fs::write(&file, bytes).is_err() {
        return false;
    }
    let ok = schtasks(&[
        "/Create",
        "/F",
        "/TN",
        TASK,
        "/XML",
        &file.to_string_lossy(),
    ]);
    let _ = std::fs::remove_file(&file);
    ok
}

pub fn unregister_task() -> bool {
    schtasks(&["/Delete", "/F", "/TN", TASK])
}
