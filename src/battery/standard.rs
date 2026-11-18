use super::*;
use windows_sys::{Win32::Devices::Properties::*, core::GUID};

const MOUSE_CLASS: GUID = GUID::from_u128(0x4d36e96f_e325_11ce_bfc1_08002be10318);
const BATTERY_LIFE: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x49cd1f76_5626_4b17_a4e8_18b4aa1a2213),
    pid: 10,
};
// Windows Bluetooth provider's cached battery property (not a public contract).
const BLUETOOTH_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 2,
};

fn percent(value: u32, min: i32, max: i32) -> Option<u8> {
    if min < 0 || max <= min || value < min as u32 || value > max as u32 {
        return None;
    }
    Some((((value - min as u32) as u64 * 100 + (max - min) as u64 / 2) / (max - min) as u64) as u8)
}

unsafe fn property(node: u32, key: &DEVPROPKEY) -> Option<(u32, Vec<u8>)> {
    let mut kind = 0;
    let mut size = 0;
    CM_Get_DevNode_PropertyW(node, key, &mut kind, null_mut(), &mut size, 0);
    if size == 0 || size > 4096 {
        return None;
    }
    let mut bytes = vec![0; size as usize];
    if CM_Get_DevNode_PropertyW(node, key, &mut kind, bytes.as_mut_ptr(), &mut size, 0)
        != CR_SUCCESS
    {
        return None;
    }
    bytes.truncate(size as usize);
    Some((kind, bytes))
}
fn property_percent(kind: u32, bytes: &[u8]) -> Option<u8> {
    let value = match kind {
        DEVPROP_TYPE_BYTE if bytes.len() == 1 => bytes[0] as u32,
        DEVPROP_TYPE_UINT32 if bytes.len() == 4 => u32::from_le_bytes(bytes.try_into().ok()?),
        _ => return None,
    };
    (value <= 100).then_some(value as u8)
}
unsafe fn node_name(node: u32) -> String {
    for key in [&DEVPKEY_Device_FriendlyName, &DEVPKEY_Device_DeviceDesc] {
        if let Some((DEVPROP_TYPE_STRING, bytes)) = property(node, key) {
            let utf16: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
                .take_while(|v| *v != 0)
                .collect();
            let name = String::from_utf16_lossy(&utf16);
            if !name.is_empty() {
                return name;
            }
        }
    }
    "Mouse".into()
}
unsafe fn system() -> Battery {
    let set = SetupDiGetClassDevsW(&MOUSE_CLASS, null(), null_mut(), DIGCF_PRESENT);
    if set == -1 {
        return Battery::default();
    }
    let mut fallback = Battery::default();
    for index in 0..128 {
        let mut info: SP_DEVINFO_DATA = std::mem::zeroed();
        info.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        if SetupDiEnumDeviceInfo(set, index, &mut info) == 0 {
            break;
        }
        let mut status = 0;
        let mut problem = 0;
        if CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) != CR_SUCCESS
            || status & DN_DEVICE_DISCONNECTED != 0
        {
            continue;
        }
        let mut id = [0u16; 1024];
        if CM_Get_Device_IDW(info.DevInst, id.as_mut_ptr(), id.len() as u32, 0) != CR_SUCCESS {
            continue;
        }
        let length = id.iter().position(|v| *v == 0).unwrap_or(id.len());
        let identity = String::from_utf16_lossy(&id[..length]);
        // Ignore software/remote pointers; follow a physical mouse's parent
        // nodes because Bluetooth providers publish the value on the parent.
        if !identity.starts_with("HID\\")
            && !identity.starts_with("BTH")
            && !identity.starts_with("USB\\")
        {
            continue;
        }
        let mut result = Battery {
            device: node_name(info.DevInst),
            device_id: identity,
            message: "Bateria não informada pelo dispositivo".into(),
            ..Default::default()
        };
        let mut node = info.DevInst;
        for _ in 0..5 {
            for key in [&BATTERY_LIFE, &BLUETOOTH_BATTERY] {
                if let Some((kind, bytes)) = property(node, key) {
                    if let Some(value) = property_percent(kind, &bytes) {
                        result.percent = Some(value);
                        result.source = "Windows".into();
                        result.device = node_name(node);
                        result.message = "Bateria informada pelo Windows".into();
                        SetupDiDestroyDeviceInfoList(set);
                        return result;
                    }
                }
            }
            let mut parent = 0;
            if CM_Get_Parent(&mut parent, node, 0) != CR_SUCCESS {
                break;
            }
            node = parent;
        }
        if fallback.device_id.is_empty() {
            fallback = result;
        }
    }
    SetupDiDestroyDeviceInfoList(set);
    fallback
}
pub(super) unsafe fn product(handle: HANDLE, fallback: &str) -> String {
    let mut name = [0u16; 128];
    if HidD_GetProductString(handle, name.as_mut_ptr().cast(), (name.len() * 2) as u32) {
        let length = name.iter().position(|v| *v == 0).unwrap_or(name.len());
        let text = String::from_utf16_lossy(&name[..length]).trim().to_owned();
        if !text.is_empty() {
            return text;
        }
    }
    fallback.into()
}
unsafe fn hid_percent(handle: HANDLE, ppd: PHIDP_PREPARSED_DATA, caps: &HIDP_CAPS) -> Option<u8> {
    for (kind, count, length) in [
        (
            HidP_Feature,
            caps.NumberFeatureValueCaps,
            caps.FeatureReportByteLength,
        ),
        (
            HidP_Input,
            caps.NumberInputValueCaps,
            caps.InputReportByteLength,
        ),
    ] {
        if count == 0 || count > 128 || length == 0 || length > 4096 {
            continue;
        }
        let mut count = count;
        let mut values = vec![HIDP_VALUE_CAPS::default(); count as usize];
        if HidP_GetValueCaps(kind, values.as_mut_ptr(), &mut count, ppd) != HIDP_STATUS_SUCCESS {
            continue;
        }
        for value in values.iter().take(count as usize) {
            let has_usage = if value.IsRange {
                let range = value.Anonymous.Range;
                range.UsageMin <= 0x20 && range.UsageMax >= 0x20
            } else {
                value.Anonymous.NotRange.Usage == 0x20
            };
            if value.UsagePage != 6 || !has_usage || value.ReportCount != 1 {
                continue;
            }
            let mut report = vec![0u8; length as usize];
            report[0] = value.ReportID;
            let read = if kind == HidP_Feature {
                HidD_GetFeature(handle, report.as_mut_ptr().cast(), length as u32)
            } else {
                HidD_GetInputReport(handle, report.as_mut_ptr().cast(), length as u32)
            };
            if !read {
                continue;
            }
            let mut raw = 0;
            if HidP_GetUsageValue(
                kind,
                6,
                value.LinkCollection,
                0x20,
                &mut raw,
                ppd,
                report.as_ptr(),
                length as u32,
            ) == HIDP_STATUS_SUCCESS
            {
                if let Some(p) = percent(raw, value.LogicalMin, value.LogicalMax) {
                    return Some(p);
                }
            }
        }
    }
    None
}
pub(super) unsafe fn read(candidates: &[Vec<u16>]) -> Battery {
    let mut system = system();
    if system.message.is_empty() {
        system.message = "Nenhum mouse com bateria disponível".into();
    }
    if system.percent.is_some() {
        return system;
    }
    for path in candidates {
        // Access zero permits descriptor/feature queries even when Windows owns
        // the mouse input collection. Never consume pointer input with ReadFile.
        let handle = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            continue;
        }
        let mut ppd = 0;
        if !HidD_GetPreparsedData(handle, &mut ppd) {
            CloseHandle(handle);
            continue;
        }
        let mut caps: HIDP_CAPS = std::mem::zeroed();
        let mut result = None;
        if HidP_GetCaps(ppd, &mut caps) == HIDP_STATUS_SUCCESS
            && caps.UsagePage == 1
            && caps.Usage == 2
        {
            if let Some(percent) = hid_percent(handle, ppd, &caps) {
                result = Some(Battery {
                    percent: Some(percent),
                    device: product(handle, "Mouse HID"),
                    device_id: String::from_utf16_lossy(&path[..path.len() - 1]),
                    source: "HID padrão".into(),
                    message: "Bateria informada pelo dispositivo · HID".into(),
                    ..Default::default()
                });
            }
        }
        HidD_FreePreparsedData(ppd);
        CloseHandle(handle);
        if let Some(result) = result {
            return result;
        }
    }
    system
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_battery_scales_descriptor_range_and_rejects_nulls() {
        assert_eq!(percent(0, 0, 255), Some(0));
        assert_eq!(percent(128, 0, 255), Some(50));
        assert_eq!(percent(255, 0, 255), Some(100));
        assert_eq!(percent(101, 0, 100), None);
        assert_eq!(percent(5, 10, 100), None);
        assert_eq!(percent(30, 10, 110), Some(20));
        assert_eq!(percent(0, 0, 0), None);
    }
    #[test]
    fn windows_values_require_a_percentage_and_correct_type() {
        assert_eq!(property_percent(DEVPROP_TYPE_BYTE, &[92]), Some(92));
        assert_eq!(property_percent(DEVPROP_TYPE_BYTE, &[101]), None);
        assert_eq!(
            property_percent(DEVPROP_TYPE_UINT32, &50u32.to_le_bytes()),
            Some(50)
        );
        assert_eq!(property_percent(DEVPROP_TYPE_STRING, &[50]), None);
    }
}
