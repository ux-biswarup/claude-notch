//! Phase 5 (PRD §9, §19): bring the window that owns the Claude Code session
//! to the foreground.
//!
//! V1 strategy: hooks give us the session's `cwd`; find a visible top-level
//! window whose title contains the workspace folder name, preferring VS Code
//! ("file — folder — Visual Studio Code"), then terminals. Precise terminal /
//! input focus is explicitly out of scope for V1.
//!
//! The module is named `windows`, so the `windows` *crate* is addressed with a
//! leading `::` to avoid ambiguity.

use ::windows::Win32::Foundation::{HWND, LPARAM};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VK_MENU,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use ::windows::core::BOOL;

#[derive(Debug, thiserror::Error)]
pub enum FocusError {
    #[error("no Claude Code session with a known working directory")]
    NoSession,
    #[error("no window found for \"{0}\"")]
    NoWindow(String),
}

/// Returns `Ok(true)` if the window was brought to the front, `Ok(false)` if
/// Windows refused (the caller may tell the user), or an error if nothing matched.
pub fn focus_claude_code(cwd: Option<&str>) -> Result<bool, FocusError> {
    let cwd = cwd.ok_or(FocusError::NoSession)?;
    let needle = folder_name(cwd);
    let hwnd = find_window_by_title(&needle).ok_or_else(|| FocusError::NoWindow(needle.clone()))?;
    Ok(bring_to_front(hwnd))
}

/// Last path segment of a Windows or POSIX path.
pub fn folder_name(cwd: &str) -> String {
    let trimmed = cwd.trim_end_matches(['\\', '/']);
    trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

/// 0 = no match. Higher is better: VS Code first, then terminals, then anything
/// else that mentions the folder.
pub fn score_title(title: &str, needle: &str) -> u32 {
    if needle.is_empty() {
        return 0;
    }
    let title = title.to_lowercase();
    if !title.contains(&needle.to_lowercase()) {
        return 0;
    }
    if title.contains("visual studio code") {
        3
    } else if title.contains("windows terminal")
        || title.contains("powershell")
        || title.contains("command prompt")
        || title.contains("claude")
    {
        2
    } else {
        1
    }
}

struct Search {
    needle: String,
    best: Option<(u32, HWND)>,
}

fn find_window_by_title(needle: &str) -> Option<HWND> {
    let mut search = Search {
        needle: needle.to_string(),
        best: None,
    };
    // EnumWindows reports failure if the callback ever returns FALSE; ours never
    // does, and either way the result is irrelevant — we only care about `best`.
    let _ = unsafe { EnumWindows(Some(enum_proc), LPARAM(&mut search as *mut Search as isize)) };
    search.best.map(|(_, hwnd)| hwnd)
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the `*mut Search` passed to EnumWindows above; it
    // outlives the (synchronous) enumeration and nothing else touches it.
    let search = unsafe { &mut *(lparam.0 as *mut Search) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL::from(true);
    }
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return BOOL::from(true);
    }
    let mut buf = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if copied <= 0 {
        return BOOL::from(true);
    }
    let title = String::from_utf16_lossy(&buf[..copied as usize]);
    let score = score_title(&title, &search.needle);
    if score > 0 && search.best.is_none_or(|(best, _)| score > best) {
        search.best = Some((score, hwnd));
    }
    BOOL::from(true)
}

fn bring_to_front(hwnd: HWND) -> bool {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }
        // Windows only lets the *current* foreground process hand focus over.
        // A synthetic Alt press/release lifts that lock (well-known workaround).
        nudge_foreground_lock();
        SetForegroundWindow(hwnd).as_bool()
    }
}

unsafe fn nudge_foreground_lock() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_MENU,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_name_handles_both_separators() {
        assert_eq!(
            folder_name(r"C:\Personal\projects\claude-notch"),
            "claude-notch"
        );
        assert_eq!(
            folder_name(r"C:\Personal\projects\claude-notch\"),
            "claude-notch"
        );
        assert_eq!(folder_name("/home/me/app"), "app");
        assert_eq!(folder_name("app"), "app");
    }

    #[test]
    fn prefers_vs_code_then_terminals() {
        assert_eq!(
            score_title("lib.rs - claude-notch - Visual Studio Code", "claude-notch"),
            3
        );
        assert_eq!(
            score_title("claude-notch - Windows Terminal", "claude-notch"),
            2
        );
        assert_eq!(
            score_title("C:\\Personal\\projects\\claude-notch", "claude-notch"),
            1
        );
        assert_eq!(score_title("Something else", "claude-notch"), 0);
        assert_eq!(score_title("anything", ""), 0);
        assert_eq!(
            score_title("CLAUDE-NOTCH — Visual Studio Code", "claude-notch"),
            3
        );
    }
}
