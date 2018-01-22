/*!
 * Initializers for platform-specific input device fields.
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

use super::get_device_properties;
use super::build_hid_detail_list;

unsafe fn register_input_devices(target_window: HWND) { //registerAllInputDevices(target_window: HWND) {
	//want to register:
	//joysticks
	//gamepads
	//keyboard
	//mouse

	//This may not be the right format for the Win32 call;
	//if you get an unexpected panic, check if there's
	//a different way to allocate C arrays
	let mut rid_list: [winuser::RAWINPUTDEVICE; 4] = mem::uninitialized();

	rid_list[0].usUsagePage = 1;			//hidusage::HID_USAGE_PAGE_GENERIC
	rid_list[0].usUsage = 4;				//joystick
	rid_list[0].dwFlags = 0;				//no special options
	rid_list[0].hwndTarget = target_window;	//window is foreground handle

	rid_list[1].usUsagePage = 1;			//hidusage::HID_USAGE_PAGE_GENERIC
	rid_list[1].usUsage = 5;				//gamepad
	rid_list[1].dwFlags = 0;				//no special options
	rid_list[1].hwndTarget = target_window;

	rid_list[2].usUsagePage = 1;			//hidusage::HID_USAGE_PAGE_GENERIC
	rid_list[2].usUsage = 6;				//keyboard
	//disable legacy messages for keyboard;
	//we don't want the start menu buttons actually bringing up the menu while in game, for example
	rid_list[2].dwFlags = RIDEV_NOLEGACY;
	rid_list[2].hwndTarget = target_window;

	rid_list[3].usUsagePage = 1;			//hidusage::HID_USAGE_PAGE_GENERIC
	rid_list[3].usUsage = 2;				//hidusage::HID_USAGE_GENERIC_MOUSE
	//Do NOT disable legacy messages for the mouse.
	//We don't intercept those messages, but the OS does.
	//In windowed mode, that would make the target_window not respond to any commands.
	//Plus, it's also useful for GUI work!
	rid_list[3].dwFlags = 0;
	rid_list[3].hwndTarget = target_window;

	if !winuser::RegisterRawInputDevices(&rid, 4, mem::size_of::<winuser::RAWINPUTDEVICE>() as u32) {
		LogD("Couldn't register raw input devices!");
		Win32Helpers::LogLastError();
	}

	LogD("Registered raw input devices");
	let mut numDevs = 0u32;
	winuser::GetRegisteredRawInputDevices(NULL, &numDevs, mem::size_of::<winuser::RAWINPUTDEVICE>() as u32);
	LogD(String("OS recognizes ") + numDevs + " registered categories");
}

void Win32Platform::InitInput(WindowHnd target_window, TypedHandle<InputManager> inputMgr)
{
	if(!window)
	{
		LogW("InitInput: target window is invalid, aborting!");
		return;
	}

	if(!inputMgr)
	{
		LogW("InitInput: couldn't get input manager instance, aborting!");
		return;
	}

	SetInputManager(inputMgr);
	build_hid_detail_list();
	//and tell platform we're ready for input
	register_input_devices((HWND)target_window);
}