//TODO_rust: Raw input functions aren't all necessarily
//in the Win32 DLLs commonly loaded by compatibility libraries.
//They may have to be manually loaded here.
use std::cmp;
use std::mem::size_of;

//DLL handle for USB HID
HMODULE hidDLL = NULL;
//Function pointers for input funcs
typedef BOOLEAN		(__stdcall *FuncGetProdStr)(HANDLE hidHnd, PVOID buf, ULONG buf_len);
let StrGetProdStr = "HidD_GetProductString";
typedef NTSTATUS	(__stdcall *FuncGetCaps)(PHIDP_PREPARSED_DATA preData, PHIDP_CAPS caps);
let StrGetCaps = "HidP_GetCaps";
typedef NTSTATUS	(__stdcall *FuncGetBtnCaps)(HIDP_REPORT_TYPE repType, PHIDP_BUTTON_CAPS btnCaps,
											PUSHORT btnCapsLen, PHIDP_PREPARSED_DATA preData);
let StrGetBtnCaps = "HidP_GetButtonCaps";
typedef NTSTATUS	(__stdcall *FuncGetValCaps)(HIDP_REPORT_TYPE repType, PHIDP_VALUE_CAPS valCaps,
										   PUSHORT valCapsLen, PHIDP_PREPARSED_DATA preData);
let StrGetValCaps = "HidP_GetValueCaps";
typedef NTSTATUS	(__stdcall *FuncGetUsgs)(HIDP_REPORT_TYPE repType, USAGE usgPage,
										USHORT lnkColl, PUSAGE usgList, PULONG usgLen,
										PHIDP_PREPARSED_DATA preData, PCHAR report,
										ULONG repLen);
let StrGetUsgs = "HidP_GetUsages";
typedef NTSTATUS	(__stdcall *FuncGetUsgVal)(HIDP_REPORT_TYPE repType, USAGE usgPage,
										USHORT lnkColl, USAGE usg, PULONG usgVal,
										PHIDP_PREPARSED_DATA preData, PCHAR report,
										ULONG repLen);
let StrGetUsgVal = "HidP_GetUsageValue";
typedef NTSTATUS	(__stdcall *FuncGetUsgValArr)(HIDP_REPORT_TYPE repType, USAGE usgPage,
										USHORT lnkColl, USAGE usg, PCHAR usgVal,
										USHORT usgValByteLen, PHIDP_PREPARSED_DATA preData, 
										PCHAR report, ULONG repLen);
let StrGetUsgValArr = "HidP_GetUsageValueArray";

FuncGetProdStr getProductString;
FuncGetCaps getCaps;
FuncGetBtnCaps getButtonCaps;
FuncGetValCaps getValueCaps;
FuncGetUsgs getUsages;
FuncGetUsgVal getUsageValue;
FuncGetUsgValArr getUsageValueArray;

/**
 * Gets the data for a HID, processing it into a winit-compatible info struct,
 * a platform-specific info struct,
 * and a preparsed data map also specific to the platform.
 */
