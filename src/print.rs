//! Printing: one diagram per page, scaled to fit the chosen paper.
//!
//! The pictures of the diagrams are taken by the app (in tiles, at print resolution); this
//! module lays them out on pages and either sends them to a printer (Windows: the system's
//! Print dialog, then GDI) or writes a PDF (every system; on macOS and Linux "Print" opens
//! that PDF in the system viewer to print from there).

use image::RgbImage;
use std::path::Path;

/// Paper sizes offered, with their size in millimetres (portrait) and the Windows DMPAPER code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paper {
    A3,
    A4,
    A5,
    Letter,
    Legal,
    Tabloid,
}

impl Paper {
    pub const ALL: [Paper; 6] = [Paper::A4, Paper::A3, Paper::A5, Paper::Letter, Paper::Legal, Paper::Tabloid];

    pub fn label(self) -> &'static str {
        match self {
            Paper::A3 => "A3 (297 x 420 mm)",
            Paper::A4 => "A4 (210 x 297 mm)",
            Paper::A5 => "A5 (148 x 210 mm)",
            Paper::Letter => "Letter (8.5 x 11 in)",
            Paper::Legal => "Legal (8.5 x 14 in)",
            Paper::Tabloid => "Tabloid (11 x 17 in)",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Paper::A3 => "A3",
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
            Paper::Tabloid => "Tabloid",
        }
    }

    pub fn from_id(s: &str) -> Paper {
        Paper::ALL.into_iter().find(|p| p.id().eq_ignore_ascii_case(s)).unwrap_or(Paper::A4)
    }

    /// Width and height in millimetres, portrait.
    pub fn mm(self) -> (f32, f32) {
        match self {
            Paper::A3 => (297.0, 420.0),
            Paper::A4 => (210.0, 297.0),
            Paper::A5 => (148.0, 210.0),
            Paper::Letter => (215.9, 279.4),
            Paper::Legal => (215.9, 355.6),
            Paper::Tabloid => (279.4, 431.8),
        }
    }

    #[allow(dead_code)]
    fn dmpaper(self) -> i16 {
        match self {
            Paper::Letter => 1,
            Paper::Tabloid => 3,
            Paper::Legal => 5,
            Paper::A3 => 8,
            Paper::A4 => 9,
            Paper::A5 => 11,
        }
    }
}

/// Page orientation: Auto turns each page to suit its diagram.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orient {
    Auto,
    Portrait,
    Landscape,
}

impl Orient {
    pub const ALL: [Orient; 3] = [Orient::Auto, Orient::Portrait, Orient::Landscape];
    pub fn label(self) -> &'static str {
        match self {
            Orient::Auto => "Automatic",
            Orient::Portrait => "Portrait",
            Orient::Landscape => "Landscape",
        }
    }
    pub fn code(self) -> u8 {
        match self {
            Orient::Auto => 0,
            Orient::Portrait => 1,
            Orient::Landscape => 2,
        }
    }
    pub fn from_code(c: u8) -> Orient {
        match c {
            1 => Orient::Portrait,
            2 => Orient::Landscape,
            _ => Orient::Auto,
        }
    }
    /// Whether a page for a picture of this size is landscape.
    pub fn landscape_for(self, w: u32, h: u32) -> bool {
        match self {
            Orient::Auto => w > h,
            Orient::Portrait => false,
            Orient::Landscape => true,
        }
    }
}

/// Blank space kept around the picture on every side of the page.
pub const MARGIN_MM: f32 = 10.0;

/// How many pixels a diagram's picture should have along its longer side for this paper
/// (about 250 dots per inch over the printable length), within sensible limits.
pub fn target_long_px(paper: Paper) -> f32 {
    let (_, h) = paper.mm();
    ((h - 2.0 * MARGIN_MM) / 25.4 * 250.0).clamp(1600.0, 4200.0)
}

/// Fit a w x h picture into an area, keeping its shape; returns (x, y, w, h) centred.
fn fit(iw: f32, ih: f32, ax: f32, ay: f32, aw: f32, ah: f32) -> (f32, f32, f32, f32) {
    let s = (aw / iw).min(ah / ih);
    let (w, h) = (iw * s, ih * s);
    (ax + (aw - w) / 2.0, ay + (ah - h) / 2.0, w, h)
}

