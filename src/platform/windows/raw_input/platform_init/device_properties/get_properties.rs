use super::get_hid_properties;

use std::mem::size_of;

//TODO_rust:
//Make return type into Result<(InputDevInfo, u32), (some error type)>
/**
 * Retrieves device info for all raw input devices available to the window.
 * Should be called after platform_init::register_input_devices().
 */
pub unsafe fn get_device_properties(outDevs: &mut u32) -> TypedHandle<InputDevInfo> {
	LogV("Finding input devices...");
	let mut numDevs = 0u32;
	//Get the number of controllers
	GetRawInputDeviceList(NULL, &numDevs, size_of<RAWINPUTDEVICELIST>());
	//notify output var of the device count
	LogD(String("Found ") + numDevs + " controllers");
	*outDevs = 0;
	//if there's no applicable devices, give up
	if numDevs < 1 {
		LogW("No raw input devices attached!");
		return 0;
	}
	//now create the device info
	let mut devices: RAWINPUTDEVICELIST* = CustomArrayNew<RAWINPUTDEVICELIST>(numDevs, PLAT_ALLOC, "RawDevListAlloc");
	memset(devices, 0, numDevs*size_of<RAWINPUTDEVICELIST>());
	let mut devInfoList: InputDevInfo* = CustomArrayNew<InputDevInfo>(numDevs, PLAT_ALLOC, "RawDevListAlloc");
	
	//register the info list with handle system
	let infoHnd: TypedHandle<InputDevInfo> = HandleMgr::RegisterPtr(devInfoList);
	//if we couldn't get a handle, also give up
	if !infoHnd.GetHandle() {
		LogW("Couldn't get handle for device info!");
		return 0;
	}

	//get actual device data
	GetRawInputDeviceList(devices, &numDevs, size_of<RAWINPUTDEVICELIST>());

	//init capability information array
	let mut ctrlStatsTemp: HIDP_CAPS* = CustomArrayNew<HIDP_CAPS>(numDevs, PLAT_ALLOC, "RawDevListAlloc");
	memset(ctrlStatsTemp, 0, numDevs*size_of<HIDP_CAPS>());
	
	//Make the HID capability lists.
	//Note that these are both arrays of arrays;
	//each device can have more than one capability.
	//In practice, game controllers & joysticks only have multiple value caps,
	//and one usage range for buttons.
	let mut btnStatsTemp: HIDP_BUTTON_CAPS** = CustomArrayNew<HIDP_BUTTON_CAPS*>(numDevs, PLAT_ALLOC, "RawDevListAlloc");
	memset(btnStatsTemp, 0, numDevs*size_of<HIDP_BUTTON_CAPS*>());
	let mut valStatsTemp: HIDP_VALUE_CAPS** = CustomArrayNew<HIDP_VALUE_CAPS*>(numDevs, PLAT_ALLOC, "RawDevListAlloc");
	memset(valStatsTemp, 0, numDevs*size_of<HIDP_VALUE_CAPS*>());

	//fill RI device list
	for i in 0..numDevs {
		let err: LONG = 0;
		let mut info: InputDevInfo& = devInfoList[i];
		//set handle
		info.PlatHandle = devices[i].hDevice as size_t;
		LogV(String("Setting device ") + i + " handle to " + info.PlatHandle);

		//set type
		match devices[i].dwType {
		RIM_TYPEHID => {
				//TODO_rust
				get_hid_properties(???);
			},
			//Mice and keyboards are much simpler, note the type and move on.
			//All the input manager needs to know is the device handle, and we've taken care of that.
		RIM_TYPEMOUSE => {
				LogV(String("\tFound mouse ") + info.PlatHandle);
				//note that this is an actual device
				*outDevs += 1;
				info.Type = InputDevInfo::Mouse;
				info.NumAxis = 0;
				info.NumBtns = 0;
			},
		RIM_TYPEKEYBOARD => {
				LogV(String("\tFound keyboard ") + info.PlatHandle);
				//note that this is an actual device
				*outDevs += 1;
				info.Type = InputDevInfo::Keyboard;
				info.NumAxis = 0;
				info.NumBtns = 0;
			},
		_ => {
				LogV(String("Found invalid device ") + info.PlatHandle + "!");
				info.PlatHandle = 0;
				info.NumAxis = 0;
				info.NumBtns = 0;
			}
		}
		LogV(String("\tDevice ID for device ") + i + ": " + info.PlatHandle);
	}

	//TODO_rust: this is normally where we'd drop the allocations
	// CustomArrayDelete(devices);
	// for(U32 i = 0; i < numDevs; ++i)
	// {
	// 	CustomArrayDelete(btnStatsTemp[i]);
	// 	CustomArrayDelete(valStatsTemp[i]);
	// }
	// CustomArrayDelete(btnStatsTemp);
	// CustomArrayDelete(valStatsTemp);
	// CustomArrayDelete(ctrlStatsTemp);

	LogV("All input devices found, listing output: ");
	for(unsigned int i = 0; i < *outDevs; ++i)
	{
		LogV(String("Device ") + i + ": " + infoHnd.Ptr()[i].PlatHandle);
	}

	infoHnd
}