use serde::Serialize;
mod standard;
use std::{
    mem::size_of,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
    Foundation::*,
    Storage::FileSystem::*,
    System::{IO::*, Threading::*},
};

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Battery {
    pub percent: Option<u8>,
    pub charging: bool,
    pub stale: bool,
    pub message: String,
    pub device: String,
    pub device_id: String,
    pub source: String,
}
#[derive(Debug, PartialEq)]
pub struct Reading {
    pub percent: u8,
    pub charging: Option<bool>,
    pub info: bool,
}
pub fn parse(mut data: &[u8]) -> Option<Reading> {
    if data.len() == 33 && data[0] == 0 {
        data = &data[1..];
    }
    if data.len() != 32 {
        return None;
    }
    if data[0] == 0xc0 && data[1] == 1 && (1..=100).contains(&data[2]) {
        return Some(Reading {
            percent: data[2],
            charging: None,
            info: false,
        });
    }
    if data[..4] != [0x10, 0, 1, 11] || !(1..=100).contains(&data[13]) || data[14] > 1 {
        return None;
    }
    if data[4..31].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != data[31] {
        return None;
    }
    Some(Reading {
        percent: data[13],
        charging: Some(data[14] == 0),
        info: true,
    })
}
unsafe fn paths() -> Vec<Vec<u16>> {
    unsafe {
        let mut guid = std::mem::zeroed();
        HidD_GetHidGuid(&mut guid);
        let set = SetupDiGetClassDevsW(
            &guid,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == -1 {
            return vec![];
        }
        let mut found = vec![];
        for i in 0..128 {
            let mut interface: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            interface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(set, null(), &guid, i, &mut interface) == 0 {
                break;
            }
            let mut needed = 0;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                null_mut(),
                0,
                &mut needed,
                null_mut(),
            );
            if needed < 8 {
                continue;
            }
            let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = 8;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                detail,
                needed,
                null_mut(),
                null_mut(),
            ) != 0
            {
                let raw = buffer.as_ptr().cast::<u8>().add(4).cast::<u16>();
                let max = (needed as usize - 4) / 2;
                let data = std::slice::from_raw_parts(raw, max);
                let length = data.iter().position(|c| *c == 0).unwrap_or(max);
                let mut path = data[..length].to_vec();
                path.push(0);
                found.push(path);
            }
        }
        SetupDiDestroyDeviceInfoList(set);
        found
    }
}
unsafe fn io(handle: HANDLE, buffer: &mut [u8], write: bool) -> Option<usize> {
    unsafe {
        let event = CreateEventW(null(), 1, 0, null());
        if event.is_null() {
            return None;
        }
        let mut op: OVERLAPPED = std::mem::zeroed();
        op.hEvent = event;
        let mut count = 0;
        let result = if write {
            WriteFile(
                handle,
                buffer.as_ptr(),
                buffer.len() as u32,
                &mut count,
                &mut op,
            )
        } else {
            ReadFile(
                handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut count,
                &mut op,
            )
        };
        let complete = if result != 0 {
            true
        } else if GetLastError() == ERROR_IO_PENDING {
            if WaitForSingleObject(event, 400) == WAIT_OBJECT_0 {
                GetOverlappedResult(handle, &op, &mut count, 0) != 0
            } else {
                CancelIoEx(handle, &op);
                GetOverlappedResult(handle, &op, &mut count, 1);
                false
            }
        } else {
            false
        };
        CloseHandle(event);
        complete.then_some(count as usize)
    }
}
pub fn read() -> Battery {
    unsafe {
        let mut candidates = paths();
        candidates.sort_by_key(|p| {
            !String::from_utf16_lossy(p)
                .to_lowercase()
                .contains("pid_5d2e")
        });
        let known = read_ajazz(&candidates);
        if known.percent.is_some() {
            return known;
        }
        let generic = standard::read(&candidates);
        if generic.percent.is_some() || known.device_id.is_empty() {
            generic
        } else {
            known
        }
    }
}
fn read_ajazz(candidates: &[Vec<u16>]) -> Battery {
    unsafe {
        let mut detected = Battery::default();
        for path in candidates {
            let name = String::from_utf16_lossy(&path).to_lowercase();
            let wireless = name.contains("vid_249a&pid_5c2f") || name.contains("vid_248a&pid_5c2f");
            let wired = [
                "vid_248a&pid_5c2e",
                "vid_248a&pid_5d2e",
                "vid_248a&pid_5e2e",
            ]
            .iter()
            .any(|p| name.contains(p));
            if (!wireless && !wired) || !name.contains("mi_02") {
                continue;
            }
            detected.device_id = String::from_utf16_lossy(&path[..path.len() - 1]);
            detected.device = "Mouse USB".into();
            let handle = CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                continue;
            }
            detected.device = standard::product(handle, "Mouse USB");
            let mut ppd = 0;
            let mut caps: HIDP_CAPS = std::mem::zeroed();
            let valid = HidD_GetPreparsedData(handle, &mut ppd);
            let matching = valid
                && HidP_GetCaps(ppd, &mut caps) >= 0
                && caps.InputReportByteLength == 33
                && caps.OutputReportByteLength == 33;
            if valid {
                HidD_FreePreparsedData(ppd);
            }
            if matching {
                let mut request = [0; 33];
                request[1] = 0x10;
                if io(handle, &mut request, true).is_some() {
                    let started = Instant::now();
                    let mut result = None;
                    while started.elapsed() < Duration::from_millis(1300) {
                        let mut reply = [0; 33];
                        let Some(n) = io(handle, &mut reply, false) else {
                            break;
                        };
                        if let Some(r) = parse(&reply[..n]) {
                            result = Some(Battery {
                                percent: Some(r.percent),
                                charging: wired && r.charging == Some(true),
                                stale: false,
                                message: if wired {
                                    "Conectado · cabo USB"
                                } else {
                                    "Conectado · receptor USB"
                                }
                                .into(),
                                device: detected.device.clone(),
                                device_id: detected.device_id.clone(),
                                source: "AJAZZ/CompX".into(),
                            });
                            if r.info {
                                break;
                            }
                        }
                    }
                    if let Some(result) = result {
                        CloseHandle(handle);
                        return result;
                    }
                }
            }
            CloseHandle(handle);
        }
        detected.message = if !detected.device_id.is_empty() {
            "Sem leitura · mova o mouse para acordá-lo"
        } else {
            "Nenhum mouse com bateria disponível"
        }
        .into();
        detected
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_m129_report() {
        let data = [
            0, 0x10, 0, 1, 11, 77, 49, 50, 57, 54, 1, 149, 1, 0, 63, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 246,
        ];
        assert_eq!(parse(&data).unwrap().percent, 63);
        let mut corrupt = data;
        corrupt[32] ^= 1;
        assert!(parse(&corrupt).is_none());
    }
    #[test]
    fn announcement_and_range() {
        let mut data = [0; 32];
        data[..3].copy_from_slice(&[0xc0, 1, 42]);
        assert_eq!(parse(&data).unwrap().percent, 42);
        data[2] = 101;
        assert!(parse(&data).is_none());
        data[2] = 0;
        assert!(parse(&data).is_none());
        assert!(parse(&data[..4]).is_none());
    }
}