pub unsafe fn get_hid_properties(out_num_devices: &mut u32, out_device_winit_info: &mut ???, out_device_platform_info: mut& ???, out_preparsed_data_map: mut& Map<???>) {
	//log that this is an actual device we found
	LogV(String("\tFound HID ") + out_device_winit_info.PlatHandle);
	*out_num_devices += 1;
	//#pragma region Init HID Data
	out_device_winit_info.Type = InputDevInfo::HID;
	//fill in additional data
	//also need to get the preparsed data
	let mut buf_len = 0u32;
	let mut err = GetRawInputDeviceInfo(out_device_platform_info.hDevice, RIDI_PREPARSEDDATA, NULL, &buf_len);
	//if we somehow couldn't get the preparsed data size,
	//there's no getting more information here;
	//go to the next controller.
	if err < -1 {
		LogW(String("Could not get allocation size for device ") + i + "!");
		Win32Helpers::LogLastError();
		//invalidate this device
		out_device_winit_info.PlatHandle = 0;
		out_device_winit_info.NumAxis = 0;
		out_device_winit_info.NumBtns = 0;
		break;
	}

	//save this data to the map
	//TODO_rust: How do we do an arbitrary mem:: allocation?
	out_preparsed_data_map[out_device_winit_info.PlatHandle] = (PHIDP_PREPARSED_DATA)Allocator::_CustomMalloc(buf_len, PLAT_ALLOC, "RawDevListAlloc", __FILE__, __LINE__);
	
	//load preparsed data
	err = GetRawInputDeviceInfo(out_device_winit_info.PlatHandle as HANDLE, RIDI_PREPARSEDDATA, out_preparsed_data_map[out_device_winit_info.PlatHandle], &buf_len);
	if err < -1 {
		//if we couldn't get the preparsed data
		//report failure
		Log::W(String("Could not get dev out_device_winit_info for device ") + i + "!");
		Win32Helpers::LogLastError();
		//and invalidate this device
		out_device_winit_info.PlatHandle = 0;
		out_device_winit_info.NumAxis = 0;
		out_device_winit_info.NumBtns = 0;
		break;
	}
	LogV(String("\tPulled device out_device_winit_info for ") + out_device_winit_info.PlatHandle);
	
	//get detailed data now
	getCaps(out_preparsed_data_map[out_device_winit_info.PlatHandle], &ctrlStatsTemp[i]);

	//Capabilities don't directly correspond to individual physical controls;
	//for instance, one button capability might represent a range of buttons
	//like the F1-F12 keys on a keyboard.
	//Because of this we need to loop through each capability and
	//get its specific info.
	let mut numBtnCaps: u16 = ctrlStatsTemp[i].NumberInputButtonCaps;
	out_device_winit_info.NumBtns = 0;
	let mut numValCaps: u16 = ctrlStatsTemp[i].NumberInputValueCaps;
	out_device_winit_info.NumAxis = 0;

	//make cap arrays for this device
	btnStatsTemp = CustomArrayNew<HIDP_BUTTON_CAPS>(numBtnCaps, PLAT_ALLOC, "RawDevListAlloc");
	valStatsTemp = CustomArrayNew<HIDP_VALUE_CAPS>(numValCaps, PLAT_ALLOC, "RawDevListAlloc");
	
	//fill cap arrays
	LogV(String("\tLoading button out_device_winit_info for ") + out_device_winit_info.PlatHandle + "...");
	getButtonCaps(HidP_Input, btnStatsTemp, &numBtnCaps, out_preparsed_data_map[out_device_winit_info.PlatHandle]);
	LogV(String("\tLoaded button out_device_winit_info for ") + out_device_winit_info.PlatHandle + ", parsing data");
	for i in 0..numBtnCaps {
		let btnCap: HIDP_BUTTON_CAPS& = btnStatsTemp[i] as HIDP_BUTTON_CAPS&;
		if btnCap.IsRange {
			out_device_winit_info.NumBtns += (btnCap.Range.UsageMax - btnCap.Range.UsageMin);
		}
		else {
			out_device_winit_info.NumBtns++;
		}
	}
	LogV(String("\tParsed button out_device_winit_info for ") + out_device_winit_info.PlatHandle);

	LogV(String("\tLoading axis out_device_winit_info for ") + out_device_winit_info.PlatHandle + "...");
	getValueCaps(HidP_Input, valStatsTemp, &numValCaps, out_preparsed_data_map[out_device_winit_info.PlatHandle]);
	//cap the number of axii to the out_device_winit_info struct's maximum axii,
	//to avoid corrupting the structure
	numValCaps = cmp::min(numValCaps, Controller::MAX_AXIS as u16);
	LogV(String("\tLoaded axis out_device_winit_info for ") + out_device_winit_info.PlatHandle + ", parsing data");
	
	//setup axis data
	for i in 0..numValCaps {
		LogV(String("Parsing axis ") + j + " for device " + out_device_winit_info.PlatHandle);
		let valCap: &HIDP_VALUE_CAPS = valStatsTemp[i] as &HIDP_VALUE_CAPS;
		if(valCap.IsRange) {
			out_device_winit_info.NumAxis += (valCap.Range.UsageMax - valCap.Range.UsageMin);
		}
		else {
			out_device_winit_info.NumAxis++;
		}

		//hat switches are special; don't do any scaling to their values
		//(midpoint = 0 and halfRange = 1)
		if valCap.NotRange.Usage == 0x39 {
			out_device_winit_info.Axii[i].HalfRange = 1;
			out_device_winit_info.Axii[i].Midpoint = 0;
		}
		else {
			//if there's no specified max, we'll need to make an assumption
			//some controllers (like the 360) use REALLY high max values (0xffffffff)
			//that would be considered negative values if cast to signed values.
			//this is invalid too!
			if valCap.LogicalMax <= 0 {
				out_device_winit_info.Axii[i].HalfRange = ((AXIS_MAX - AXIS_MIN) as f32) / 2;
				out_device_winit_info.Axii[i].Midpoint = AXIS_MID;
			}
			else {
				let max = valCap.LogicalMax as f32;
				let min = valCap.LogicalMin as f32;
			
				out_device_winit_info.Axii[i].HalfRange = (max - min) / 2;
				//if the half range is 0, this is going to be a huge pain!!!
				if out_device_winit_info.Axii[i].HalfRange == 0.0f32 {
					out_device_winit_info.Axii[i].HalfRange = ((AXIS_MAX - AXIS_MIN) as f32) / 2;
					out_device_winit_info.Axii[i].Midpoint = AXIS_MID;
				}
				else {
					out_device_winit_info.Axii[i].Midpoint = (min + out_device_winit_info.Axii[i].HalfRange);
				}
			}
		}
	}
	LogV(String("\tParsed axis out_device_winit_info for ") + out_device_winit_info.PlatHandle);
}

