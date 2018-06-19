#![cfg(target_os = "windows")]

use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::raw;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::channel;

use platform::platform::events_loop;
use platform::platform::EventsLoop;
use platform::platform::PlatformSpecificWindowBuilderAttributes;
use platform::platform::MonitorId;
use platform::platform::WindowId;
use platform::platform::window::init;

use CreationError;
use CursorState;
use MouseCursor;
use WindowAttributes;
use MonitorId as RootMonitorId;

use winapi::shared::minwindef::{UINT, WORD, DWORD, BOOL};
use winapi::shared::windef::{HWND, HDC, RECT, POINT};
use winapi::shared::hidusage;
use winapi::um::{winuser, dwmapi, wingdi, libloaderapi, processthreadsapi};
use winapi::um::winnt::{LPCWSTR, LONG};

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    pub window: WindowWrapper,

    /// The current window state.
    pub window_state: Arc<Mutex<events_loop::WindowState>>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    pub fn new(events_loop: &EventsLoop, w_attr: &WindowAttributes,
               pl_attr: &PlatformSpecificWindowBuilderAttributes) -> Result<Window, CreationError>
    {
        let mut w_attr = Some(w_attr.clone());
        let mut pl_attr = Some(pl_attr.clone());

        let (tx, rx) = channel();

        events_loop.execute_in_thread(move |inserter| {
            // We dispatch an `init` function because of code style.
            let win = unsafe { init(w_attr.take().unwrap(), pl_attr.take().unwrap(), inserter) };
            let _ = tx.send(win);
        });

        rx.recv().unwrap()
    }

    pub fn set_title(&self, text: &str) {
        unsafe {
            let text = OsStr::new(text).encode_wide().chain(Some(0).into_iter())
                                       .collect::<Vec<_>>();

            winuser::SetWindowTextW(self.window.0, text.as_ptr() as LPCWSTR);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_SHOW);
        }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_HIDE);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut placement: winuser::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<winuser::WINDOWPLACEMENT>() as UINT;

        if unsafe { winuser::GetWindowPlacement(self.window.0, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            winuser::SetWindowPos(self.window.0, ptr::null_mut(), x as raw::c_int, y as raw::c_int,
                                 0, 0, winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOSIZE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetClientRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetWindowRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe {
            // Calculate the outer size based upon the specified inner size
            let mut rect = RECT { top: 0, left: 0, bottom: y as LONG, right: x as LONG };
            let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
            let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
            let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
            winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as raw::c_int;
            let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

            winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    // TODO: remove
    pub fn platform_display(&self) -> *mut ::libc::c_void {
        panic!()        // Deprecated function ; we don't care anymore
    }
    // TODO: remove
    pub fn platform_window(&self) -> *mut ::libc::c_void {
        self.window.0 as *mut ::libc::c_void
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_id = match cursor {
            MouseCursor::Arrow | MouseCursor::Default => winuser::IDC_ARROW,
            MouseCursor::Hand => winuser::IDC_HAND,
            MouseCursor::Crosshair => winuser::IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => winuser::IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => winuser::IDC_NO,
            MouseCursor::EResize => winuser::IDC_SIZEWE,
            MouseCursor::NResize => winuser::IDC_SIZENS,
            MouseCursor::WResize => winuser::IDC_SIZEWE,
            MouseCursor::SResize => winuser::IDC_SIZENS,
            MouseCursor::EwResize | MouseCursor::ColResize => winuser::IDC_SIZEWE,
            MouseCursor::NsResize | MouseCursor::RowResize => winuser::IDC_SIZENS,
            MouseCursor::Wait | MouseCursor::Progress => winuser::IDC_WAIT,
            MouseCursor::Help => winuser::IDC_HELP,
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = cursor_id;
    }

    // TODO: it should be possible to rework this function by using the `execute_in_thread` method
    // of the events loop.
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.window_state.lock().unwrap();

        let foreground_thread_id = unsafe { winuser::GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

        let res = match (state, current_state.cursor_state) {
            (CursorState::Normal, CursorState::Normal) => Ok(()),
            (CursorState::Hide, CursorState::Hide) => Ok(()),
            (CursorState::Grab, CursorState::Grab) => Ok(()),

            (CursorState::Hide, CursorState::Normal) => {
                current_state.cursor_state = CursorState::Hide;
                Ok(())
            },

            (CursorState::Normal, CursorState::Hide) => {
                current_state.cursor_state = CursorState::Normal;
                Ok(())
            },

            (CursorState::Grab, CursorState::Normal) | (CursorState::Grab, CursorState::Hide) => {
                unsafe {
                    let mut rect = mem::uninitialized();
                    if winuser::GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if winuser::ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    if winuser::ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = POINT {
            x: x,
            y: y,
        };

        unsafe {
            if winuser::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(());
            }

            if winuser::SetCursorPos(point.x, point.y) == 0 {
                return Err(());
            }
        }

        Ok(())
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        unimplemented!()
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        unimplemented!()
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // We are sending WM_CLOSE, and our callback will process this by calling DefWindowProcW,
            // which in turn will send a WM_DESTROY.
            winuser::PostMessageW(self.window.0, winuser::WM_CLOSE, 0, 0);
        }
    }
}

/// A simple wrapper that destroys the window when it is destroyed.
#[doc(hidden)]
pub struct WindowWrapper(pub HWND, pub HDC);

impl Drop for WindowWrapper {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            winuser::DestroyWindow(self.0);
        }
    }
}
