//! Give a console-less Codex process a windowless console on Windows.
//!
//! A process started without a console (the detached app-server daemon,
//! desktop-app hosts, `DETACHED_PROCESS` launches) hands none to its
//! children, so every console child it spawns without `CREATE_NO_WINDOW`
//! (git, hooks, helper binaries) allocates its own visible console window.
//! Patching each spawn site one by one keeps missing some; allocating a single
//! windowless console up front makes every child inherit it instead.
//!
//! `AllocConsoleWithOptions` is Windows 11 24H2+. It is resolved at runtime,
//! so older systems keep today's behavior. It fails harmlessly when the process
//! already has a console, which covers every interactive launch.

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::GetConsoleWindow;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
use windows_sys::Win32::System::Console::STD_HANDLE;
use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Console::SetStdHandle;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::LibraryLoader::GetProcAddress;

const ALLOC_CONSOLE_MODE_NO_WINDOW: i32 = 2;

#[repr(C)]
struct AllocConsoleOptions {
    mode: i32,
    use_show_window: i32,
    show_window: u16,
}

type AllocConsoleWithOptionsFn =
    unsafe extern "system" fn(options: *const AllocConsoleOptions, result: *mut i32) -> i32;

/// Returns true when a windowless console was allocated.
pub(crate) fn ensure_windowless_console() -> bool {
    // SAFETY: plain Win32 queries with no pointer arguments.
    if unsafe { GetConsoleWindow() } != 0 {
        return false;
    }
    let Some(alloc) = resolve_alloc_console_with_options() else {
        return false;
    };

    // Allocating a console rebinds any unset standard handle to it. Keep the
    // handles the launcher gave us (pipes, log files) exactly as they were.
    let saved = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
        // SAFETY: GetStdHandle has no preconditions.
        .map(|which| (which, unsafe { GetStdHandle(which) }));

    let options = AllocConsoleOptions {
        mode: ALLOC_CONSOLE_MODE_NO_WINDOW,
        use_show_window: 0,
        show_window: 0,
    };
    let mut result = 0;
    // SAFETY: `options` and `result` are valid for the duration of the call.
    let hr = unsafe { alloc(&options, &mut result) };
    if hr < 0 {
        return false;
    }

    for (which, handle) in saved {
        restore_std_handle(which, handle);
    }
    true
}

fn restore_std_handle(which: STD_HANDLE, handle: HANDLE) {
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return;
    }
    // SAFETY: restoring a handle value this process already owned.
    unsafe {
        SetStdHandle(which, handle);
    }
}

fn resolve_alloc_console_with_options() -> Option<AllocConsoleWithOptionsFn> {
    let module_name: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    // SAFETY: NUL-terminated wide string; kernel32 is always loaded.
    let module = unsafe { GetModuleHandleW(module_name.as_ptr()) };
    if module == 0 {
        return None;
    }
    // SAFETY: NUL-terminated ANSI name.
    let proc = unsafe { GetProcAddress(module, c"AllocConsoleWithOptions".as_ptr().cast()) }?;
    // SAFETY: signature matches the documented AllocConsoleWithOptions export.
    Some(unsafe {
        std::mem::transmute::<unsafe extern "system" fn() -> isize, AllocConsoleWithOptionsFn>(
            proc,
        )
    })
}
