/*!
 * Handlers for raw input events such as WM_INPUT.
 */
use std::mem::size_of;
//TODO_rust: need Win32 uses: GET_X_LPARAM, GET_Y_LPARAM

/**
 * Callback for Win32 raw input message events.
 * As per Win32 handler convention, returns nonzero if
 * the message was fully processed and zero otherwise
 */
pub fn onUpdateInput(wPar: WPARAM, lPar: LPARAM) -> u32 {
	//this only updates if we have something to handle input in the first place
	if inputMgrHnd {
		TypedHandle<InputManager> castHnd = inputMgrHnd;
		//data handle is the LPARAM
		//WPARAM indicates where window was when event happened, foreground or background
		let mut bufSz: u32 = 0;
		let mut err: LONG = 0;
		let MAX_BTNS = 64;
		let mut usageList: [USAGE; MAX_BTNS] = [USAGE{}; MAX_BTNS];
		let mut static axisVal = 0;
		let mut rawInputBuf = RAWINPUT{};

		//get buffer size
		err = GetRawInputData(lPar as HRAWINPUT, RID_INPUT, NULL, &bufSz, size_of<RAWINPUTHEADER>());
		if err == -1 {
			//we couldn't get the buffer size!
			return 0;
		}

		bufSz = RIBUF_LEN;
		//get the actual data
		err = GetRawInputData(lPar as HRAWINPUT, RID_INPUT, &rawInputBuf, &bufSz, size_of<RAWINPUTHEADER>());
		let DEVICE_ID = rawInputBuf.header.hDevice;
		if err == -1 {
			//we've got a unknown problem
			return 0;
		}

		//now data should be filled
		//do whatever here
		//TODO: We could maybe optimize this by using the type to convert to known type enums in InputManager
		//and restructure InputManager such that all controllers are stored in a 2D array - indexed first by type,
		//then by individual number.
		//By doing so, we could eliminate switches in calls to InputManager.
		match rawInputBuf.header.dwType {
			RIM_TYPEHID => {
					//get controller
					Controller& ctrlr = castHnd->GetControllerByHandle(rawInputBuf.header.hDevice as u64);
					//reinterpret as HID. You'll need the preparsed data.
					let hidDat: RAWHID = rawInputBuf.data.hid;
					
					//get the pointer to that data while we're at it
					if preParDataMap.count(rawInputBuf.header.hDevice as u64) < 1 {
						break;
					}

					PHIDP_PREPARSED_DATA preParDat = preParDataMap[rawInputBuf.header.hDevice as u64];
					
					//first, get the buttons
					//only get page 0 because we're dumb!
					//TODO: test for if buttons AREN'T a range
					U8 index = castHnd->HandleToIndex(rawInputBuf.header.hDevice as u64);
					HIDP_BUTTON_CAPS* btnCaps = btnStats[index];
					U32 btnRange = btnCaps[0].Range.UsageMax - btnCaps[0].Range.UsageMin;

					//clear button state now
					getUsages(	HidP_Input, btnCaps[0].UsagePage, 0, usageList,
								&btnRange as PULONG, preParDat, hidDat.bRawData as PCHAR, hidDat.dwSizeHid);
					ctrlr.ClearAllButtons();
					for i in 0..btnRange {
						ctrlr.SetButton(usageList[i] - btnStats[index][0].Range.UsageMin);
					}
					
					//next, get the axis
					HIDP_VALUE_CAPS* valCaps = valStats[index];
					axisVal = 0;
					for i in 0..ctrlr.Info.NumAxis {
						//TODO: test for if axis is a range
						getUsageValue(	HidP_Input, valCaps[i].UsagePage, 0, valCaps[i].NotRange.Usage,
										&axisVal as PULONG, preParDat, hidDat.bRawData as PCHAR, hidDat.dwSizeHid);
						//now set axis value
						ctrlr.SetAxisRawVal(i, axisVal);
					}
			},
			//otherwise, we can interpret this right here!
			RIM_TYPEMOUSE => {
				if shouldLockMouse {
					//reset the mouse position
					POINT pt = POINT();
					pt.x = clientWidth / 2;
					pt.y = clientHeight / 2;
					ClientToScreen(window, &pt);
					SetCursorPos(pt.x, pt.y);
				}

				//get the needed mouse
				let mut mouse: Mouse& = castHnd->GetMouseByHandle(rawInputBuf.header.hDevice as u64);
				//also setup a shortcut for the data
				let mouseDat: RAWMOUSE& = rawInputBuf.data.mouse;

				//set mouse data
				mouse.SetMousePos(mouseDat.lLastX, mouseDat.lLastY);
				//set mouse wheel as needed
				if((mouseDat.usButtonFlags & RI_MOUSE_WHEEL) != 0)
				{
					mouse.SetWheelDelta(mouseDat.usButtonData);
				}
				let flags: u16 = mouseDat.usButtonFlags;

				//now also set mouse buttons
				//buttons are WEIRD under RI.
				//there's flags indicating buttons being pressed and released,
				//and buttons hold multiple positions
				//(M1 has 01b for down and 10b for up, for instance).
				//since buttons are cleared on update, however, we can just check for button down
				
				//TODO_rust: This screams making the mouse buttons into an array you set here;
				//then you wouldn't have any branches
				let mouse_flags = [[RI_MOUSE_BUTTON_1_DOWN, RI_MOUSE_BUTTON_1_UP], [RI_MOUSE_BUTTON_2_DOWN, RI_MOUSE_BUTTON_2_UP], [RI_MOUSE_BUTTON_3_DOWN, RI_MOUSE_BUTTON_3_UP], [RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP], [RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP]];
				
				use events::MouseButton;
				//TODO_rust: The mouse event interface handles a maximum of 4 buttons;
				//if you want to expand this you'll need to alter the event definition itself.
				let mouse_button_map = [MouseButton::Left, MouseButton::Right, MouseButton::Middle, MouseButton::Other, MouseButton::Other];
				
				for mouse_button in 0..4 {
					let flag_down = mouse_flags[mouse_button][0];
					let flag_up = mouse_flags[mouse_button][1];
					let which_button_changed = mouse_button_map[mouse_button];

					let mouse_state = 1 if (flags & flag_down) != 0 else (2 if (flags & flag_up) != 0 else 0);
					
					match mouse_state {
						use events::WindowEvent::MouseInput;
						use events::ElementState;
						1 => { 
							send_event(Event::WindowEvent {
								window_id: SuperWindowId(WindowId(window)),
								event: MouseInput {
									device_id: DEVICE_ID,
									state: ElementState::Pressed,
									button: which_button_changed,
									modifiers: event::get_key_mods()
								}
							});
						},
						2 => { 
							send_event(Event::WindowEvent {
								window_id: SuperWindowId(WindowId(window)),
								event: MouseInput {
									device_id: DEVICE_ID,
									state: ElementState::Released,
									button: which_button_changed,
									modifiers: event::get_key_mods()
								}
							});
						},
						_ => {}
					}
				}
			},
			RIM_TYPEKEYBOARD => {
				//Two possible states, key down and key up
				Keyboard& kbd = castHnd->GetKeyboardByHandle(rawInputBuf.header.hDevice as u64);
				const RAWKEYBOARD& kbdDat = rawInputBuf.data.keyboard;
				//each message carries ONE VKey.
				//Convert it to a platform-independent keycode.
				use events::VirtualKeyCode;
				let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
				let key_state = kbdDat.Flags & RI_KEY_BREAK;
				let key_down = key_state != RI_KEY_BREAK;
				let key_up = key_state == RI_KEY_BREAK;

				match key_state {
					//KEY UP
					RI_KEY_BREAK => {
						use events::ElementState::Released;
						
						send_event(
							Event::WindowEvent {
								window_id: SuperWindowId(WindowId(window)),
								event: WindowEvent::KeyboardInput {
									device_id: DEVICE_ID,
									input: KeyboardInput {
										state: Released,
										scancode: scancode,
										virtual_keycode: vkey,
										modifiers: event::get_key_mods(),
									},
								}
							}
						);
					},
					//KEY DOWN
					_ => {
						use events::ElementState::Pressed;
						//If the key is F4, this might be an Alt-F4 command.
						//Pass back out to event loop.
						if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
							winuser::DefWindowProcW(window, msg, wparam, lparam)
						}
						else {
							//Otherwise broadcast the key event.
							send_event(
								Event::WindowEvent {
									window_id: SuperWindowId(WindowId(window)),
									event: WindowEvent::KeyboardInput {
										device_id: DEVICE_ID,
										input: KeyboardInput {
											state: Pressed,
											scancode: scancode,
											virtual_keycode: vkey,
											modifiers: event::get_key_mods(),
										}
									}
								}
							);
							// Windows doesn't emit a delete character by default, but in order to make it
							// consistent with the other platforms we'll emit a delete character here.
							if vkey == Some(VirtualKeyCode::Delete) {
								send_event(
									Event::WindowEvent {
										window_id: SuperWindowId(WindowId(window)),
										event: WindowEvent::ReceivedCharacter('\u{7F}'),
									}
								);
							}
						}
					}
				}
			}
		}
		return 0;
	}
	return 0;
}

void Win32Platform::onUpdateGUI(LPARAM lPar)
{
	if(inputMgrHnd)
	{
		TypedHandle<InputManager> castHnd = inputMgrHnd;
		castHnd->SetGUIMousePos(GET_X_LPARAM(lPar) as f32, GET_Y_LPARAM(lPar) as f32);
	}
}