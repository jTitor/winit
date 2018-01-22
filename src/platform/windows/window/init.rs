/*!
 * Contains setup methods.
 */

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

use super::super::raw_input::register_input_devices;

unsafe fn init(window: WindowAttributes, pl_attribs: PlatformSpecificWindowBuilderAttributes,
               inserter: events_loop::Inserter) -> Result<Window, CreationError> {
    let title = OsStr::new(&window.title).encode_wide().chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    // registering the window class
    let class_name = register_window_class();

    // building a RECT object with coordinates
    let mut rect = RECT {
        left: 0, right: window.dimensions.unwrap_or((1024, 768)).0 as LONG,
        top: 0, bottom: window.dimensions.unwrap_or((1024, 768)).1 as LONG,
    };

    // switching to fullscreen if necessary
    // this means adjusting the window's position so that it overlaps the right monitor,
    //  and change the monitor's resolution if necessary
    let fullscreen = if let Some(RootMonitorId { ref inner }) = window.fullscreen {
        try!(switch_to_fullscreen(&mut rect, inner));
        true
    } else {
        false
    };

    // computing the style and extended style of the window
    let (ex_style, style) = if fullscreen || !window.decorations {
        (winuser::WS_EX_APPWINDOW,
            //winapi::WS_POPUP is incompatible with winapi::WS_CHILD
            if pl_attribs.parent.is_some() {
                winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
            else {
                winuser::WS_POPUP | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
        )
    } else {
        (winuser::WS_EX_APPWINDOW | winuser::WS_EX_WINDOWEDGE,
            winuser::WS_OVERLAPPEDWINDOW | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN)
    };

    // adjusting the window coordinates using the style
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (width, height) = if fullscreen || window.dimensions.is_some() {
            (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
        } else {
            (None, None)
        };

        let (x, y) = if fullscreen {
            (Some(rect.left), Some(rect.top))
        } else {
            (None, None)
        };

        let mut style = if !window.visible {
            style
        } else {
            style | winuser::WS_VISIBLE
        };

        if pl_attribs.parent.is_some() {
            style |= winuser::WS_CHILD;
        }

        let handle = winuser::CreateWindowExW(ex_style | winuser::WS_EX_ACCEPTFILES,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN,
            x.unwrap_or(winuser::CW_USEDEFAULT), y.unwrap_or(winuser::CW_USEDEFAULT),
            width.unwrap_or(winuser::CW_USEDEFAULT), height.unwrap_or(winuser::CW_USEDEFAULT),
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(), libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut());

        if handle.is_null() {
            return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        let hdc = winuser::GetDC(handle);
        if hdc.is_null() {
            return Err(CreationError::OsError(format!("GetDC function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle, hdc)
    };

    // Set up raw input here.
    register_input_devices(real_window.0);
    // {
    //     let mut rid: winuser::RAWINPUTDEVICE = mem::uninitialized();
    //     rid.usUsagePage = hidusage::HID_USAGE_PAGE_GENERIC;
    //     rid.usUsage = hidusage::HID_USAGE_GENERIC_MOUSE;
    //     rid.dwFlags = 0;
    //     rid.hwndTarget = real_window.0;

    //     winuser::RegisterRawInputDevices(&rid, 1, mem::size_of::<winuser::RAWINPUTDEVICE>() as u32);
    // }

    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(events_loop::WindowState {
        cursor: winuser::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window.clone(),
        mouse_in_window: false,
    }));

    inserter.insert(real_window.0, window_state.clone());

    // making the window transparent
    if window.transparent {
        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
    }

    // calling SetForegroundWindow if fullscreen
    if fullscreen {
        winuser::SetForegroundWindow(real_window.0);
    }

    // Building the struct.
    Ok(Window {
        window: real_window,
        window_state: window_state,
    })
}

unsafe fn register_window_class() -> Vec<u16> {
    let class_name = OsStr::new("Window Class").encode_wide().chain(Some(0).into_iter())
                                               .collect::<Vec<_>>();

    let class = winuser::WNDCLASSEXW {
        cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
        style: winuser::CS_HREDRAW | winuser::CS_VREDRAW | winuser::CS_OWNDC,
        lpfnWndProc: Some(events_loop::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: libloaderapi::GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),       // must be null in order for cursor state to work properly
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    winuser::RegisterClassExW(&class);

    class_name
}

unsafe fn switch_to_fullscreen(rect: &mut RECT, monitor: &MonitorId)
                               -> Result<(), CreationError>
{
    // adjusting the rect
    {
        let pos = monitor.get_position();
        rect.left += pos.0 as LONG;
        rect.right += pos.0 as LONG;
        rect.top += pos.1 as LONG;
        rect.bottom += pos.1 as LONG;
    }

    // changing device settings
    let mut screen_settings: wingdi::DEVMODEW = mem::zeroed();
    screen_settings.dmSize = mem::size_of::<wingdi::DEVMODEW>() as WORD;
    screen_settings.dmPelsWidth = (rect.right - rect.left) as DWORD;
    screen_settings.dmPelsHeight = (rect.bottom - rect.top) as DWORD;
    screen_settings.dmBitsPerPel = 32;      // TODO: ?
    screen_settings.dmFields = wingdi::DM_BITSPERPEL | wingdi::DM_PELSWIDTH | wingdi::DM_PELSHEIGHT;

    let result = winuser::ChangeDisplaySettingsExW(monitor.get_adapter_name().as_ptr(),
                                                  &mut screen_settings, ptr::null_mut(),
                                                  winuser::CDS_FULLSCREEN, ptr::null_mut());

    if result != winuser::DISP_CHANGE_SUCCESSFUL {
        return Err(CreationError::OsError(format!("ChangeDisplaySettings failed: {}", result)));
    }

    Ok(())
}