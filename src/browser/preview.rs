use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub struct Preview {
    preview_rx: std::sync::mpsc::Receiver<PathBuf>,
}

impl Preview {
    pub fn new(rx: std::sync::mpsc::Receiver<PathBuf>) -> Self {
        Preview { preview_rx: rx }
    }

    pub fn start_sample_loop(&mut self) {
        while let Ok(path) = self.preview_rx.recv() {
            let file = BufReader::new(File::open(path).expect("Open source file"));
            let (_, stream_handle) = OutputStream::try_default().expect("Default output stream");
            let source = Decoder::new(file).expect("Create decoder");
            let sink = Sink::try_new(&stream_handle).unwrap();
            sink.append(source);
            sink.play();
            sink.sleep_until_end();
        }
    }
}
