//! Read-only battery telemetry. Enumeration and GATT run on a worker, never the
//! render/settings thread. Only currently connected endpoints enter the snapshot.
use crate::desktop_widgets::WirelessDevice;
use std::{collections::BTreeMap, sync::mpsc, time::{Duration, Instant}};
use windows::{core::{GUID, PCWSTR}, Devices::{Bluetooth::{BluetoothDevice, BluetoothLEDevice, BluetoothConnectionStatus, BluetoothCacheMode, GenericAttributeProfile::GattCommunicationStatus}, Enumeration::DeviceInformation}, Storage::Streams::DataReader};

// WinRT operations can remain Started when a Bluetooth endpoint disappears.
// Bound each operation and the complete scan; cancellation never starts another
// worker, so an unresponsive driver cannot cause a growing thread pool.
macro_rules! wait_until {
    ($operation:expr, $deadline:expr) => {{
        let operation = $operation;
        let deadline = $deadline.min(Instant::now() + Duration::from_secs(3));
        (|| {
            loop {
                if operation.Status()?.0 != 0 { return operation.GetResults(); }
                if Instant::now() >= deadline {
                    let _ = operation.Cancel();
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x800705B4_u32 as i32)));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })()
    }};
}

#[derive(Default)]
pub struct Reader { pending: Option<mpsc::Receiver<Result<Vec<WirelessDevice>,String>>>, last: Option<Instant> }
impl Reader {
    pub fn poll(&mut self) -> Option<Result<Vec<WirelessDevice>,String>> {
        if let Some(rx)=&self.pending {
            match rx.try_recv() {
                Ok(result)=>{self.pending=None;self.last=Some(Instant::now());return Some(result)},
                Err(mpsc::TryRecvError::Disconnected)=>{self.pending=None;self.last=Some(Instant::now());return Some(Err("无线设备读取中断".into()))},
                Err(mpsc::TryRecvError::Empty)=>return None,
            }
        }
        if self.last.is_some_and(|at|at.elapsed()<Duration::from_secs(30)){return None}
        let(tx,rx)=mpsc::channel();self.pending=Some(rx);
        std::thread::spawn(move||{
            use windows::Win32::System::WinRT::{RoInitialize,RoUninitialize,RO_INIT_MULTITHREADED};
            let initialized=unsafe{RoInitialize(RO_INIT_MULTITHREADED)}.is_ok();
            let result=scan();let _=tx.send(result);
            if initialized{unsafe{RoUninitialize()}};
        });
        None
    }
}

fn category(name:&str)->String {
    let name=name.to_lowercase();
    if ["mouse","鼠标","trackpad"].iter().any(|s|name.contains(s)){"mouse"}
    else if ["keyboard","键盘"].iter().any(|s|name.contains(s)){"keyboard"}
    else if ["controller","gamepad","手柄","xbox","dualsense"].iter().any(|s|name.contains(s)){"gamepad"}
    else if ["head","buds","pods","耳机","bose","sony","beats"].iter().any(|s|name.contains(s)){"headphones"}
    else{"device"}.into()
}
fn ble_level(device:&BluetoothLEDevice,deadline:Instant)->Option<u8>{
    let services=wait_until!(device.GetGattServicesForUuidWithCacheModeAsync(GUID::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb),BluetoothCacheMode::Cached).ok()?,deadline).ok()?;
    if services.Status().ok()?!=GattCommunicationStatus::Success{return None}
    let services=services.Services().ok()?;
    for i in 0..services.Size().ok()? {
        let service=services.GetAt(i).ok()?;
        let result=(||{
            let chars=wait_until!(service.GetCharacteristicsForUuidWithCacheModeAsync(GUID::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb),BluetoothCacheMode::Cached).ok()?,deadline).ok()?;
            if chars.Status().ok()?!=GattCommunicationStatus::Success{return None}
            let chars=chars.Characteristics().ok()?;
            for j in 0..chars.Size().ok()? {
                let characteristic=chars.GetAt(j).ok()?;
                // Cached values avoid probing unsupported vendor commands.
                let result=wait_until!(characteristic.ReadValueWithCacheModeAsync(BluetoothCacheMode::Cached).ok()?,deadline).ok()?;
                if result.Status().ok()?!=GattCommunicationStatus::Success{continue}
                let buffer=result.Value().ok()?;if buffer.Length().ok()?<1{continue}
                let value=DataReader::FromBuffer(&buffer).ok()?.ReadByte().ok()?;
                if value<=100{return Some(value)}
            }
            None
        })();
        let _=service.Close();if result.is_some(){return result}
    }
    None
}