/**
 * TODO_rust
 */
pub unsafe fn build_hid_detail_list() {
	if inputMgrHnd {
		LogV("Building HID details...");
		let inputMgr: TypedHandle<InputManager> = inputMgrHnd;
		numHID = inputMgr->NumHID();
		LogV(String("Found ") + numHID + " HIDs");
		//note that this should only be done when the cap data's invalid
		L_ASSERT(!ctrlStats && !btnStats & !valStats && "Tried to build input device list when list has already been built!");

		//only HIDs need this data, all else can be directly read via API calls
		//TODO_rust: move to rust allocations/memclears
		ctrlStats = LArrayNew(HIDP_CAPS, numHID, INPUT_ALLOC, "RawDevListAlloc");
		btnStats = LArrayNew(HIDP_BUTTON_CAPS*, numHID, INPUT_ALLOC, "RawDevListAlloc");
		memset(btnStats, 0, numHID*size_of<&HIDP_BUTTON_CAPS>());
		valStats = LArrayNew(HIDP_VALUE_CAPS*, numHID, INPUT_ALLOC, "RawDevListAlloc");
		memset(valStats, 0, numHID*size_of<&HIDP_VALUE_CAPS>());

		//retraverse device list!
		for i in 0...numHID {
			//check for index's existence in manager
			let ctrlr: Controller& = inputMgr->GetController(i);
			let preParCtrlr = preParDataMap[ctrlr.Info.PlatHnd];
			LogV(String("Controller ") + i + " found with handle " + HexStrFromVal(ctrlr.Info.PlatHnd));
			
			//get detailed data now
			getCaps(preParCtrlr, &ctrlStats[i]);
			
			//make cap arrays for this device
			let numBtnCaps = ctrlStats[i].NumberInputButtonCaps;
			let numValCaps = ctrlStats[i].NumberInputValueCaps;
			btnStats[i] = LArrayNew(HIDP_BUTTON_CAPS, numBtnCaps, PLAT_ALLOC, "RawDevListAlloc");
			valStats[i] = LArrayNew(HIDP_VALUE_CAPS, numValCaps, PLAT_ALLOC, "RawDevListAlloc");
			
			//fill cap arrays
			getButtonCaps(HidP_Input, btnStats[i], &numBtnCaps, preParCtrlr);
			getValueCaps(HidP_Input, valStats[i], &numValCaps, preParCtrlr);
		}
		LogV("Built HID details");
	}
	else {
		LogW("No input manager attached, couldn't build HID details!");
	}
}