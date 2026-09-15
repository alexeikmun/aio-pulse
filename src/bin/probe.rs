use aio_pulse::nzxt::lcd::LcdFramebuffer;
use hidapi::HidApi;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Devices::Usb::{
    WinUsb_Free, WinUsb_Initialize, WinUsb_WritePipe, WINUSB_INTERFACE_HANDLE,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};

fn rgba_to_rgb565(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgb565 = Vec::with_capacity(width * height * 2);
    for chunk in rgba.chunks_exact(4) {
        let r = (chunk[0] >> 3) as u16;
        let g = (chunk[1] >> 2) as u16;
        let b = (chunk[2] >> 3) as u16;

        let byte0 = ((r << 3) | (g >> 3)) as u8;
        let byte1 = (((g & 0x07) << 5) | b) as u8;

        rgb565.push(byte0);
        rgb565.push(byte1);
    }
    rgb565
}

fn send_frame(
    dev: &hidapi::HidDevice,
    winusb: WINUSB_INTERFACE_HANDLE,
    frame_data: &[u8],
) -> Result<(), String> {
    unsafe {
        // 1. Send start data transfer via HID
        let mut start_cmd = [0u8; 64];
        start_cmd[0] = 0x36;
        start_cmd[1] = 0x01;
        start_cmd[2] = 0x00;
        start_cmd[3] = 0x01;
        start_cmd[4] = 0x06;
        dev.write(&start_cmd).map_err(|e| format!("HID start error: {}", e))?;

        let mut ack = [0u8; 64];
        let _ = dev.read_timeout(&mut ack, 200);

        // 2. Send Bulk OUT Header (20 bytes)
        let data_len = frame_data.len() as u32;
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&[
            0x12, 0xFA, 0x01, 0xE8, 0xAB, 0xCD, 0xEF, 0x98, 0x76, 0x54, 0x32, 0x10,
        ]);
        header.extend_from_slice(&[0x06, 0x00, 0x00, 0x00]);
        header.extend_from_slice(&data_len.to_le_bytes());

        let mut written = 0u32;
        WinUsb_WritePipe(winusb, 0x02, &header, Some(&mut written), None)
            .map_err(|e| format!("WinUSB header write error: {}", e))?;

        // 3. Send Bulk OUT Payload (chunks of 4096 bytes)
        for chunk in frame_data.chunks(4096) {
            WinUsb_WritePipe(winusb, 0x02, chunk, Some(&mut written), None)
                .map_err(|e| format!("WinUSB payload write error: {}", e))?;
        }

        // 4. Send end data transfer via HID
        let mut end_cmd = [0u8; 64];
        end_cmd[0] = 0x36;
        end_cmd[1] = 0x02;
        dev.write(&end_cmd).map_err(|e| format!("HID end error: {}", e))?;

        let _ = dev.read_timeout(&mut ack, 200);
    }
    Ok(())
}

fn main() {
    println!("=== Streaming Test Frame to Kraken 360 LCD Screen ===");

    // 1. Open HID device
    let api = HidApi::new().expect("Failed to initialize HID API");
    let hid_path = std::ffi::CString::new(
        "\\\\?\\HID#VID_1E71&PID_300E&MI_01#a&2903d999&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}",
    ).unwrap();

    let dev = api.open_path(&hid_path).expect("Failed to open Kraken HID interface");
    println!("Opened HID interface.");

    // 2. Open WinUSB Bulk device
    let winusb_path = "\\\\?\\USB#VID_1E71&PID_300E&MI_00#9&fb10aea&0&0000#{300e300d-7ee7-1125-0724-101503010819}";
    let hstring_path = HSTRING::from(winusb_path);

    unsafe {
        let file_handle = CreateFileW(
            PCWSTR(hstring_path.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
            None,
        ).expect("Failed to open WinUSB CreateFileW");

        let mut winusb = WINUSB_INTERFACE_HANDLE::default();
        WinUsb_Initialize(file_handle, &mut winusb).expect("WinUsb_Initialize failed");
        println!("Opened WinUSB interface.");

        // 3. Render test CPU/GPU load to 240x240 framebuffer
        let mut fb = LcdFramebuffer::new(240, 240);
        fb.render_load(42.0, 67.0);

        // Convert RGBA -> RGB565
        let rgb565_data = rgba_to_rgb565(&fb.pixels, 240, 240);
        println!("Generated RGB565 payload: {} bytes", rgb565_data.len());

        // Send frame twice (initialization / buffer swap)
        print!("Sending frame 1... ");
        if let Err(e) = send_frame(&dev, winusb, &rgb565_data) {
            println!("Error: {}", e);
        } else {
            println!("OK!");
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        print!("Sending frame 2... ");
        if let Err(e) = send_frame(&dev, winusb, &rgb565_data) {
            println!("Error: {}", e);
        } else {
            println!("OK!");
        }

        let _ = WinUsb_Free(winusb);
        let _ = CloseHandle(file_handle);
    }

    println!("Done! Check Kraken 360 LCD display!");
}
