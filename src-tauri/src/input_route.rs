// TTP - Talk To Paste
// Which microphone a dictation records from when the user has not picked one.
//
// Opening a Bluetooth headset's microphone switches the headset from its
// music profile to its call profile for as long as the microphone is open:
// everything the user is listening to drops to call quality, and on AirPods
// the capture itself runs at 24 kHz (every AirPods `capture.start` in the
// 2026-09-27 trace says `rate: 24000`). Waking that profile is also what makes
// Bluetooth starts slow — the 2026-08-30 race began with a 544 ms build.
//
// So when the OS default input is Bluetooth and the Mac has a microphone of
// its own, TTP records from the Mac's. The headphones stay in music quality
// and the start is instant. Two exceptions keep the Bluetooth microphone:
// the lid is closed (Apple silicon and T2 MacBooks disconnect the built-in
// microphone in hardware, so it would record zeros), and the user pinned a
// device in Settings, which always wins.

/// Why a capture uses the device it uses. Written into `capture.start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The user picked this device in Settings.
    Pinned,
    /// The user's pick is not connected; the OS default stands in.
    PinnedMissing,
    /// The OS default input, which is not Bluetooth (or not a Mac).
    SystemDefault,
    /// The OS default is Bluetooth; the Mac's own microphone records instead.
    BuiltInOverBluetooth,
    /// The OS default is Bluetooth and stays: the lid is closed, so the
    /// built-in microphone is disconnected.
    BluetoothLidClosed,
    /// The OS default is Bluetooth and stays: this Mac has no microphone of
    /// its own (a Mac mini, a Mac Pro).
    BluetoothNoBuiltIn,
}

impl Route {
    pub fn slug(self) -> &'static str {
        match self {
            Route::Pinned => "pinned",
            Route::PinnedMissing => "pinned_missing",
            Route::SystemDefault => "system_default",
            Route::BuiltInOverBluetooth => "builtin_over_bluetooth",
            Route::BluetoothLidClosed => "bluetooth_lid_closed",
            Route::BluetoothNoBuiltIn => "bluetooth_no_builtin",
        }
    }
}

/// What the system looks like right now, as far as choosing a microphone goes.
#[derive(Debug, Default)]
pub struct InputScene {
    pub default_is_bluetooth: bool,
    /// Name of the Mac's own input device, as cpal names it.
    pub builtin_input: Option<String>,
    pub lid_closed: bool,
}

/// The decision, separate from the CoreAudio reads so it can be tested.
///
/// Returns the route, and the name of the device to open instead of the OS
/// default when there is one.
pub fn choose(scene: &InputScene) -> (Route, Option<&str>) {
    if !scene.default_is_bluetooth {
        return (Route::SystemDefault, None);
    }
    match scene.builtin_input.as_deref() {
        None => (Route::BluetoothNoBuiltIn, None),
        Some(_) if scene.lid_closed => (Route::BluetoothLidClosed, None),
        Some(name) => (Route::BuiltInOverBluetooth, Some(name)),
    }
}

/// Read the scene from CoreAudio and IOKit. Anything that cannot be read
/// comes back as its harmless value: not Bluetooth, no built-in, lid open.
#[cfg(target_os = "macos")]
pub fn scene() -> InputScene {
    mac::scene()
}

#[cfg(not(target_os = "macos"))]
pub fn scene() -> InputScene {
    InputScene::default()
}

#[cfg(target_os = "macos")]
mod mac {
    use super::InputScene;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::{CFString, CFStringRef};
    use std::ffi::c_void;

