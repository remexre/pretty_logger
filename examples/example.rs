#[macro_use]
extern crate log;
extern crate pretty_logger;

fn main() {
    pretty_logger::init_to_defaults().unwrap();

    error!("Error");
    warn!("Warn");
    info!("Info");
    debug!("Debug");
    trace!("Trace");
}
