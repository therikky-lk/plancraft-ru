//! The only unsafe printing boundary: owned Win32 printer handles and image buffers.
#![allow(unsafe_code)]
use plancraft_core::print::{Orientation, PageSetup, Paper};
use windows_sys::Win32::{
    Foundation::GlobalFree,
    Graphics::Gdi::*,
    Storage::Xps::*,
    System::Memory::{GlobalLock, GlobalUnlock},
    UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow},
};

struct Printer {
    dialog: PRINTDLGW,
    started: bool,
}
impl Drop for Printer {
    fn drop(&mut self) {
        // SAFETY: handles are owned by this dialog, never shared, and released exactly once.
        unsafe {
            if !self.dialog.hDC.is_null() {
                if self.started {
                    AbortDoc(self.dialog.hDC);
                }
                DeleteDC(self.dialog.hDC);
            }
            if !self.dialog.hDevMode.is_null() {
                GlobalFree(self.dialog.hDevMode);
            }
            if !self.dialog.hDevNames.is_null() {
                GlobalFree(self.dialog.hDevNames);
            }
        }
    }
}

pub(super) fn print(
    title: &str,
    s: &PageSetup,
    count: &dyn Fn((f32, f32)) -> anyhow::Result<usize>,
    render: &dyn Fn((f32, f32), usize) -> anyhow::Result<egui::ColorImage>,
) -> anyhow::Result<bool> {
    let (w, h) = s.paper.size();
    let (w, h) = (w * 25.4 / 72.0, h * 25.4 / 72.0);
    let mm = if s.orientation == Orientation::Landscape { (h, w) } else { (w, h) };
    let preview_pages = u16::try_from(count(mm)?)?;
    let mut printer = Printer {
        dialog: PRINTDLGW {
            lStructSize: std::mem::size_of::<PRINTDLGW>() as u32,
            Flags: PD_RETURNDEFAULT,
            nMinPage: 1,
            nMaxPage: preview_pages,
            nFromPage: 1,
            nToPage: preview_pages,
            nCopies: 1,
            ..Default::default()
        },
        started: false,
    };
    // SAFETY: PRINTDLGW is initialized to the documented size. PrintDlg allocates and owns
    // the movable hDevMode/hDevNames blocks until Printer::drop; locks are paired locally.
    unsafe {
        if PrintDlgW(&mut printer.dialog) == 0 {
            anyhow::bail!("Unable to find a printer (Windows error {})", CommDlgExtendedError());
        }
        if !printer.dialog.hDevMode.is_null() {
            let ptr = GlobalLock(printer.dialog.hDevMode).cast::<DEVMODEW>();
            if !ptr.is_null() {
                (*ptr).dmFields |= DM_ORIENTATION | DM_PAPERSIZE;
                (*ptr).Anonymous1.Anonymous1.dmOrientation =
                    if s.orientation == Orientation::Landscape { DMORIENT_LANDSCAPE as i16 } else { DMORIENT_PORTRAIT as i16 };
                (*ptr).Anonymous1.Anonymous1.dmPaperSize = match s.paper {
                    Paper::Letter => DMPAPER_LETTER as i16,
                    Paper::Legal => DMPAPER_LEGAL as i16,
                    Paper::Tabloid => DMPAPER_TABLOID as i16,
                    Paper::A3 => DMPAPER_A3 as i16,
                    Paper::A4 => DMPAPER_A4 as i16,
                };
                GlobalUnlock(printer.dialog.hDevMode);
            }
        }
        printer.dialog.Flags = PD_RETURNDC | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOSELECTION;
        printer.dialog.hwndOwner = GetActiveWindow();
        if PrintDlgW(&mut printer.dialog) == 0 {
            let error = CommDlgExtendedError();
            if error == 0 {
                return Ok(false);
            }
            anyhow::bail!("Unable to open the print dialog (Windows error {error})");
        }
    }
    spool(&mut printer, title, count, render, None)
}