    #[repr(C)]
    struct Address {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(
            object: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
        ) -> i32;
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOServiceMatching(name: *const std::ffi::c_char) -> *mut c_void;
        fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
        fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: CFStringRef,
            allocator: *const c_void,
            options: u32,
        ) -> *const c_void;
        fn IOObjectRelease(object: u32) -> i32;
    }

    const fn fourcc(code: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*code)
    }
    const SYSTEM_OBJECT: u32 = 1;
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const SCOPE_INPUT: u32 = fourcc(b"inpt");
    const DEVICES: u32 = fourcc(b"dev#");
    const DEFAULT_INPUT: u32 = fourcc(b"dIn ");
    const TRANSPORT: u32 = fourcc(b"tran");
    const STREAMS: u32 = fourcc(b"stm#");
    const NAME: u32 = fourcc(b"lnam");
    const TRANSPORT_BUILT_IN: u32 = fourcc(b"bltn");
    const TRANSPORT_BLUETOOTH: u32 = fourcc(b"blue");
    const TRANSPORT_BLUETOOTH_LE: u32 = fourcc(b"blea");

    fn read_u32(object: u32, selector: u32, scope: u32) -> Option<u32> {
        let address = Address { selector, scope, element: 0 };
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: `value` is a live u32 and `size` says so.
        let status = unsafe {
            AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, &mut value as *mut u32 as *mut c_void)
        };
        (status == 0).then_some(value)
    }

    fn size_of_property(object: u32, selector: u32, scope: u32) -> Option<u32> {
        let address = Address { selector, scope, element: 0 };
        let mut size: u32 = 0;
        // SAFETY: only writes `size`.
        let status = unsafe { AudioObjectGetPropertyDataSize(object, &address, 0, std::ptr::null(), &mut size) };
        (status == 0).then_some(size)
    }

    fn devices() -> Vec<u32> {
        let Some(size) = size_of_property(SYSTEM_OBJECT, DEVICES, SCOPE_GLOBAL) else {
            return Vec::new();
        };
        let mut ids = vec![0u32; size as usize / std::mem::size_of::<u32>()];
        let mut size = size;
        let address = Address { selector: DEVICES, scope: SCOPE_GLOBAL, element: 0 };
        // SAFETY: `ids` holds exactly `size` bytes.
        let status = unsafe {
            AudioObjectGetPropertyData(SYSTEM_OBJECT, &address, 0, std::ptr::null(), &mut size, ids.as_mut_ptr() as *mut c_void)
        };
        if status != 0 {
            return Vec::new();
        }
        ids.truncate(size as usize / std::mem::size_of::<u32>());
        ids
    }

    fn name(device: u32) -> Option<String> {
        let address = Address { selector: NAME, scope: SCOPE_GLOBAL, element: 0 };
        let mut value: CFStringRef = std::ptr::null();
        let mut size = std::mem::size_of::<CFStringRef>() as u32;
        // SAFETY: `value` is a live pointer-sized slot; the HAL returns a +1
        // CFString, which the create rule below takes ownership of.
        let status = unsafe {
            AudioObjectGetPropertyData(device, &address, 0, std::ptr::null(), &mut size, &mut value as *mut CFStringRef as *mut c_void)
        };
        if status != 0 || value.is_null() {
            return None;
        }
        Some(unsafe { CFString::wrap_under_create_rule(value) }.to_string())
    }

    fn has_input(device: u32) -> bool {
        size_of_property(device, STREAMS, SCOPE_INPUT).is_some_and(|s| s > 0)
    }

    fn is_bluetooth(transport: u32) -> bool {
        transport == TRANSPORT_BLUETOOTH || transport == TRANSPORT_BLUETOOTH_LE
    }

    fn lid_closed() -> bool {
        // SAFETY: IOKit calls with a static C string; the service handle and
        // the +1 CF property are both released.
        unsafe {
            let matching = IOServiceMatching(c"IOPMrootDomain".as_ptr());
            if matching.is_null() {
                return false;
            }
            // Consumes `matching`. 0 is kIOMainPortDefault.
            let service = IOServiceGetMatchingService(0, matching);
            if service == 0 {
                return false;
            }
            let key = CFString::from_static_string("AppleClamshellState");
            let value = IORegistryEntryCreateCFProperty(service, key.as_concrete_TypeRef(), std::ptr::null(), 0);
            IOObjectRelease(service);
            if value.is_null() {
                // Desktops have no clamshell: no lid, so not closed.
                return false;
            }
            let value = CFType::wrap_under_create_rule(value as _);
            value.downcast::<CFBoolean>().map(bool::from).unwrap_or(false)
        }
    }

    pub fn scene() -> InputScene {
        let default_is_bluetooth = read_u32(SYSTEM_OBJECT, DEFAULT_INPUT, SCOPE_GLOBAL)
            .filter(|&id| id != 0)
            .and_then(|id| read_u32(id, TRANSPORT, SCOPE_GLOBAL))
            .is_some_and(is_bluetooth);
        if !default_is_bluetooth {
            // Nothing else matters; skip the device walk and the IOKit read.
            return InputScene::default();
        }
        let builtin_input = devices()
            .into_iter()
            .find(|&id| read_u32(id, TRANSPORT, SCOPE_GLOBAL) == Some(TRANSPORT_BUILT_IN) && has_input(id))
            .and_then(name);
        InputScene { default_is_bluetooth, builtin_input, lid_closed: lid_closed() }
    }

    #[cfg(test)]
    mod tests {
        /// Talks to the real HAL and IOKit: every read must come back without
        /// crashing, whatever this machine has plugged in.
        #[test]
        fn the_scene_reads_on_a_real_mac() {
            eprintln!("scene on this Mac: {:?}", super::scene());
            let _ = super::lid_closed();
            assert!(!super::devices().is_empty(), "the HAL listed no devices");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(bt: bool, builtin: Option<&str>, lid_closed: bool) -> InputScene {
        InputScene { default_is_bluetooth: bt, builtin_input: builtin.map(String::from), lid_closed }
    }

    #[test]
    fn a_wired_default_is_left_alone() {
        assert_eq!(choose(&scene(false, Some("MacBook Air Microphone"), false)), (Route::SystemDefault, None));
    }

    #[test]
    fn bluetooth_default_records_from_the_mac() {
        assert_eq!(
            choose(&scene(true, Some("MacBook Air Microphone"), false)),
            (Route::BuiltInOverBluetooth, Some("MacBook Air Microphone"))
        );
    }

    #[test]
    fn a_closed_lid_keeps_the_headset() {
        assert_eq!(choose(&scene(true, Some("MacBook Air Microphone"), true)), (Route::BluetoothLidClosed, None));
    }

    #[test]
    fn a_mac_without_a_microphone_keeps_the_headset() {
        assert_eq!(choose(&scene(true, None, false)), (Route::BluetoothNoBuiltIn, None));
    }
}