// Windows Bluetooth drivers expose this read-only PnP device property when they
// know a classic/HID battery level. Missing or vendor-only properties stay None.
fn pnp_levels(deadline:Instant)->Vec<(String,Option<u8>,Option<String>)>{
    use windows::Win32::{Devices::{DeviceAndDriverInstallation::*,Properties::{DEVPROPTYPE,DEVPROP_TYPE_BYTE,DEVPROP_TYPE_UINT32,DEVPROP_TYPE_GUID,DEVPKEY_Device_ContainerId}},Foundation::DEVPROPKEY};
    let key=DEVPROPKEY{fmtid:GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbE5),pid:2};
    let mut out=vec![];
    unsafe{
        let Ok(set)=SetupDiGetClassDevsW(None,PCWSTR::null(),None,DIGCF_ALLCLASSES|DIGCF_PRESENT)else{return out};
        let mut i=0;
        while Instant::now()<deadline{
            let mut dev=SP_DEVINFO_DATA{cbSize:std::mem::size_of::<SP_DEVINFO_DATA>()as u32,..Default::default()};
            if SetupDiEnumDeviceInfo(set,i,&mut dev).is_err(){break}i+=1;
            let mut id=[0u16;1024];if SetupDiGetDeviceInstanceIdW(set,&dev,Some(&mut id),None).is_err(){continue}
            let id=String::from_utf16_lossy(&id[..id.iter().position(|v|*v==0).unwrap_or(id.len())]).to_lowercase();
            if !id.starts_with("bth"){continue}
            let mut kind=DEVPROPTYPE::default();let mut bytes=[0u8;4];let mut len=0;
            let has_level=SetupDiGetDevicePropertyW(set,&dev,&key,&mut kind,Some(&mut bytes),Some(&mut len),0).is_ok();
            let value=if !has_level{None}else if kind==DEVPROP_TYPE_BYTE&&len==1 {Some(bytes[0]as u32)} else if kind==DEVPROP_TYPE_UINT32&&len==4 {Some(u32::from_le_bytes(bytes))}else{None};
            let level=value.filter(|v|*v<=100).map(|v|v as u8);
            let mut raw=[0u8;16];let mut prop=DEVPROPTYPE::default();let mut count=0;
            let container=if SetupDiGetDevicePropertyW(set,&dev,&DEVPKEY_Device_ContainerId,&mut prop,Some(&mut raw),Some(&mut count),0).is_ok()&&prop==DEVPROP_TYPE_GUID&&count==16{
                Some(format!("{:?}",std::ptr::read_unaligned(raw.as_ptr().cast::<GUID>())))
            }else{None};out.push((id,level,container));
        }
        let _=SetupDiDestroyDeviceInfoList(set);
    }
    out
}
fn scan()->Result<Vec<WirelessDevice>,String>{
    let deadline=Instant::now()+Duration::from_secs(20);let levels=pnp_levels(deadline);let mut devices=BTreeMap::<u64,WirelessDevice>::new();let mut enumerated=false;
    for le in [false,true] {
        if Instant::now()>=deadline{break}
        let selector=if le{BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)}else{BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)};
        let Ok(selector)=selector else{continue};
        let Ok(operation)=DeviceInformation::FindAllAsyncAqsFilter(&selector)else{continue};
        let Ok(items)=wait_until!(operation,deadline)else{continue};enumerated=true;
        for i in 0..items.Size().unwrap_or(0).min(64){
            if Instant::now()>=deadline{break}
            let Ok(info)=items.GetAt(i)else{continue};let Ok(id)=info.Id()else{continue};
            let name=info.Name().map(|s|s.to_string()).unwrap_or_else(|_|"无线设备".into());
            let read=(||->windows::core::Result<(u64,Option<u8>)>{
                if le {
                    let device=wait_until!(BluetoothLEDevice::FromIdAsync(&id)?,deadline)?;
                    if device.ConnectionStatus()?!=BluetoothConnectionStatus::Connected{let _=device.Close();return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x8007048F_u32 as i32)))}
                    let address=device.BluetoothAddress()?;let level=ble_level(&device,deadline);let _=device.Close();Ok((address,level))
                }else{
                    let device=wait_until!(BluetoothDevice::FromIdAsync(&id)?,deadline)?;
                    let connected=device.ConnectionStatus()?==BluetoothConnectionStatus::Connected;let address=device.BluetoothAddress()?;let _=device.Close();
                    if !connected{return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x8007048F_u32 as i32)))}
                    Ok((address,None))
                }
            })();
            let Ok((address,level))=read else{continue};let address_text=format!("{address:012x}");
            let level=level.or_else(||levels.iter().find(|(id,_,_)|id.contains(&address_text)).and_then(|(_,value,_)|*value));
            let entry=devices.entry(address).or_insert_with(||WirelessDevice{container_id:levels.iter().find(|(id,_,_)|id.contains(&address_text)).and_then(|(_,_,c)|c.clone()),id:address_text,name:name.clone(),kind:category(&name),connected:true,battery_percent:None});
            if level.is_some(){entry.battery_percent=level}
        }
    }
    if !enumerated{return Err("系统暂未提供蓝牙设备信息".into())}
    Ok(devices.into_values().collect())
}

#[cfg(test)]mod tests{use super::*;#[test]fn device_categories_cover_common_names(){assert_eq!(category("Magic Mouse"),"mouse");assert_eq!(category("AirPods Pro"),"headphones");assert_eq!(category("无线键盘"),"keyboard");}}

#[cfg(test)]
mod bounded_wait_tests {
    use super::*;
    use std::cell::Cell;
    struct Status(i32);
    struct Operation { completed: bool, cancelled: Cell<bool> }
    #[allow(non_snake_case)]
    impl Operation {
        fn Status(&self)->windows::core::Result<Status>{Ok(Status(if self.completed{1}else{0}))}
        fn Cancel(&self)->windows::core::Result<()>{self.cancelled.set(true);Ok(())}
        fn GetResults(&self)->windows::core::Result<u8>{Ok(76)}
    }
    #[test]
    fn stalled_operation_cancels_at_deadline_without_joining() {
        let operation=Operation{completed:false,cancelled:Cell::new(false)};
        let start=Instant::now();
        assert!(wait_until!(&operation,start).is_err());
        assert!(operation.cancelled.get());
        assert!(start.elapsed()<Duration::from_millis(100));
    }
    #[test]
    fn completed_operation_returns_its_real_value() {
        let operation=Operation{completed:true,cancelled:Cell::new(false)};
        assert_eq!(wait_until!(&operation,Instant::now()).unwrap(),76);
        assert!(!operation.cancelled.get());
    }
}