// ---------- PDF ----------

/// Write the pages as a PDF: each picture on its own page of the chosen paper.
pub fn write_pdf(path: &Path, pages: &[RgbImage], paper: Paper, orient: Orient) -> Result<(), String> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let n = pages.len();
    // objects: 1 catalog, 2 page tree, then per page: page, content, image
    let page_obj = |i: usize| 3 + i * 3;
    let push = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    push(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page_obj(i))).collect();
    push(&mut out, &mut offsets, format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).as_bytes());
    let pt = |mm: f32| mm / 25.4 * 72.0;
    for (i, img) in pages.iter().enumerate() {
        let (iw, ih) = img.dimensions();
        let (mut pw, mut ph) = (pt(paper.mm().0), pt(paper.mm().1));
        if orient.landscape_for(iw, ih) {
            std::mem::swap(&mut pw, &mut ph);
        }
        let m = pt(MARGIN_MM);
        let (x, y, w, h) = fit(iw as f32, ih as f32, m, m, pw - 2.0 * m, ph - 2.0 * m);
        let content_id = page_obj(i) + 1;
        let image_id = page_obj(i) + 2;
        push(
            &mut out,
            &mut offsets,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 {image_id} 0 R >> >> /Contents {content_id} 0 R >>").as_bytes(),
        );
        let content = format!("q {w:.2} 0 0 {h:.2} {x:.2} {y:.2} cm /Im0 Do Q");
        push(&mut out, &mut offsets, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
        let data = miniz_oxide::deflate::compress_to_vec_zlib(img.as_raw(), 6);
        let mut body = format!("<< /Type /XObject /Subtype /Image /Width {iw} /Height {ih} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\nendstream");
        push(&mut out, &mut offsets, &body);
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    std::fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg_attr(windows, allow(dead_code))]
/// Open a file with the system's default program (the PDF viewer, to print from there).
pub fn open_with_system(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).creation_flags(0x0800_0000).spawn()
    };
    r.map(|_| ()).map_err(|e| e.to_string())
}

// ---------- Windows printing ----------

#[cfg(windows)]
mod win {
    #![allow(non_snake_case, clippy::upper_case_acronyms)]
    use std::ffi::c_void;
    pub type HANDLE = *mut c_void;

    #[repr(C)]
    pub struct PRINTDLGW {
        pub lStructSize: u32,
        pub hwndOwner: HANDLE,
        pub hDevMode: HANDLE,
        pub hDevNames: HANDLE,
        pub hDC: HANDLE,
        pub Flags: u32,
        pub nFromPage: u16,
        pub nToPage: u16,
        pub nMinPage: u16,
        pub nMaxPage: u16,
        pub nCopies: u16,
        pub hInstance: HANDLE,
        pub lCustData: isize,
        pub lpfnPrintHook: *const c_void,
        pub lpfnSetupHook: *const c_void,
        pub lpPrintTemplateName: *const u16,
        pub lpSetupTemplateName: *const u16,
        pub hPrintTemplate: HANDLE,
        pub hSetupTemplate: HANDLE,
    }

    #[repr(C)]
    pub struct DOCINFOW {
        pub cbSize: i32,
        pub lpszDocName: *const u16,
        pub lpszOutput: *const u16,
        pub lpszDatatype: *const u16,
        pub fwType: u32,
    }

    #[repr(C)]
    pub struct BITMAPINFOHEADER {
        pub biSize: u32,
        pub biWidth: i32,
        pub biHeight: i32,
        pub biPlanes: u16,
        pub biBitCount: u16,
        pub biCompression: u32,
        pub biSizeImage: u32,
        pub biXPelsPerMeter: i32,
        pub biYPelsPerMeter: i32,
        pub biClrUsed: u32,
        pub biClrImportant: u32,
    }

    #[repr(C)]
    pub struct BITMAPINFO {
        pub bmiHeader: BITMAPINFOHEADER,
        pub bmiColors: [u32; 1],
    }

