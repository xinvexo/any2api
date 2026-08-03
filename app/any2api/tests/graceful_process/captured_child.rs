use std::{
    io::{self, Read},
    process::{Child, ExitStatus},
    thread::{self, JoinHandle},
};

pub(super) struct CapturedChild {
    child: Child,
    stdout: Option<JoinHandle<io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

impl CapturedChild {
    pub(super) fn new(mut child: Child) -> Self {
        let stdout = child.stdout.take().expect("child stdout pipe");
        let stderr = child.stderr.take().expect("child stderr pipe");
        Self {
            child,
            stdout: Some(spawn_reader(stdout)),
            stderr: Some(spawn_reader(stderr)),
        }
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub(super) fn output_after_exit(&mut self) -> String {
        let mut output =
            String::from_utf8_lossy(&join_reader(&mut self.stdout, "stdout")).into_owned();
        output.push_str(&String::from_utf8_lossy(&join_reader(
            &mut self.stderr,
            "stderr",
        )));
        output
    }

    fn discard_output(&mut self) {
        for reader in [&mut self.stdout, &mut self.stderr] {
            if let Some(reader) = reader.take() {
                let _ = reader.join();
            }
        }
    }
}

impl Drop for CapturedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.discard_output();
    }
}

fn spawn_reader<R>(mut reader: R) -> JoinHandle<io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_reader(reader: &mut Option<JoinHandle<io::Result<Vec<u8>>>>, name: &str) -> Vec<u8> {
    reader
        .take()
        .unwrap_or_else(|| panic!("{name} reader was already consumed"))
        .join()
        .unwrap_or_else(|_| panic!("{name} reader panicked"))
        .unwrap_or_else(|error| panic!("failed to drain child {name}: {error}"))
}
