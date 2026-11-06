//! Desktop spooler adapter. Discovery and submission never run on the UI thread.
use gridcraft_ui_egui::printing::{Discover, Duplex, Printer, Settings, Submit};

/// Returns a printer-discovery closure that enumerates the system's printers on a named background thread and sends the mapped list through a channel.
pub fn discover() -> Discover {
    Box::new(|| {
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = std::thread::Builder::new().name("print-discovery".into()).spawn(move || {
            let _ = sender.send(list());
        });
        receiver
    })
}

/// Windows: the spooler's queue names and the default queue, straight from Win32.
///
/// `craft_print::printers()` also asks every driver for its PrintCapabilities to report duplex
/// support. An unreachable shared queue can stall each of those COM calls for seconds, leaving the
/// Print page with an empty printer list for the best part of a minute. The desktop UI does not
/// show that flag, so enumeration alone is enough here; submission still reads the selected
/// driver's capabilities when it prints.
#[cfg(windows)]
fn list() -> Vec<Printer> {
    let Ok(devices) = winprint::printer::PrinterDevice::all() else {
        return Vec::new();
    };
    let default = winsafe::GetDefaultPrinter().ok().filter(|name| !name.is_empty());
    let mut printers: Vec<Printer> = devices
        .iter()
        .map(|device| device.name())
        .filter(|name| !name.trim().is_empty())
        .map(|name| Printer { name: name.to_owned(), label: String::new(), default: default.as_deref() == Some(name) })
        .collect();
    printers.sort_by(|a, b| a.name.cmp(&b.name));
    printers.dedup_by(|a, b| a.name == b.name);
    printers
}

#[cfg(not(windows))]
fn list() -> Vec<Printer> {
    craft_print::printers().into_iter().map(|printer| Printer { name: printer.name, label: printer.description, default: printer.default }).collect()
}

// Keep submission workers reachable until normal app shutdown; dropping a JoinHandle would
// let process exit kill PowerShell/CUPS before it hands the job to the spooler.
static JOBS: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>> = std::sync::Mutex::new(Vec::new());

pub fn finish_pending_jobs() {
    let workers = std::mem::take(&mut *JOBS.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    for worker in workers {
        if worker.join().is_err() {
            log::error!("the print worker stopped unexpectedly during shutdown");
        }
    }
}

pub fn submit() -> Submit {
    Box::new(|bytes, settings: Settings| {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new().name("print-submit".into()).spawn(move || {
            let job = craft_print::Job {
                printer: Some(settings.printer),
                copies: settings.copies.clamp(1, 999),
                duplex: settings.duplex.map(|value| match value {
                    Duplex::LongEdge => craft_print::Duplex::LongEdge,
                    Duplex::ShortEdge => craft_print::Duplex::ShortEdge,
                    Duplex::Off => craft_print::Duplex::Off,
                }),
                grayscale: settings.grayscale,
                scaling: craft_print::Scaling::ShrinkToFit,
                title: "GridCraft document".into(),
                ..Default::default()
            };
            let _ = sender.send(craft_print::submit(&bytes, &job).map_err(|error| error.to_string()));
        });
        if let Ok(worker) = worker {
            let mut jobs = JOBS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            jobs.retain(|job| !job.is_finished());
            jobs.push(worker);
        }
        receiver
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shutdown_waits_for_pending_submission_workers() {
        let (release, wait) = std::sync::mpsc::channel();
        let (completed, completion) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            wait.recv().unwrap();
            completed.send(()).unwrap();
        });
        JOBS.lock().unwrap().push(worker);
        let shutdown = std::thread::spawn(finish_pending_jobs);
        assert!(completion.try_recv().is_err(), "the print worker is still pending");
        release.send(()).unwrap();
        shutdown.join().unwrap();
        completion.recv().unwrap();
        assert!(JOBS.lock().unwrap().is_empty(), "workers were joined before shutdown returned");
    }
}
