/*!
 * Handlers for raw input events such as WM_INPUT.
 */

//Input callbacks.

pub fn onUpdateInput(wPar: WPARAM, lPar: LPARAM) -> u32 {
	//this only updates if we have something to handle input in the first place
	if inputMgrHnd {
		TypedHandle<InputManager> castHnd = inputMgrHnd;
		//data handle is the LPARAM
		//WPARAM indicates where window was when event happened, foreground or background
		U32 bufSz = 0;
		let err: LONG = 0;
		static const U32 MAX_BTNS = 64;
		static USAGE usageList[MAX_BTNS];
		static U32 axisVal = 0;

		//get buffer size
		err = GetRawInputData((HRAWINPUT)lPar, RID_INPUT, NULL, &bufSz, sizeof(RAWINPUTHEADER));
		if err == -1 {
			//we couldn't get the buffer size!
			return 0;
		}

		bufSz = RIBUF_LEN;
		//get the actual data
		err = GetRawInputData((HRAWINPUT)lPar, RID_INPUT, rawInputBuf, &bufSz, sizeof(RAWINPUTHEADER));
		RAWINPUT* riBuf = (RAWINPUT*)(void*)rawInputBuf;
		if err == -1 {
			//we've got a unknown problem
			return false;
		}

		//now data should be filled
		//do whatever here
		//TODO: We could maybe optimize this by using the type to convert to known type enums in InputManager
		//and restructure InputManager such that all controllers are stored in a 2D array - indexed first by type,
		//then by individual number.
		//By doing so, we could eliminate switches in calls to InputManager.
		match riBuf->header.dwType {
			RIM_TYPEHID => {
					//get controller
					Controller& ctrlr = castHnd->GetControllerByHandle((U64)riBuf->header.hDevice);
					//reinterpret as HID. You'll need the preparsed data.
					RAWHID& hidDat = riBuf->data.hid;
					
					//get the pointer to that data while we're at it
					if preParDataMap.count((U64)riBuf->header.hDevice) < 1 {
						break;
					}

					PHIDP_PREPARSED_DATA preParDat = preParDataMap[(U64)riBuf->header.hDevice];
					
					//first, get the buttons
					//only get page 0 because we're dumb!
					//TODO: test for if buttons AREN'T a range
					U8 index = castHnd->HandleToIndex((U64)riBuf->header.hDevice);
					HIDP_BUTTON_CAPS* btnCaps = btnStats[index];
					U32 btnRange = btnCaps[0].Range.UsageMax - btnCaps[0].Range.UsageMin;

					//clear button state now
					getUsages(	HidP_Input, btnCaps[0].UsagePage, 0, usageList,
								(PULONG)&btnRange, preParDat, (PCHAR)hidDat.bRawData, hidDat.dwSizeHid);
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
										(PULONG)&axisVal, preParDat, (PCHAR)hidDat.bRawData, hidDat.dwSizeHid);
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
				let mut mouse: Mouse& = castHnd->GetMouseByHandle((U64)riBuf->header.hDevice);
				//also setup a shortcut for the data
				let mouseDat: RAWMOUSE& = riBuf->data.mouse;

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
				Keyboard& kbd = castHnd->GetKeyboardByHandle((U64)riBuf->header.hDevice);
				const RAWKEYBOARD& kbdDat = riBuf->data.keyboard;
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
		castHnd->SetGUIMousePos((F32)GET_X_LPARAM(lPar), (F32)GET_Y_LPARAM(lPar));
	}
}