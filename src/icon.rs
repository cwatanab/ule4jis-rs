use std::cell::RefCell;

use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, IMAGE_FLAGS,
};

const ICON_ENABLED: &[u8] = include_bytes!("../assets/app_enabled.ico");
const ICON_DISABLED: &[u8] = include_bytes!("../assets/app_disabled.ico");

thread_local! {
    static CACHED_ENABLED: RefCell<Option<HICON>> = const { RefCell::new(None) };
    static CACHED_DISABLED: RefCell<Option<HICON>> = const { RefCell::new(None) };
}

pub fn get_icon(active: bool) -> Option<HICON> {
    if active {
        get_cached(ICON_ENABLED, &CACHED_ENABLED)
    } else {
        get_cached(ICON_DISABLED, &CACHED_DISABLED)
    }
}

pub fn cleanup_icons() {
    CACHED_ENABLED.with(destroy_cached);
    CACHED_DISABLED.with(destroy_cached);
}

fn destroy_cached(cell: &RefCell<Option<HICON>>) {
    let Ok(mut icon) = cell.try_borrow_mut() else {
        return;
    };
    if let Some(h) = icon.take().filter(|h| !h.is_invalid()) {
        unsafe {
            DestroyIcon(h).ok();
        }
    }
}

fn get_cached(
    data: &'static [u8],
    cache: &'static std::thread::LocalKey<RefCell<Option<HICON>>>,
) -> Option<HICON> {
    cache.with(|c| {
        let Ok(mut opt) = c.try_borrow_mut() else {
            return None;
        };
        if opt.is_none() {
            *opt = ico_to_hicon(data);
        }
        opt.filter(|h| !h.is_invalid())
    })
}

fn ico_to_hicon(data: &[u8]) -> Option<HICON> {
    if data.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;

    let mut best = (0u32, 0u32, 0u32);

    for i in 0..count {
        let off = 6 + i * 16;
        if off + 16 > data.len() {
            return None;
        }
        let w = data[off] as u32;
        let h = data[off + 1] as u32;
        let w = if w == 0 { 256 } else { w };
        let h = if h == 0 { 256 } else { h };
        let area = w * h;
        let img_size = le_u32(data, off + 8);
        let img_offset = le_u32(data, off + 12);

        let is_32 = w == 32 && h == 32;
        let better = best.2 == 0 || (best.2 != 1024 && (is_32 || area > best.2));

        if better {
            best = (img_offset, img_size, area);
        }
    }

    let (offset, size, _) = best;
    if size == 0 {
        return None;
    }

    let image_data = data.get(offset as usize..offset.checked_add(size)? as usize)?;

    let hicon = unsafe {
        CreateIconFromResourceEx(
            image_data,
            windows::Win32::Foundation::BOOL::from(true),
            0x00030000,
            0,
            0,
            IMAGE_FLAGS(0),
        )
    };

    match hicon {
        Ok(h) if !h.is_invalid() => Some(h),
        _ => None,
    }
}

fn le_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