    #[link(name = "comdlg32")]
    extern "system" {
        pub fn PrintDlgW(p: *mut PRINTDLGW) -> i32;
        pub fn CommDlgExtendedError() -> u32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        pub fn StartDocW(hdc: HANDLE, di: *const DOCINFOW) -> i32;
        pub fn EndDoc(hdc: HANDLE) -> i32;
        pub fn AbortDoc(hdc: HANDLE) -> i32;
        pub fn StartPage(hdc: HANDLE) -> i32;
        pub fn EndPage(hdc: HANDLE) -> i32;
        pub fn DeleteDC(hdc: HANDLE) -> i32;
        pub fn GetDeviceCaps(hdc: HANDLE, index: i32) -> i32;
        pub fn SetStretchBltMode(hdc: HANDLE, mode: i32) -> i32;
        pub fn SetBrushOrgEx(hdc: HANDLE, x: i32, y: i32, old: *mut c_void) -> i32;
        pub fn StretchDIBits(
            hdc: HANDLE,
            xd: i32,
            yd: i32,
            wd: i32,
            hd: i32,
            xs: i32,
            ys: i32,
            ws: i32,
            hs: i32,
            bits: *const c_void,
            bmi: *const BITMAPINFO,
            usage: u32,
            rop: u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalLock(h: HANDLE) -> *mut c_void;
        pub fn GlobalUnlock(h: HANDLE) -> i32;
        pub fn GlobalFree(h: HANDLE) -> HANDLE;
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn GetForegroundWindow() -> HANDLE;
    }

    pub const PD_NOSELECTION: u32 = 0x4;
    pub const PD_NOPAGENUMS: u32 = 0x8;
    pub const PD_RETURNDC: u32 = 0x100;
    pub const PD_RETURNDEFAULT: u32 = 0x400;
    pub const PD_USEDEVMODECOPIESANDCOLLATE: u32 = 0x40000;
    pub const HORZRES: i32 = 8;
    pub const VERTRES: i32 = 10;
    pub const LOGPIXELSX: i32 = 88;
    pub const LOGPIXELSY: i32 = 90;
    pub const PHYSICALWIDTH: i32 = 110;
    pub const PHYSICALHEIGHT: i32 = 111;
    pub const PHYSICALOFFSETX: i32 = 112;
    pub const PHYSICALOFFSETY: i32 = 113;
    pub const HALFTONE: i32 = 4;
    pub const SRCCOPY: u32 = 0x00CC_0020;
}

/// Print the pages: shows Windows' Print dialog (printer, copies; paper and orientation are
/// set beforehand from the choice in NapkinUML and can still be changed under Properties),
/// then prints one picture per page. Ok(false) when the person cancels.
#[cfg(windows)]
pub fn print_windows(pages: &[RgbImage], paper: Paper, orient: Orient, doc_name: &str) -> Result<bool, String> {
    use std::ptr::{null, null_mut};
    use win::*;
    if pages.is_empty() {
        return Ok(false);
    }
    unsafe {
        let mut pd: PRINTDLGW = std::mem::zeroed();
        pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
        pd.hwndOwner = GetForegroundWindow();
        // the default printer's settings, with our paper and orientation filled in
        pd.Flags = PD_RETURNDEFAULT;
        if PrintDlgW(&mut pd) != 0 && !pd.hDevMode.is_null() {
            let dm = GlobalLock(pd.hDevMode) as *mut u8;
            if !dm.is_null() {
                let fields = dm.add(72) as *mut u32;
                let landscape = orient.landscape_for(pages[0].width(), pages[0].height());
                // DM_ORIENTATION | DM_PAPERSIZE, and no custom paper length/width
                *fields = (*fields | 0x1 | 0x2) & !(0x4 | 0x8);
                *(dm.add(76) as *mut i16) = if landscape { 2 } else { 1 };
                *(dm.add(78) as *mut i16) = paper.dmpaper();
                GlobalUnlock(pd.hDevMode);
            }
        }
        pd.Flags = PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE;
        pd.nCopies = 1;
        let shown = PrintDlgW(&mut pd);
        let free = |pd: &PRINTDLGW| {
            if !pd.hDevMode.is_null() {
                GlobalFree(pd.hDevMode);
            }
            if !pd.hDevNames.is_null() {
                GlobalFree(pd.hDevNames);
            }
        };
        if shown == 0 {
            let err = CommDlgExtendedError();
            free(&pd);
            if err == 0 {
                return Ok(false);
            }
            return Err(if err == 0x1008 { "No printer is installed.".to_string() } else { format!("The Print dialog failed (error {err:#x}).") });
        }
        let hdc = pd.hDC;
        free(&pd);
        if hdc.is_null() {
            return Err("The printer could not be opened.".into());
        }
        let name: Vec<u16> = doc_name.encode_utf16().chain(std::iter::once(0)).collect();
        let di = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: name.as_ptr(), lpszOutput: null(), lpszDatatype: null(), fwType: 0 };
        if StartDocW(hdc, &di) <= 0 {
            DeleteDC(hdc);
            return Err("The printer refused the document.".into());
        }
        // the paper: printable area and where it sits on the sheet, in printer dots
        let hres = GetDeviceCaps(hdc, HORZRES) as f32;
        let vres = GetDeviceCaps(hdc, VERTRES) as f32;
        let dpix = GetDeviceCaps(hdc, LOGPIXELSX).max(72) as f32;
        let dpiy = GetDeviceCaps(hdc, LOGPIXELSY).max(72) as f32;
        let physw = GetDeviceCaps(hdc, PHYSICALWIDTH) as f32;
        let physh = GetDeviceCaps(hdc, PHYSICALHEIGHT) as f32;
        let offx = GetDeviceCaps(hdc, PHYSICALOFFSETX) as f32;
        let offy = GetDeviceCaps(hdc, PHYSICALOFFSETY) as f32;
        let (physw, physh) = if physw > 0.0 && physh > 0.0 { (physw, physh) } else { (hres, vres) };
        let mx = MARGIN_MM / 25.4 * dpix;
        let my = MARGIN_MM / 25.4 * dpiy;
        let left = (mx - offx).max(0.0);
        let top = (my - offy).max(0.0);
        let right = (physw - mx - offx).min(hres);
        let bottom = (physh - my - offy).min(vres);
        let page_landscape = physw > physh;
        let mut result = Ok(true);
        for img in pages {
            // a picture whose shape doesn't suit the sheet (Automatic, mixed pages) is turned
            let wide = img.width() > img.height();
            let turned;
            let img = if orient == Orient::Auto && wide != page_landscape && (img.width() as f32 / img.height() as f32 - 1.0).abs() > 0.15 {
                turned = image::imageops::rotate90(img);
                &turned
            } else {
                img
            };
            let (iw, ih) = img.dimensions();
            let (x, y, w, h) = fit(iw as f32, ih as f32, left, top, right - left, bottom - top);
            // 24-bit colour (rows padded to 4 bytes) is the most widely understood by printer drivers
            let stride = ((iw * 3 + 3) & !3) as usize;
            let mut bits = vec![255u8; stride * ih as usize];
            for (yy, row) in img.rows().enumerate() {
                let dst = &mut bits[yy * stride..yy * stride + iw as usize * 3];
                for (xx, p) in row.enumerate() {
                    dst[xx * 3] = p[2];
                    dst[xx * 3 + 1] = p[1];
                    dst[xx * 3 + 2] = p[0];
                }
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: iw as i32,
                    biHeight: -(ih as i32),
                    biPlanes: 1,
                    biBitCount: 24,
                    biCompression: 0,
                    biSizeImage: bits.len() as u32,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [0],
            };
            if StartPage(hdc) <= 0 {
                result = Err("The printer stopped accepting pages.".into());
                break;
            }
            SetStretchBltMode(hdc, HALFTONE);
            SetBrushOrgEx(hdc, 0, 0, null_mut());
            StretchDIBits(hdc, x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32, 0, 0, iw as i32, ih as i32, bits.as_ptr() as *const _, &bmi, 0, SRCCOPY);
            if EndPage(hdc) <= 0 {
                result = Err("The printer stopped accepting pages.".into());
                break;
            }
        }
        if result.is_ok() {
            EndDoc(hdc);
        } else {
            AbortDoc(hdc);
        }
        DeleteDC(hdc);
        result
    }
}