fn spool(
    printer: &mut Printer,
    title: &str,
    count: &dyn Fn((f32, f32)) -> anyhow::Result<usize>,
    render: &dyn Fn((f32, f32), usize) -> anyhow::Result<egui::ColorImage>,
    output: Option<&std::path::Path>,
) -> anyhow::Result<bool> {
    let dc = printer.dialog.hDC;
    anyhow::ensure!(!dc.is_null(), "Printer did not return a device context");
    // SAFETY: the selected printer's DC remains alive in Printer for this whole job.
    let (w, h, dx, dy, ox, oy) = unsafe {
        (
            GetDeviceCaps(dc, PHYSICALWIDTH as i32),
            GetDeviceCaps(dc, PHYSICALHEIGHT as i32),
            GetDeviceCaps(dc, LOGPIXELSX as i32),
            GetDeviceCaps(dc, LOGPIXELSY as i32),
            GetDeviceCaps(dc, PHYSICALOFFSETX as i32),
            GetDeviceCaps(dc, PHYSICALOFFSETY as i32),
        )
    };
    anyhow::ensure!(w > 0 && h > 0 && dx > 0 && dy > 0, "Printer returned invalid paper dimensions");
    // SAFETY: capability queries use the same owned, live printer DC.
    let (print_w, print_h) = unsafe { (GetDeviceCaps(dc, HORZRES as i32), GetDeviceCaps(dc, VERTRES as i32)) };
    anyhow::ensure!(print_w > 0 && print_h > 0, "Printer returned an invalid printable area");
    let fit = (print_w as f64 / w as f64).min(print_h as f64 / h as f64).min(1.0);
    let dest_w = (w as f64 * fit).floor().max(1.0) as i32;
    let dest_h = (h as f64 * fit).floor().max(1.0) as i32;
    let dest_x = ((w - dest_w) / 2 - ox).clamp(0, print_w - dest_w);
    let dest_y = ((h - dest_h) / 2 - oy).clamp(0, print_h - dest_h);
    let mm = (w as f32 / dx as f32 * 25.4, h as f32 / dy as f32 * 25.4);
    let pages = count(mm)?;
    let (first, last) = if printer.dialog.Flags & PD_PAGENUMS != 0 {
        ((printer.dialog.nFromPage as usize).saturating_sub(1), (printer.dialog.nToPage as usize).min(pages))
    } else {
        (0, pages)
    };
    anyhow::ensure!(first < last && last <= pages, "Selected page range is outside the document ({pages} pages)");
    let name = title.chars().filter(|c| *c != '\0').collect::<String>().encode_utf16().chain([0]).collect::<Vec<_>>();
    use std::os::windows::ffi::OsStrExt;
    let output = output.map(|p| p.as_os_str().encode_wide().chain([0]).collect::<Vec<_>>());
    let info = DOCINFOW {
        cbSize: std::mem::size_of::<DOCINFOW>() as i32,
        lpszDocName: name.as_ptr(),
        lpszOutput: output.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
        ..Default::default()
    };
    // SAFETY: name is NUL terminated and lives until EndDoc. All pointers below refer
    // to initialized owned buffers kept alive through their synchronous Win32 calls.
    unsafe {
        anyhow::ensure!(StartDocW(dc, &info) > 0, "Printer could not start the document");
        printer.started = true;
    }
    for page in first..last {
        let image = render(mm, page)?;
        let [iw, ih] = image.size;
        anyhow::ensure!(iw > 0 && ih > 0 && iw.saturating_mul(ih) == image.pixels.len(), "Invalid print image");
        let mut bgra = Vec::with_capacity(image.pixels.len().saturating_mul(4));
        for pixel in &image.pixels {
            bgra.extend([pixel.b(), pixel.g(), pixel.r(), 255]);
        }
        let bitmap = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: i32::try_from(iw)?,
                biHeight: -i32::try_from(ih)?,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: image is a top-down 32bpp DIB; dimensions and buffer length were checked.
        // The destination is inside the printable area, including on large-margin printers.
        unsafe {
            anyhow::ensure!(StartPage(dc) > 0, "Printer could not start page {}", page + 1);
            SetStretchBltMode(dc, HALFTONE);
            SetBrushOrgEx(dc, 0, 0, std::ptr::null_mut());
            let written = StretchDIBits(
                dc,
                dest_x,
                dest_y,
                dest_w,
                dest_h,
                0,
                0,
                iw as i32,
                ih as i32,
                bgra.as_ptr().cast(),
                &bitmap,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            anyhow::ensure!(written != 0 && written != -1, "Printer could not draw page {}", page + 1);
            anyhow::ensure!(EndPage(dc) > 0, "Printer could not finish page {}", page + 1);
        }
    }
    // SAFETY: balances StartDoc; failure keeps started=true so Drop aborts the job.
    unsafe {
        anyhow::ensure!(EndDoc(dc) > 0, "Printer could not finish the document");
    }
    printer.started = false;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires the Windows Microsoft Print to PDF driver; writes target/print-proof-native.pdf"]
    fn microsoft_pdf_driver_prints_the_same_page_images() {
        let device = "Microsoft Print to PDF".encode_utf16().chain([0]).collect::<Vec<_>>();
        let driver = "WINSPOOL".encode_utf16().chain([0]).collect::<Vec<_>>();
        let mut mode =
            DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, dmFields: DM_ORIENTATION | DM_PAPERSIZE, ..Default::default() };
        mode.Anonymous1.Anonymous1.dmOrientation = DMORIENT_LANDSCAPE as i16;
        mode.Anonymous1.Anonymous1.dmPaperSize = DMPAPER_A4 as i16;
        // SAFETY: NUL terminated names and a valid DEVMODE; the returned DC is owned by Printer.
        let dc = unsafe { CreateDCW(driver.as_ptr(), device.as_ptr(), std::ptr::null(), &mode) };
        assert!(!dc.is_null(), "Microsoft Print to PDF is not installed");
        let mut printer = Printer { dialog: PRINTDLGW { hDC: dc, ..Default::default() }, started: false };
        let mut p = plancraft_core::sample::demo_project(chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        p.name = "新製品開発計画 / Product Launch".into();
        if let Some(t) = p.tasks.get_mut(1) {
            t.name = "要件定義と目標の確認".into();
        }
        let st = crate::state::UiState::default();
        let document = crate::printing::document(&p, &st, crate::printing::PrintView::for_view(st.view), &p.page_setup, None, None);
        let ctx = super::super::tests::context();
        let target = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target").canonicalize().unwrap();
        let path = target.join("print-proof-native.pdf");
        let ok = spool(
            &mut printer,
            &p.name,
            &|_| Ok(document.pages.len()),
            &|_, page| {
                let image = super::super::render_document_page(
                    &ctx,
                    document.pages.get(page).ok_or_else(|| anyhow::anyhow!("Invalid print page"))?,
                )?;
                let bytes = image.pixels.iter().flat_map(|p| p.to_array()).collect::<Vec<_>>();
                image::save_buffer(
                    target.join(format!("print-proof-page-{}.png", page + 1)),
                    &bytes,
                    image.size.first().copied().unwrap_or(0) as u32,
                    image.size.get(1).copied().unwrap_or(0) as u32,
                    image::ColorType::Rgba8,
                )?;
                Ok(image)
            },
            Some(&path),
        )
        .unwrap();
        assert!(ok);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF-") && bytes.len() > 100_000);
    }
}
